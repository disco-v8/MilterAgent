// =========================
// init.rs - MilterAgent 設定管理モジュール
// =========================
//
// 【使用クレート】
// - fancy-regex: 高機能正規表現エンジン（負先読み・後読み対応、メール内容パターンマッチング用）
// - std::fs: ファイルシステム操作（設定ファイルの読み書き）
// - std::path: ファイルパス処理（設定ファイルのパス操作）
// - std::io::BufRead: バッファ付きファイル読み込み（大容量設定ファイル対応）
//
// 【主要機能】
// 1. メイン設定ファイル(MilterAgent.conf)の解析
// 2. includeディレクトリ内の追加設定ファイル(.conf)の再帰読み込み
// 3. サーバー設定（Listen、タイムアウト、ログレベル等）の構造化
// 4. フィルタールール設定の解析とValidation
// 5. Spamhaus API連携設定の管理

use fancy_regex::Regex;

// フィルタールール構造体
// メールの内容や送信者情報に対する判定条件を定義
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct FilterRule {
    pub key: String,    // フィルター対象フィールド（decode_from, decode_subject, body等）
    pub regex: Regex,   // マッチング用正規表現パターン
    pub negate: bool,   // 否定条件フラグ（!で指定時はtrue）
    pub logic: String,  // 複数条件の論理演算子（AND/OR）
    pub action: String, // マッチ時の実行アクション（REJECT/DROP/WARN/ACCEPT）
}

// メイン設定構造体
// 設定ファイルから読み込んだ全ての設定項目を保持
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Config {
    pub address: String,          // Milterサーバー待受アドレス（例: "[::]:8898"）
    pub client_timeout: u64,      // SMTPクライアント無応答タイムアウト時間（秒）
    pub log_file: Option<String>, // ログ出力先ファイルパス（None時は標準出力）
    pub log_level: u8,            // ログ詳細度（0=info, 2=trace, 8=debug）
    pub filters: Vec<(String, Vec<FilterRule>)>, // フィルター定義リスト（名前とルールセット）
    pub spamhaus_report: bool,    // Spamhaus情報をログ出力するかのフラグ
    pub spamhaus_api_token: Option<String>, // Spamhaus API認証用トークン
    pub spamhaus_api_url: Option<String>, // Spamhaus API接続先エンドポイント
    pub spamhaus_safe_addresses: Vec<String>, // Spamhaus通知除外IPアドレス/ネットワーク
}

// ログレベル定数定義
// アプリケーション全体で使用するログ出力レベルの統一定義
pub const LOG_INFO: u8 = 0; // 通常運用情報（エラー、警告、重要な動作）
pub const LOG_TRACE: u8 = 2; // 処理トレース情報（関数の入出力、状態変化）
pub const LOG_DEBUG: u8 = 8; // デバッグ詳細情報（変数値、内部処理詳細）

