// =========================
// init.rs
// MilterAgent 設定管理モジュール
//
// 【このファイルで使う主なクレート】
// - std: ファイル入出力（fs::read_to_string）、文字列処理（lines, trim, parse）、同期（sync::RwLock）
// - lazy_static: グローバル変数初期化（設定の静的共有）
//
// 【役割】
// - サーバー設定（Listenアドレス、クライアントタイムアウト等）の読み込み・保持
// - 設定ファイル(MilterAgent.conf)からConfig構造体を生成
// - グローバル設定CONFIGとして全体で参照可能
// =========================

// (不要なuse削除)
use regex::Regex;

/// サーバー設定情報構造体（Listen/Client_timeoutなど）
/// - address: サーバー待受アドレス（例: 0.0.0.0:8898）
/// - client_timeout: クライアント無通信タイムアウト秒

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct FilterRule {
    pub key: String,
    pub regex: Regex,
    pub negate: bool,
    pub logic: String,
    pub action: String,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub address: String,     // サーバー待受アドレス（Listen）
    pub client_timeout: u64, // クライアントタイムアウト秒（Client_timeout）
    pub log_file: Option<String>, // ログファイルパス（未指定ならNone）
    pub log_level: u8,       // ログレベル（info=0, trace=2, debug=8）
    pub filters: Vec<(String, Vec<FilterRule>)>, // フィルター設定（記述順）
}

// ログレベル定数（Configのlog_levelで使用）
pub const LOG_INFO: u8 = 0;
pub const LOG_TRACE: u8 = 2;
pub const LOG_DEBUG: u8 = 8;

