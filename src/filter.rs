// =========================
// filter.rs
// MilterAgent フィルター判定ロジック専用モジュール
//
// 【このファイルで使う主なクレート】
// - std::collections::HashMap: メール内容のキー・バリュー格納（header_from, decode_text等）
// - crate::init::CONFIG: グローバル設定（フィルター条件、正規表現等）
// - fancy-regex: 高機能正規表現マッチング判定（負先読み・後読み対応、メール内容パターンマッチ）
// - unicode-normalization: 文字列のUnicode正規化（NFKC変換、BOMや制御文字除去など）
//
// 【役割】
// - mail-parserで抽出されたメール内容（差出人、件名、本文等）とフィルター設定の照合
// - AND/OR/判定アクション(REJECT/DROP/WARN/ACCEPT)の論理処理
// - フィルター結果の出力（REJECT等の判定結果をmilter_commandに返す）
// =========================

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;
use unicode_normalization::UnicodeNormalization;

use crate::init::{Config, LOG_INFO, LOG_TRACE};
use crate::init::LOG_DEBUG;

/// フィルター判定関数（並列処理版）
/// - mail_values: キーごとの値（header_～, decode_～など）
/// - 設定済みフィルターに従い判定し、結果（REJECT/DROP/WARN/ACCEPT等）を出力
///
/// # 引数
/// - _mail_values: mail-parserで抽出されたメール内容のHashMap
///   * header_from: 生ヘッダーFromフィールド
///   * decode_from: デコード済み差出人名
///   * decode_text: デコード済みテキスト本文
///   * decode_html: デコード済みHTML本文
///   * その他多数のメールヘッダー・本文情報
///
/// # 処理フロー（並列処理版）
/// 1. 全フィルター設定をスレッドチャンクに分割
/// 2. 各スレッドで並列にフィルター判定を実行
/// 3. いずれかのスレッドでNONE以外の結果が出たら他スレッドを停止
/// 4. 結果をメインスレッドで集約してログ出力
pub fn filter_check(
    _mail_values: &HashMap<String, String>,
    config: &Config,
) -> Option<(String, String)> {
    // _mail_valuesの値をNFKC正規化したHashMapを作成
    let normalized_mail_values: HashMap<String, String> = _mail_values
        .iter()
        .map(|(k, v)| (k.clone(), normalize_mail_value(v)))
        .collect();

    // フィルター設定をベクタに変換（所有権付きで並列処理用）
    let filters: Vec<(String, Vec<crate::init::FilterRule>)> = config
        .filters
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    if filters.is_empty() {
        // フィルターが設定されていない場合は"NONE"と"none"を返す
        // これにより、フィルターなしの状態でも正常に動作する
        return Some(("NONE".to_string(), "none".to_string()));
    }

    // 利用可能なCPUコア数を取得（最大8スレッドに制限）
    let num_threads = std::cmp::min(
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4),
        8,
    );
    let chunk_size = filters.len().div_ceil(num_threads);

    // 早期終了フラグ（AtomicBool）
    let should_stop = Arc::new(AtomicBool::new(false));

    // 結果格納用のMutex
    let result = Arc::new(Mutex::new(None::<(String, String)>)); // (action, logname)

    // スレッド実行時間収集用（thread_index, Duration）
    let thread_stats = Arc::new(Mutex::new(Vec::<(usize, std::time::Duration)>::new()));

    // スレッドハンドルを格納するベクタ
    let mut handles = vec![];

    // フィルターをチャンクに分割して各スレッドで処理
    for (i, chunk_filters) in filters.chunks(chunk_size).enumerate() {
        let chunk_filters = chunk_filters.to_vec();
        let should_stop = Arc::clone(&should_stop);
        let result = Arc::clone(&result);
        let mail_values = normalized_mail_values.clone();
        let thread_stats = Arc::clone(&thread_stats);

        let handle = thread::spawn(move || {
            let t0 = Instant::now();
            // チャンク内の各フィルターを順次処理
            for (logname, rules) in chunk_filters {
                // 早期終了チェック
                if should_stop.load(Ordering::Relaxed) {
                    break;
                }

                if rules.is_empty() {
                    continue;
                }

                // 個別フィルターの処理（フィルター名も渡す）
                let filter_result =
                    process_single_filter(&mail_values, &rules, &should_stop, &logname);

                // NONE以外の結果が出た場合
                if filter_result != "NONE" {
                    // 他スレッドに停止シグナル送信
                    should_stop.store(true, Ordering::Relaxed);

                    // 結果を格納
                    {
                        let mut result_guard = result.lock().unwrap();
                        if result_guard.is_none() {
                            *result_guard = Some((filter_result, logname));
                        }
                    }
                    break;
                }
            }
            // スレッドの処理時間を記録
            let elapsed = t0.elapsed();
            if let Ok(mut st) = thread_stats.lock() {
                st.push((i, elapsed));
            }
        });

        handles.push(handle);
    }

    // 全スレッドの完了を待機
    for handle in handles {
        handle.join().unwrap();
    }

    // スレッド実行時間を出力（thread_stats をスレッド番号順にソートして表示）
    if let Ok(mut stats) = thread_stats.lock() {
        stats.sort_by_key(|(i, _)| *i);
        for (idx, dur) in stats.iter() {
            crate::printdaytimeln!(LOG_INFO, "[filter] thread{} elapsed: {:?}", idx, dur);
        }
    }

    // 結果を返す
    let final_result = result.lock().unwrap();
    // Noneの場合は"NONE"と"none"を返す
    // それ以外はアクションとログ名を返す
    match &*final_result {
        Some((action, logname)) => Some((action.clone(), logname.clone())),
        None => Some(("NONE".to_string(), "none".to_string())),
    }
}

