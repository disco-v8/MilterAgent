// =========================
// parse.rs
// MilterAgent メールパース処理モジュール
//
// 【このファイルで使う主なクレート】
// - mail_parser: MIMEメール構造解析・ヘッダ抽出・エンコーディング処理（Message, MessageParser, MimeHeaders等）
// - std: 標準ライブラリ（コレクション、I/O、文字列操作など）
// - crate::printdaytimeln!: JSTタイムスタンプ付きログ出力マクロ
//
// 【役割】
// - BODYEOB時のヘッダ＋ボディ合体処理
// - mail-parserによるMIME構造パース（マルチパート対応）
// - From/To/Subject/Content-Type/エンコーディング等のメタ情報抽出・出力
// - テキストパート（text/plain, text/html）の本文抽出・出力
// - 非テキストパート（添付ファイル等）の属性情報抽出・出力
// - NULバイト混入の可視化・除去処理
// - フィルター処理用の正規化されたデータ構造への変換
// =========================

use mail_parser::{MessageParser, MimeHeaders}; // メールパース・MIMEヘッダアクセス用
use std::collections::HashMap;

use crate::init::{LOG_DEBUG, LOG_INFO, LOG_TRACE};

/// パース済みメール情報の構造体
#[derive(Debug, Clone)]
pub struct ParsedMail {
    pub decode_remote_host: String,
    pub decode_remote_ip: String,
    pub decode_from: String,
    pub decode_to: String,
    pub decode_subject: String,
    pub decode_text: String,
    pub decode_html: String,
    pub header_fields: HashMap<String, String>,
    pub macro_fields: HashMap<String, String>,
}

impl ParsedMail {
    /// フィルター処理用のHashMapに変換（効率的な一括変換）
    ///
    /// # 戻り値
    /// - HashMap<String, String>: フィルター処理で使用するキー・バリューペア
    ///
    /// # 説明
    /// - デコード済み情報（decode_from, decode_to等）を格納
    /// - 生ヘッダー情報（header_fields）をマージ
    /// - 重複するキーがあれば生ヘッダー情報が優先される
    pub fn into_hash_map(self) -> HashMap<String, String> {
        let mut map = HashMap::new();

        // デコード済み情報を格納
        map.insert("decode_remote_host".to_string(), self.decode_remote_host);
        map.insert("decode_remote_ip".to_string(), self.decode_remote_ip);
        map.insert("decode_from".to_string(), self.decode_from);
        map.insert("decode_to".to_string(), self.decode_to);
        map.insert("decode_subject".to_string(), self.decode_subject);
        map.insert("decode_text".to_string(), self.decode_text);
        map.insert("decode_html".to_string(), self.decode_html);

        // マクロ情報をマージ
        map.extend(self.macro_fields);
        // 生ヘッダー情報をマージ（既存のキーがあれば上書き）
        map.extend(self.header_fields);

        map
    }
}

