// =========================
// filter.rs
// MilterAgent フィルター判定ロジック専用モジュール
//
// 【このファイルで使う主なクレート】
// - std::collections::HashMap: メール内容のキー・バリュー格納（header_from, decode_text等）
// - crate::init::CONFIG: グローバル設定（フィルター条件、正規表現等）
// - regex: 正規表現マッチング判定（mail-parserで抽出したメール内容とのパターンマッチ）
//
// 【役割】
// - mail-parserで抽出されたメール内容（差出人、件名、本文等）とフィルター設定の照合
// - AND/OR/判定アクション(REJECT/DROP/WARN/ACCEPT)の論理処理
// - フィルター結果の出力（REJECT等の判定結果をmilter_commandに返す）
// =========================

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use crate::init::Config;

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
pub fn filter_check(_mail_values: &HashMap<String, String>, config: &Config) -> Option<(String, String)> {
    // フィルター設定をベクタに変換（所有権付きで並列処理用）
    let filters: Vec<(String, Vec<crate::init::FilterRule>)> = config.filters.iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    
    if filters.is_empty() {
        // フィルターが設定されていない場合は"NONE"と"none"を返す
        // これにより、フィルターなしの状態でも正常に動作する
        return Some(("NONE".to_string(), "none".to_string()));
    }

    // 利用可能なCPUコア数を取得（最大8スレッドに制限）
    let num_threads = std::cmp::min(
        std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4),
        8
    );
    let chunk_size = filters.len().div_ceil(num_threads);

    // 早期終了フラグ（AtomicBool）
    let should_stop = Arc::new(AtomicBool::new(false));
    
    // 結果格納用のMutex
    let result = Arc::new(Mutex::new(None::<(String, String)>)); // (action, logname)

    // スレッドハンドルを格納するベクタ
    let mut handles = vec![];

    // フィルターをチャンクに分割して各スレッドで処理
    for chunk_filters in filters.chunks(chunk_size) {
        let chunk_filters = chunk_filters.to_vec();
        let should_stop = Arc::clone(&should_stop);
        let result = Arc::clone(&result);
        let mail_values = _mail_values.clone();

        let handle = thread::spawn(move || {
            // チャンク内の各フィルターを順次処理
            for (logname, rules) in chunk_filters {
                // 早期終了チェック
                if should_stop.load(Ordering::Relaxed) {
                    break;
                }

                if rules.is_empty() { continue; }

                // 個別フィルターの処理（既存のロジックと同じ）
                let filter_result = process_single_filter(&mail_values, &rules, &should_stop);
                
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
        });

        handles.push(handle);
    }

    // 全スレッドの完了を待機
    for handle in handles {
        handle.join().unwrap();
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

/// 単一フィルターの処理（既存ロジックをヘルパー関数化）
fn process_single_filter(
    mail_values: &HashMap<String, String>,
    rules: &[crate::init::FilterRule],
    should_stop: &Arc<AtomicBool>
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
        
        // 正規表現でパターンマッチング実行
        let is_match = rule.regex.is_match(value);
        
        // negate指定がある場合は結果を反転
        let ok = if rule.negate { !is_match } else { is_match };
        
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