/// 文字列をNFKC正規化し、不要な制御文字や結合文字を除去する
/// - NFKC: 全角/半角・合成文字などを正規化
/// - BOMやゼロ幅スペース、双方向制御文字、結合記号などを除去
/// - 最後に空白を除去して連結
fn normalize_mail_value(s: &str) -> String {
    // 1. NFKC正規化（全角/半角・合成文字などを統一）
    let nfkc = s.nfkc().collect::<String>();

    // 2. 不要な制御文字・結合文字を除去
    let cleaned: String = nfkc
        .chars()
        .filter(|c| {
            let code = *c as u32;
            !(code == 0xFEFF || // BOM
              (0x0000..=0x001F).contains(&code) || // C0 controls
              code == 0x007F || // DEL
              (0x200B..=0x200F).contains(&code) || // ZWSP, ZWNJ, ZWJ, LRM, RLM
              (0x202A..=0x202E).contains(&code) || // Bidi controls
              (0x2060..=0x206F).contains(&code) || // Word Joiner etc.
              (0x0300..=0x036F).contains(&code) || // Combining diacritics
              (0x2000..=0x200A).contains(&code) || // Invisible spaces
              code == 0x202F || // Narrow NBSP
              code == 0x00A0 || // NBSP
              (0xFE00..=0xFE0F).contains(&code) || // Variation Selectors
              code == 0x180E) // Mongolian Vowel Separator
        })
        .collect();

    // 3. 空白（スペース・改行等）で分割し、すべて連結（余計な空白を除去）
    cleaned.split_whitespace().collect::<String>()
}