/// BODYEOB時にヘッダ＋ボディを合体してメール全体をパース・出力する関数
///
/// # 引数
/// - `header_fields`: Milterで受信したヘッダ情報（HashMap<String, Vec<String>>）
/// - `body_field`: Milterで受信したボディ情報（文字列）
/// - `macro_fields`: Milterで受信したマクロ情報（HashMap<String, String>）
///
/// # 戻り値
/// - Some(ParsedMail): パース成功時の構造化データ
/// - None: パース失敗時
///
/// # 説明
/// 1. ヘッダ＋ボディを合体してメール全体の生データを構築
/// 2. mail-parserでMIME構造をパース
/// 3. From/To/Subject/Content-Type/エンコーディング等の情報を出力（デバッグ用）
/// 4. パートごとのテキスト/非テキスト判定・出力（デバッグ用）
/// 5. 添付ファイル名抽出・属性出力（デバッグ用）
/// 6. NULバイト混入の可視化・除去
/// 7. 構造化されたパース結果を返却
pub fn parse_mail(
    header_fields: &HashMap<String, Vec<String>>,
    body_field: &str,
    macro_fields: &HashMap<String, String>,
) -> Option<ParsedMail> {
    // ヘッダ情報とボディ情報を合体し、RFC準拠のメール全体文字列を作成
    let mut mail_string = String::new(); // メール全体の文字列構築用バッファ

    // Milterで受信した各ヘッダを「ヘッダ名: 値」形式でメール文字列に追加
    for (k, vlist) in header_fields {
        // 同一ヘッダ名で複数値がある場合（Received等）も全て処理
        for v in vlist {
            mail_string.push_str(&format!("{k}: {v}\r\n")); // RFC準拠のCRLF改行
        }
    }

    mail_string.push_str("\r\n"); // ヘッダ部とボディ部の区切り空行（RFC必須）

    // ボディ部の改行コードをCRLFに統一（OS依存の改行コード差異を吸収）
    let body_crlf = body_field.replace("\r\n", "\n").replace('\n', "\r\n");
    mail_string.push_str(&body_crlf); // 正規化されたボディを追加

    // NULバイト（\0）を可視化文字に置換してデバッグ出力用に整形
    let mail_string_visible = mail_string.replace("\0", "<NUL>");
    crate::printdaytimeln!(LOG_DEBUG, "[parser] --- BODYEOB時のメール全体 ---");
    crate::printdaytimeln!(LOG_DEBUG, "{}", mail_string_visible); // 生メールデータの可視化出力
    crate::printdaytimeln!(
        LOG_DEBUG,
        "[parser] --- BODYEOB時のメール全体、ここまで ---"
    );

    // mail-parserでメール全体をパース（バイト配列として処理）
    let parser = MessageParser::default(); // パーサーインスタンス生成（デフォルト設定）
    if let Some(msg) = parser.parse(mail_string.as_bytes()) {
        // === パース成功時の処理開始 ===

        // === マクロ情報から接続情報を抽出 ===
        let (remote_host, remote_ip) = if let Some(macro_space) = macro_fields.get("MACRO_Space") {
            // "unknown [81.30.107.177]" のような形式から情報を抽出
            let mut host = "unknown".to_string();
            let mut ip = "unknown".to_string();

            // IPアドレス部分を抽出 "[xxx.xxx.xxx.xxx]" 形式
            if let Some(start) = macro_space.find('[')
                && let Some(end) = macro_space.find(']')
            {
                ip = macro_space[start + 1..end].to_string();
            }

            // ホスト名部分を抽出（IP部分より前）
            if let Some(bracket_pos) = macro_space.find('[') {
                host = macro_space[..bracket_pos].trim().to_string();
            }

            (host, ip)
        } else {
            ("unknown".to_string(), "unknown".to_string())
        };

        // 基本情報の出力1
        crate::printdaytimeln!(LOG_INFO, "[parser] remote_host: {}", remote_host);
        crate::printdaytimeln!(LOG_INFO, "[parser] remote_ip: {}", remote_ip);

        // === 差出人（From）情報の抽出・整形 ===
        let from = msg
            .from()
            .map(|addrs| {
                addrs
                    .iter()
                    .map(|addr| {
                        let name = addr.name().unwrap_or(""); // 差出人名（表示名）
                        let address = addr.address().unwrap_or(""); // メールアドレス
                        if !name.is_empty() {
                            format!("{name} <{address}>") // 名前付きフォーマット
                        } else {
                            address.to_string() // アドレスのみ
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ") // 複数アドレスをカンマ区切りで連結
            })
            .unwrap_or_else(|| "(なし)".to_string()); // From無し時のデフォルト値

        // === 宛先（To）情報の抽出・整形 ===
        let to = msg
            .to()
            .map(|addrs| {
                addrs
                    .iter()
                    .map(|addr| {
                        let name = addr.name().unwrap_or(""); // 宛先名（表示名）
                        let address = addr.address().unwrap_or(""); // 宛先メールアドレス
                        if !name.is_empty() {
                            format!("{name} <{address}>") // 名前付きフォーマット
                        } else {
                            address.to_string() // アドレスのみ
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ") // 複数アドレスをカンマ区切りで連結
            })
            .unwrap_or_else(|| "(なし)".to_string()); // To無し時のデフォルト値

        // === 件名（Subject）情報の抽出 ===
        let subject = msg.subject().unwrap_or("(なし)"); // 件名無し時のデフォルト値

        // 基本情報の出力2
        crate::printdaytimeln!(LOG_INFO, "[parser] from: {}", from); // From出力
        crate::printdaytimeln!(LOG_INFO, "[parser] to: {}", to); // To出力
        crate::printdaytimeln!(LOG_INFO, "[parser] subject: {}", subject); // 件名出力

        // === Content-Type（MIMEタイプ）情報の抽出・出力 ===
        if let Some(ct) = msg
            .headers()
            .iter()
            .find(|h| h.name().eq_ignore_ascii_case("Content-Type")) // 大文字小文字無視でヘッダ検索
            .map(|h| h.value())
        {
            crate::printdaytimeln!(LOG_TRACE, "[parser] content-type: {:?}", ct);
            // MIMEタイプ出力
        }

        // === Content-Transfer-Encoding（エンコーディング方式）情報の抽出・出力 ===
        if let Some(enc) = msg
            .headers()
            .iter()
            .find(|h| h.name().eq_ignore_ascii_case("Content-Transfer-Encoding")) // 大文字小文字無視でヘッダ検索
            .map(|h| h.value())
        {
            crate::printdaytimeln!(LOG_TRACE, "[parser] encoding: {:?}", enc); // エンコーディング出力
        }

        // === メール構造（マルチパート/シングルパート）の判定・出力 ===
        if msg.parts.len() > 1 {
            crate::printdaytimeln!(LOG_TRACE, "[parser] このメールはマルチパートです");
        // 複数パート
        } else {
            crate::printdaytimeln!(LOG_TRACE, "[parser] このメールはシングルパートです");
            // 単一パート
        }

        // === パート分類処理（テキスト/非テキスト判定） ===
        let mut text_count = 0; // テキストパート数のカウンタ
        let mut non_text_count = 0; // 非テキストパート数のカウンタ
        let mut text_indices = Vec::new(); // テキストパートのインデックス格納配列

        // 各パートを走査し、テキスト/非テキストを分類
        for (i, part) in msg.parts.iter().enumerate() {
            if part.is_text() {
                // multipart/*は親パートなので除外（実際のテキストではない）
                let is_multipart = part
                    .content_type()
                    .is_some_and(|ct| ct.c_type.eq_ignore_ascii_case("multipart"));
                if !is_multipart {
                    text_count += 1; // テキストパート数カウント
                    text_indices.push(i); // テキストパートのインデックス記録
                }
            } else {
                non_text_count += 1; // 非テキストパート数カウント
            }
        }

        // パート分類結果の出力
        crate::printdaytimeln!(LOG_TRACE, "[parser] テキストパート数: {}", text_count); // テキストパート数出力
        crate::printdaytimeln!(LOG_TRACE, "[parser] 非テキストパート数: {}", non_text_count); // 非テキストパート数出力

        // === テキストパート本文の抽出・出力処理 ===
        let mut all_text = String::new(); // 全テキストパート連結用バッファ
        let mut all_html = String::new(); // 全HTMLパート連結用バッファ

        // テキストパート本文を出力
        for (idx, _) in text_indices.iter().enumerate() {
            let part = &msg.parts[text_indices[idx]]; // 対象テキストパートの取得

            // パートのサブタイプ（text/plain, text/htmlなど）を取得
            let subtype = part
                .content_type()
                .and_then(|ct| ct.c_subtype.as_deref().map(|s| s.to_ascii_lowercase()));

            if let Some(subtype) = subtype {
                // plainまたはhtmlのテキストパートのみ処理
                if subtype == "plain" || subtype == "html" {
                    let text = msg.body_text(idx); // プレーンテキスト本文取得
                    let html = msg.body_html(idx); // HTML本文取得

                    // プレーンテキスト本文があれば出力
                    if let Some(body) = text {
                        crate::printdaytimeln!(
                            LOG_TRACE,
                            "[parser] TEXT本文({}): {}",
                            idx + 1,
                            body
                        ); // テキスト本文出力
                        all_text.push_str(&body); // 連結用バッファに追加
                        all_text.push('\n'); // パート間の区切り改行
                    }

                    // HTML本文があれば出力
                    if let Some(html_body) = html {
                        crate::printdaytimeln!(
                            LOG_TRACE,
                            "[parser] HTML本文({}): {}",
                            idx + 1,
                            html_body
                        ); // HTML本文出力
                        all_html.push_str(&html_body); // 連結用バッファに追加
                        all_html.push('\n'); // パート間の区切り改行
                    }
                }
            }
        }

        // === 非テキストパート（添付ファイル等）の情報抽出・出力処理 ===
        let mut non_text_idx = 0; // 非テキストパートの出力用インデックス

        // 非テキストパート情報を出力
        for part in msg.parts.iter() {
            if !part.is_text() {
                // Content-Type取得（MIMEタイプ情報）
                let ct = part
                    .headers
                    .iter()
                    .find(|h| h.name().eq_ignore_ascii_case("content-type"))
                    .map(|h| format!("{:?}", h.value()))
                    .unwrap_or("(不明)".to_string());

                // エンコーディング取得（Base64, quoted-printable等）
                let encoding_str = format!("{:?}", part.encoding);

                // ファイル名取得（Content-Disposition優先、なければContent-Typeのname属性）
                let fname = part
                    .content_disposition()
                    .and_then(|cd| {
                        cd.attributes()
                            .unwrap_or(&[])
                            .iter()
                            .find(|attr| attr.name.eq_ignore_ascii_case("filename"))
                            .map(|attr| attr.value.to_string())
                    })
                    .or_else(|| {
                        part.content_type().and_then(|ct| {
                            ct.attributes()
                                .unwrap_or(&[])
                                .iter()
                                .find(|attr| attr.name.eq_ignore_ascii_case("name"))
                                .map(|attr| attr.value.to_string())
                        })
                    })
                    .unwrap_or_else(|| "(ファイル名なし)".to_string());

                let size = part.body.len(); // パートサイズ（バイト数）

                // 非テキストパート詳細情報の出力
                crate::printdaytimeln!(
                    LOG_TRACE,
                    "[parser] 非テキストパート({}): content_type={}, encoding={}, filename={}, size={} bytes",
                    non_text_idx + 1,
                    ct,
                    encoding_str,
                    fname,
                    size
                ); // 非テキストパート情報出力
                non_text_idx += 1; // インデックスを次へ
            }
        }

        // === マクロ情報をフィルター用データに変換・格納 ===
        let mut macro_fields_for_filter = HashMap::new();
        macro_fields_for_filter.insert("macro_remote_host".to_string(), remote_host.clone());
        macro_fields_for_filter.insert("macro_remote_ip".to_string(), remote_ip.clone());

        // その他のマクロ情報も格納
        for (k, v) in macro_fields {
            let key_lower = k.to_ascii_lowercase(); // マクロ名を小文字化
            macro_fields_for_filter.insert(format!("macro_{key_lower}"), v.clone());
        }

        // === 生ヘッダー情報をフィルター用データに変換・格納 ===
        let mut header_fields_for_filter = HashMap::new();
        for (k, vlist) in header_fields {
            let joined = vlist.join(", "); // ヘッダ値をカンマ区切りで連結
            let key_lower = k.to_ascii_lowercase(); // ヘッダ名を小文字化

            // 主要ヘッダ（from, to, subject）の処理
            match key_lower.as_str() {
                "from" | "to" | "subject" => {
                    header_fields_for_filter.insert(format!("header_{key_lower}"), joined);
                    // header_接頭辞付きで格納
                }
                _ => {
                    header_fields_for_filter.insert(key_lower, joined); // その他ヘッダはそのまま格納
                }
            }
        }

        // パース結果の構造体を構築して返却
        return Some(ParsedMail {
            decode_remote_host: remote_host,
            decode_remote_ip: remote_ip,
            decode_from: from,
            decode_to: to,
            decode_subject: subject.to_string(),
            decode_text: all_text,
            decode_html: all_html,
            header_fields: header_fields_for_filter,
            macro_fields: macro_fields_for_filter,
        });
    }

    // パース失敗時はNoneを返す
    None // パース失敗：Noneを返却
}