/// 設定ファイル(MilterAgent.conf)からConfigを生成
///
/// # 説明
/// - Listen <アドレス/ポート>、Client_timeout <秒> をパースしてConfig構造体に格納
/// - Listen未指定時は[::]:8898、Client_timeout未指定時は30秒をデフォルト
pub fn load_config<P: AsRef<std::path::Path>>(path: P) -> Config {
    // 設定ファイルとincludeディレクトリの*.confをすべてパースする
    fn parse_config_text(
        text: &str,
        address: &mut Option<String>,
        client_timeout: &mut u64,
        log_file: &mut Option<String>,
        log_level: &mut u8,
        filters: &mut Vec<(String, Vec<FilterRule>)>
    ) {
        let mut lines = text.lines().peekable(); // 行ごとにパースするためイテレータ化
        while let Some(line) = lines.next() {
            let line = line.trim(); // 前後の空白除去
            // 空行やコメント行はスキップ
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // includeディレクトリ指定があれば、その中の*.confを再帰的に読み込む
            if let Some(rest) = line.strip_prefix("include ") {
                let dir_path = rest.trim(); // ディレクトリパス取得
                if let Ok(entries) = std::fs::read_dir(dir_path) {
                    for entry in entries.flatten() {
                        let path = entry.path(); // ファイルパス取得
                        if path.is_file() {
                            if let Some(ext) = path.extension() {
                                if ext == "conf" {
                                    // conf拡張子なら再帰的にパース
                                    if let Ok(sub_text) = std::fs::read_to_string(&path) {
                                        parse_config_text(&sub_text, address, client_timeout, log_file, log_level, filters);
                                    }
                                }
                            }
                        }
                    }
                }
                continue; // include行自体は他の設定とみなさない
            }
            // Listen設定（待受アドレス）
            if let Some(rest) = line.strip_prefix("Listen ") {
                let addr = rest.trim();
                if addr.contains(':') {
                    *address = Some(addr.to_string()); // host:port形式
                } else {
                    *address = Some(format!("[::]:{}", addr)); // ポートのみ指定時はIPv6全体
                }
            }
            // Client_timeout設定（クライアント無通信タイムアウト秒）
            else if let Some(rest) = line.strip_prefix("Client_timeout ") {
                if let Ok(val) = rest.trim().parse::<u64>() {
                    *client_timeout = val;
                }
            }
            // Log_file設定（ログファイルパス）
            else if let Some(rest) = line.strip_prefix("Log_file ") {
                let path = rest.trim();
                if !path.is_empty() {
                    *log_file = Some(path.to_string());
                }
            }
            // Log_level設定（info=0, trace=2, debug=8）
            else if let Some(rest) = line.strip_prefix("Log_level ") {
                let level = rest.trim().to_ascii_lowercase();
                *log_level = match level.as_str() {
                    "info" => 0,
                    "trace" => 2,
                    "debug" => 8,
                    _ => 0, // 未知値はinfo扱い
                };
            }
            // filter設定（フィルタールール）
            else if let Some(rest) = line.strip_prefix("filter[") {
            // filter[(name)] = キー:(!)正規表現:AND/OR、REJECT/DROP/WARN/ 複数行対応
            if let Some(end_idx) = rest.find(']') {
                let name = &rest[..end_idx]; // フィルター名取得
                let eq_idx = rest.find('='); // =の位置
                // まず1行目の=以降を取得
                let mut rule_str = if let Some(eq_idx) = eq_idx {
                    rest[eq_idx+1..].trim().to_string()
                } else {
                    String::new()
                };
                // 次の行が空行や新たな設定項目でなければ連結（複数行対応）
                while let Some(peek) = lines.peek() {
                    let peek_trim = peek.trim();
                    if peek_trim.is_empty() || peek_trim.starts_with("Listen ") || peek_trim.starts_with("Client_timeout ") || peek_trim.starts_with("filter[") {
                        break;
                    }
                    // コメント行はスキップして次の行へ
                    if peek_trim.starts_with('#') {
                        lines.next(); // コメント行を消費
                        continue;     // whileループの最初に戻る
                    }
                    // 連結時、カンマや空白で区切られている前提
                    rule_str.push(' ');
                    rule_str.push_str(peek_trim);
                    lines.next();
                }
                // 複数条件はカンマ区切りで分割
                let rule_list: Vec<&str> = rule_str.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
                let mut rules = Vec::new();
                // 各ルールをパース
                for rule in rule_list {
                    let parts: Vec<&str> = rule.split(':').collect();
                    // キー:パターン:アクション（OR/AND不要）
                    if parts.len() == 3 {
                        let key = parts[0].trim().trim_matches('"'); // キー
                        let (negate, pattern_raw) = if parts[1].starts_with('!') {
                            (true, &parts[1][1..]) // 否定パターン
                        } else {
                            (false, parts[1])
                        };
                        let pattern = pattern_raw.trim().trim_matches('"'); // 正規表現
                        if let Ok(regex) = Regex::new(pattern) {
                            let action = parts[2].trim().trim_matches('"'); // アクション
                            rules.push(FilterRule {
                                key: key.to_string(),
                                regex,
                                negate,
                                logic: "".to_string(),
                                action: action.to_string(),
                            });
                        }
                    }
                    // キー:パターン:ロジック:アクション（AND/ORあり）
                    else if parts.len() >= 4 {
                        let key = parts[0].trim().trim_matches('"');
                        let (negate, pattern_raw) = if parts[1].starts_with('!') {
                            (true, &parts[1][1..])
                        } else {
                            (false, parts[1])
                        };
                        let pattern = pattern_raw.trim().trim_matches('"');
                        if let Ok(regex) = Regex::new(pattern) {
                            let logic = parts[2].trim().trim_matches('"'); // AND/OR
                            let action = parts[3].trim().trim_matches('"'); // アクション
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
                // 最終アクションがREJECT/DROP/WARN/ACCEPTのいずれかかチェック
                let valid_actions = ["REJECT", "DROP", "WARN", "ACCEPT"];
                let is_valid = rules.last().map(|r| {
                    let act = r.action.to_ascii_uppercase();
                    valid_actions.contains(&act.as_str())
                }).unwrap_or(false);
                // 有効なフィルターのみ追加
                if is_valid {
                    filters.push((name.to_string(), rules));
                } else {
                    crate::printdaytimeln!(LOG_INFO, "[init] filter[{}] の最終アクションがREJECT/DROP/WARN/ACCEPT以外、または未指定のため無効化", name);
                }
            }
        }
        }
    }

    // 設定ファイル本体を読み込み
    let text = std::fs::read_to_string(path).expect("設定ファイル読み込み失敗");
    let mut address = None; // Listenアドレス初期値
    let mut client_timeout = 30u64; // タイムアウト初期値（秒）
    let mut log_file = None; // ログファイルパス初期値
    let mut log_level: u8 = 0; // ログレベル初期値（info）
    let mut filters: Vec<(String, Vec<FilterRule>)> = Vec::new(); // フィルター初期値
    parse_config_text(&text, &mut address, &mut client_timeout, &mut log_file, &mut log_level, &mut filters); // パース実行
    let address = address.unwrap_or_else(|| "[::]:8898".to_string()); // Listen未指定時のデフォルト
    Config {
        address,
        client_timeout,
        log_file,
        log_level,
        filters,
    }
}