/// 単一フィルターの処理（既存ロジックをヘルパー関数化）
/// name: フィルター名（filter[xxx]のxxx部分）
fn process_single_filter(
    mail_values: &HashMap<String, String>,
    rules: &[crate::init::FilterRule],
    should_stop: &Arc<AtomicBool>,
    name: &str,
) -> String {
    let mut idx = 0;

    while idx < rules.len() {
        // 早期終了チェック
        if should_stop.load(Ordering::Relaxed) {
            return "NONE".to_string();
        }

        let rule = &rules[idx];

        // メール内容から対象キーの値を取得
        let value = mail_values.get(&rule.key).map(|s| s.as_str()).unwrap_or("");

        // 判定方法をキーごとに分岐
        // decode_textは改行ごとに分割して判定
        let (is_match, matched_str) = if rule.key == "decode_text" {
            let mut found = false; // 一致フラグ
            let mut matched = ""; // 一致した部分文字列
            for line in value.lines() {
                // 1行ずつ処理
                // 正規表現で部分一致判定
                if rule.regex.is_match(line).unwrap_or(false) {
                    found = true; // 一致したらフラグON
                    // 最初に一致した部分文字列を取得
                    matched = rule
                        .regex
                        .captures_iter(line)
                        .next()
                        .and_then(|res| res.ok())
                        .and_then(|caps| caps.get(0).map(|m| m.as_str()))
                        .unwrap_or("");
                    break; // 最初の一致で抜ける
                } else {
                    // マッチしなかった場合はDEBUGで値を出力（フィルター名も出力）
                    crate::printdaytimeln!(
                        LOG_DEBUG,
                        "[filter] name='{}' key='{}' pattern='{}' not_matched='{}'",
                        name,
                        rule.key,
                        rule.regex.as_str(),
                        value
                    );
                }
            }
            (found, matched)
        }
        // decode_htmlは「>」または改行ごとに分割して判定（HTMLタグ・属性単位）
        else if rule.key == "decode_html" {
            let mut found = false; // 一致フラグ
            let mut matched = ""; // 一致した部分文字列
            // 「>」または改行で分割して判定
            for chunk in value.split(['"', '>', '\n']) {
                // 正規表現で部分一致判定
                if rule.regex.is_match(chunk).unwrap_or(false) {
                    found = true; // 一致したらフラグON
                    // 最初に一致した部分文字列を取得
                    matched = rule
                        .regex
                        .captures_iter(chunk)
                        .next()
                        .and_then(|res| res.ok())
                        .and_then(|caps| caps.get(0).map(|m| m.as_str()))
                        .unwrap_or("");
                    break; // 最初の一致で抜ける
                } else {
                    // マッチしなかった場合はDEBUGで値を出力（フィルター名も出力）
                    crate::printdaytimeln!(
                        LOG_DEBUG,
                        "[filter] name='{}' key='{}' pattern='{}' not_matched='{}'",
                        name,
                        rule.key,
                        rule.regex.as_str(),
                        value
                    );
                }
            }
            (found, matched)
        }
        // それ以外のキーは値全体で判定
        else {
            // 正規表現で部分一致判定（値全体）
            let is_match = rule.regex.is_match(value).unwrap_or(false);
            // 一致した場合は最初の一致部分を取得
            let matched_str = if is_match {
                rule.regex
                    .captures_iter(value)
                    .next()
                    .and_then(|res| res.ok())
                    .and_then(|caps| caps.get(0).map(|m| m.as_str()))
                    .unwrap_or("")
            } else {
                ""
            };
            (is_match, matched_str)
        };

        // negate指定がある場合は結果を反転
        let ok = if rule.negate { !is_match } else { is_match };

        // マッチした場合は一致した文字列をログ出力（フィルター名も出力）
        if is_match {
            crate::printdaytimeln!(
                LOG_TRACE,
                "[filter] name='{}' key='{}' pattern='{}' matched='{}'",
                name,
                rule.key,
                rule.regex.as_str(),
                matched_str
            );
        }

        // アクション種別を大文字化
        let action_upper = rule.action.to_ascii_uppercase();

        // AND条件の処理
        if action_upper == "AND" {
            if ok {
                idx += 1;
                continue;
            } else {
                break;
            }
        }
        // OR条件の処理
        else if action_upper == "OR" {
            if ok {
                // 最終判定アクションを探して返す
                for j in (0..rules.len()).rev() {
                    let last_action = rules[j].action.to_ascii_uppercase();
                    if last_action != "AND" && last_action != "OR" {
                        return rules[j].action.clone();
                    }
                }
                return "NONE".to_string();
            } else {
                idx += 1;
                continue;
            }
        }
        // 最終判定アクション
        else if ok {
            return rule.action.clone();
        } else {
            break;
        }
    }

    "NONE".to_string()
}