// 設定ファイル読み込み・解析のメイン関数
// 指定パスの設定ファイルを読み込み、構造化されたConfig オブジェクトを生成
//
// # 引数
// * `path` - 設定ファイルのパス（.conf ファイル）
//
// # 戻り値
// * `Config` - 解析済み設定情報オブジェクト
//
// # 処理フロー
// 1. 設定ファイルをテキストとして読み込み
// 2. include ディレクトリ内の追加設定ファイルを再帰的に処理
// 3. 各設定項目を対応する構造体フィールドにマッピング
// 4. フィルタールールの正規表現をコンパイル・検証
//
// # デフォルト値
// * Listen: "[::]:8898"（IPv6全アドレス、ポート8898）
// * Client_timeout: 30秒
// * Log_level: 0 (info レベル)
pub fn load_config<P: AsRef<std::path::Path>>(path: P) -> Config {
    // 内部用設定値一時保持構造体
    // ファイル解析中に設定値を蓄積し、最終的にConfig構造体に変換するためのワーク領域
    struct ConfigValues {
        address: Option<String>,                 // サーバー待受アドレス
        client_timeout: u64,                     // 接続タイムアウト時間
        log_file: Option<String>,                // ログファイル出力先
        log_level: u8,                           // ログ詳細度設定
        filters: Vec<(String, Vec<FilterRule>)>, // フィルタールール群
        spamhaus_report: bool,                   // Spamhaus連携フラグ
        spamhaus_api_token: Option<String>,      // API認証トークン
        spamhaus_api_url: Option<String>,        // API接続先URL
        spamhaus_safe_addresses: Vec<String>,    // Spamhaus通知除外IPアドレス/ネットワーク
    }

    // 設定テキスト行単位解析関数
    // 設定ファイルの内容を1行ずつ処理し、ConfigValues構造体に値を格納
    fn parse_config_text(text: &str, values: &mut ConfigValues) {
        // キー・値分割ヘルパー関数
        // "Key Value" または "Key\tValue" 形式の行を解析
        // 戻り値: (キー文字列, 値文字列) のタプル、または None
        fn split_key_value(line: &str) -> Option<(&str, &str)> {
            // 最初の空白文字またはタブ文字を探す
            let separator_pos = line.find([' ', '\t'])?;

            // 空白文字の連続部分をスキップして値の開始位置を特定
            let value_start = line[separator_pos..]
                .find(|c: char| !c.is_whitespace())
                .map(|pos| separator_pos + pos)?;

            let key = line[..separator_pos].trim();
            let value = line[value_start..].trim();

            if key.is_empty() || value.is_empty() {
                None
            } else {
                Some((key, value))
            }
        }

        // 設定項目キーのリスト（複数行検出用）
        fn is_config_key(line: &str) -> bool {
            line.starts_with("Listen")
                || line.starts_with("Client_timeout")
                || line.starts_with("Log_file")
                || line.starts_with("Log_level")
                || line.starts_with("Spamhaus_report")
                || line.starts_with("Spamhaus_api_token")
                || line.starts_with("Spamhaus_api_url")
                || line.starts_with("Spamhaus_safe_address")
                || line.starts_with("include")
                || line.starts_with("filter[")
        }

        // 複数行にわたる設定値を収集する統一関数
        // 次の行が新しい設定項目でなければ継続行として処理
        fn collect_multiline_value(
            lines: &mut std::iter::Peekable<std::str::Lines>,
            initial_value: &str,
            join_with_comma: bool,
        ) -> String {
            let mut current_value = initial_value.to_string();

            while let Some(peek) = lines.peek() {
                let peek_trim = peek.trim();

                // 空行または新しい設定項目の開始を検出した場合は終了
                if peek_trim.is_empty() || is_config_key(peek_trim) {
                    break;
                }

                // コメント行は読み飛ばして次の行へ
                if peek_trim.starts_with('#') {
                    lines.next();
                    continue;
                }

                // 継続行として連結
                if join_with_comma {
                    // カンマ区切りでの連結（Spamhaus_safe_address等）
                    if !current_value.trim().ends_with(',') && !peek_trim.starts_with(',') {
                        current_value.push(',');
                    }
                } else {
                    // スペース区切りでの連結（filter等）
                    current_value.push(' ');
                }
                current_value.push_str(peek_trim);
                lines.next();
            }

            current_value
        }

        let mut lines = text.lines().peekable();

        // 設定ファイルの各行を順次処理
        while let Some(line) = lines.next() {
            let line = line.trim(); // 行頭・行末の空白文字を除去

            // 空行とコメント行（#で始まる行）をスキップ
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // include ディレクトリの再帰読み込み処理
            else if line.starts_with("include") {
                crate::printdaytimeln!(LOG_DEBUG, "[init] processing include line: {}", line);
                if let Some((_, dir_path)) = split_key_value(line) {
                    // 複数行のinclude設定値を収集
                    let full_value = collect_multiline_value(&mut lines, dir_path, true);
                    crate::printdaytimeln!(
                        LOG_DEBUG,
                        "[init] include directories: '{}'",
                        full_value
                    );

                    // カンマ区切りで複数のディレクトリを処理
                    for dir in full_value.split(',') {
                        let dir = dir.trim();
                        crate::printdaytimeln!(
                            LOG_DEBUG,
                            "[init] trying to read directory: '{}'",
                            dir
                        );
                        if !dir.is_empty() {
                            if let Ok(entries) = std::fs::read_dir(dir) {
                                for entry in entries.flatten() {
                                    let path = entry.path();
                                    if path.is_file() {
                                        if let Some(ext) = path.extension() {
                                            if ext == "conf" {
                                                crate::printdaytimeln!(
                                                    LOG_INFO,
                                                    "[init] loading sub-conf file: {}",
                                                    path.display()
                                                );
                                                if let Ok(sub_text) = std::fs::read_to_string(&path)
                                                {
                                                    crate::printdaytimeln!(
                                                        LOG_DEBUG,
                                                        "[init] file content length: {} bytes",
                                                        sub_text.len()
                                                    );
                                                    parse_config_text(&sub_text, values);
                                                } else {
                                                    crate::printdaytimeln!(
                                                        LOG_INFO,
                                                        "[init] failed to read file: {}",
                                                        path.display()
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            } else {
                                crate::printdaytimeln!(
                                    LOG_INFO,
                                    "[init] failed to read directory: '{}'",
                                    dir
                                );
                            }
                        }
                    }
                }
                continue;
            }
            // Listen設定 - Milterサーバーの待受ソケット設定
            else if line.starts_with("Listen") {
                if let Some((_, addr)) = split_key_value(line) {
                    // 複数行のListen設定値を収集
                    let full_value = collect_multiline_value(&mut lines, addr, false);
                    let addr = full_value.trim();

                    if addr.contains(':') {
                        // "host:port" または "[ipv6]:port" 形式
                        values.address = Some(addr.to_string());
                    } else {
                        // ポート番号のみの場合はIPv6全アドレスでバインド
                        if addr.parse::<u16>().is_ok() {
                            values.address = Some(format!("[::]:{}", addr));
                        } else {
                            crate::printdaytimeln!(
                                LOG_INFO,
                                "[init] Invalid address/port: {}",
                                addr
                            );
                        }
                    }
                }
            }
            // Client_timeout設定 - SMTP接続の無応答タイムアウト時間
            else if line.starts_with("Client_timeout") {
                if let Some((_, val_str)) = split_key_value(line) {
                    let full_value = collect_multiline_value(&mut lines, val_str, false);
                    if let Ok(val) = full_value.trim().parse::<u64>() {
                        values.client_timeout = val;
                    }
                }
            }
            // Log_file設定 - ログの出力先ファイルパス指定
            else if line.starts_with("Log_file") {
                if let Some((_, path)) = split_key_value(line) {
                    let full_value = collect_multiline_value(&mut lines, path, false);
                    let path = full_value.trim();
                    if !path.is_empty() {
                        values.log_file = Some(path.to_string());
                    }
                }
            }
            // Log_level設定 - ログ出力の詳細度制御
            else if line.starts_with("Log_level") {
                if let Some((_, level_str)) = split_key_value(line) {
                    let full_value = collect_multiline_value(&mut lines, level_str, false);
                    let level = full_value.trim().to_ascii_lowercase();
                    values.log_level = match level.as_str() {
                        "info" => 0,  // 基本的な動作情報のみ
                        "trace" => 2, // 処理の流れを追跡
                        "debug" => 8, // 詳細なデバッグ情報
                        _ => 0,       // 不明な値はinfoレベルに設定
                    };
                }
            }
            // Spamhaus_report設定 - Spamhaus情報のログ出力制御フラグ
            else if line.starts_with("Spamhaus_report") {
                if let Some((_, value)) = split_key_value(line) {
                    let full_value = collect_multiline_value(&mut lines, value, false);
                    let report_str = full_value.trim().to_ascii_lowercase();
                    if report_str == "yes" || report_str == "true" || report_str == "1" {
                        values.spamhaus_report = true;
                    }
                }
            }
            // Spamhaus_api_token設定 - Spamhaus API認証用トークン文字列
            else if line.starts_with("Spamhaus_api_token") {
                if let Some((_, value)) = split_key_value(line) {
                    let full_value = collect_multiline_value(&mut lines, value, false);
                    let token = full_value.trim();
                    if !token.is_empty() {
                        values.spamhaus_api_token = Some(token.to_string());
                    }
                }
            }
            // Spamhaus_api_url設定 - Spamhaus API接続先エンドポイントURL（複数行対応）
            else if line.starts_with("Spamhaus_api_url") {
                if let Some((_, value)) = split_key_value(line) {
                    let full_value = collect_multiline_value(&mut lines, value, false);
                    let url = full_value.trim();
                    if !url.is_empty() {
                        values.spamhaus_api_url = Some(url.to_string());
                    }
                }
            }
            // Spamhaus_safe_address設定 - Spamhaus通知除外IPアドレス/ネットワークリスト（カンマ区切り、複数行対応）
            else if line.starts_with("Spamhaus_safe_address") {
                if let Some((_, value)) = split_key_value(line) {
                    let full_value = collect_multiline_value(&mut lines, value, true);

                    // カンマ区切りで分割してベクターに追加
                    for addr in full_value.split(',') {
                        let addr = addr.trim();
                        if !addr.is_empty() {
                            values.spamhaus_safe_addresses.push(addr.to_string());
                        }
                    }
                }
            }
            // filter設定 - メールコンテンツ判定ルールの定義（最優先で処理）
            // 書式: filter[フィルター名] = キー:正規表現:論理演算子:アクション
            else if let Some(rest) = line.strip_prefix("filter[") {
                crate::printdaytimeln!(LOG_DEBUG, "[init] processing filter line: {}", line);
                if let Some(end_idx) = rest.find(']') {
                    let name = &rest[..end_idx]; // フィルター識別名を取得
                    let eq_idx = rest.find('='); // 等号の位置を検索

                    // まず1行目の=以降のルール定義を取得
                    let initial_rule = if let Some(eq_idx) = eq_idx {
                        rest[eq_idx + 1..].trim()
                    } else {
                        ""
                    };

                    // 複数行にわたるルール定義を統一的に収集
                    let rule_str = collect_multiline_value(&mut lines, initial_rule, false);
                    crate::printdaytimeln!(
                        LOG_DEBUG,
                        "[init] filter[{}] rule_str: '{}'",
                        name,
                        rule_str
                    );

                    // カンマ区切りで複数の判定条件を分割
                    let rule_list: Vec<&str> = rule_str
                        .split(',')
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .collect();
                    crate::printdaytimeln!(
                        LOG_DEBUG,
                        "[init] filter[{}] rule_list: {:?}",
                        name,
                        rule_list
                    );
                    let mut rules = Vec::new();

                    // 各判定条件を個別にパース
                    for rule in rule_list {
                        // エスケープされた:文字を一時的に特殊文字に置換してから分割
                        let rule_escaped = rule.replace("\\:", "\x00");
                        let parts: Vec<&str> = rule_escaped.split(':').collect();

                        // 3要素形式: キー:パターン:アクション（論理演算子なし）
                        if parts.len() == 3 {
                            let key = parts[0].trim().trim_matches('"'); // 対象フィールド名
                            let (negate, pattern_raw) = if parts[1].starts_with('!') {
                                (true, &parts[1][1..]) // 否定条件フラグ付き
                            } else {
                                (false, parts[1]) // 通常の肯定条件
                            };
                            let pattern = pattern_raw.trim().trim_matches('"').replace('\x00', ":"); // 正規表現（:復元）

                            // 正規表現のコンパイル試行
                            if let Ok(regex) = Regex::new(&pattern) {
                                let action = parts[2].trim().trim_matches('"').replace('\x00', ":"); // 実行アクション
                                rules.push(FilterRule {
                                    key: key.to_string(),
                                    regex,
                                    negate,
                                    logic: "".to_string(), // 論理演算子なし
                                    action: action.to_string(),
                                });
                            }
                        }
                        // 4要素以上形式: キー:パターン:論理演算子:アクション
                        else if parts.len() >= 4 {
                            let key = parts[0].trim().trim_matches('"');
                            let (negate, pattern_raw) = if parts[1].starts_with('!') {
                                (true, &parts[1][1..])
                            } else {
                                (false, parts[1])
                            };
                            let pattern = pattern_raw.trim().trim_matches('"').replace('\x00', ":");

                            if let Ok(regex) = Regex::new(&pattern) {
                                let logic = parts[2].trim().trim_matches('"').replace('\x00', ":"); // AND/OR演算子
                                let action = parts[3].trim().trim_matches('"').replace('\x00', ":"); // 実行アクション
                                rules.push(FilterRule {
                                    key: key.to_string(),
                                    regex,
                                    negate,
                                    logic: logic.to_string(),
                                    action: action.to_string(),
                                });
                            }
                        }
                    }
                    // フィルタールールの有効性検証
                    // 最終アクションが規定の値（REJECT/DROP/WARN/ACCEPT）かチェック
                    let valid_actions = ["REJECT", "DROP", "WARN", "ACCEPT"];
                    let is_valid = rules
                        .last()
                        .map(|r| {
                            let act = r.action.to_ascii_uppercase();
                            valid_actions.contains(&act.as_str())
                        })
                        .unwrap_or(false);

                    // 有効なフィルターのみシステムに登録
                    if is_valid {
                        values.filters.push((name.to_string(), rules));
                    } else {
                        crate::printdaytimeln!(
                            LOG_INFO,
                            "[init] filter[{}] の最終アクションがREJECT/DROP/WARN/ACCEPT以外、または未指定のため無効化",
                            name
                        );
                    }
                }
                continue;
            }
            // 未知の設定項目の処理 - 将来の拡張性のため警告を出力して無視
            else if line.contains(' ') || line.contains('\t') {
                if let Some((key, _)) = split_key_value(line) {
                    crate::printdaytimeln!(LOG_INFO, "[init] Unknown Config Key: {}", key);
                }
            }
        }
    }

    // 設定ファイル本体の読み込み実行
    let text = std::fs::read_to_string(path).expect("設定ファイル読み込み失敗");

    // 初期値を設定した作業用構造体を作成
    let mut values = ConfigValues {
        address: None,
        client_timeout: 30u64, // デフォルト30秒タイムアウト
        log_file: None,        // デフォルトは標準出力
        log_level: 0,          // デフォルトはinfoレベル
        filters: Vec::new(),
        spamhaus_report: false, // デフォルトはSpamhaus情報非出力
        spamhaus_api_token: None,
        spamhaus_api_url: None,
        spamhaus_safe_addresses: Vec::new(), // デフォルトは空のホワイトリスト
    };

    // 設定ファイル内容の解析実行
    parse_config_text(&text, &mut values);

    // フィルター内容をデバッグ出力
    crate::printdaytimeln!(
        LOG_INFO,
        "[init] filters loaded: {}個",
        values.filters.len()
    );
    for (name, rules) in &values.filters {
        crate::printdaytimeln!(LOG_DEBUG, "[init] filter[{}] 条件数={}:", name, rules.len());
        for (i, rule) in rules.iter().enumerate() {
            crate::printdaytimeln!(
                LOG_DEBUG,
                "  [{}] key='{}' pattern='{}' negate={} logic='{}' action='{}'",
                i,
                rule.key,
                rule.regex.as_str(),
                rule.negate,
                rule.logic,
                rule.action
            );
        }
    }

    // 最終的なConfig構造体を生成して返却
    Config {
        address: values.address.unwrap_or_else(|| "[::]:8898".to_string()),
        client_timeout: values.client_timeout,
        log_file: values.log_file,
        log_level: values.log_level,
        filters: values.filters,
        spamhaus_report: values.spamhaus_report,
        spamhaus_api_token: values.spamhaus_api_token,
        spamhaus_api_url: values.spamhaus_api_url,
        spamhaus_safe_addresses: values.spamhaus_safe_addresses,
    }
}
