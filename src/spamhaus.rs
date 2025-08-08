// =========================
// spamhaus.rs
// Spamhaus API連携モジュール
//
// 【このファイルで使う主なクレート】
// - reqwest: HTTPSクライアント機能（POSTリクエスト送信、レスポンス処理）
// - serde_json: JSON形式データの構築（APIリクエストボディの生成）
// - chrono: 日時処理（UTC時刻のRFC3339形式フォーマット）
// - std::error: エラーハンドリング（Error traitのボックス化）
//
// 【役割】
// - スパム判定されたIPアドレスをSpamhaus APIに報告
// - APIトークンによる認証
// - レポートデータのJSON形式での送信
// - 非同期HTTPSリクエスト処理
// - レスポンス状態の確認とエラーハンドリング
// =========================

use crate::init::{Config, LOG_INFO};

/// SpamhausにスパムIPをレポートする関数
///
/// # 引数
/// - `ip_address`: レポートする送信元IPアドレス
/// - `evidence`: スパムの証拠（ヘッダ情報やフィルタールールなど）
/// - `config`: 設定情報（APIトークンとURLを含む）
///
/// # 戻り値
/// - `Result<(), Box<dyn std::error::Error>>`: 成功時は`Ok(())`、失敗時はエラー情報
///
/// # 説明
/// Spamhaus APIを使用して、スパム送信元IPアドレスを報告します。
/// API認証にはトークンを使用し、レポートにはIPアドレスと証拠情報を含めます。
/// レポート送信はHTTPSで行われ、非同期処理として実装されています。
pub async fn report_to_spamhaus(
    ip_address: &str, // 報告対象のスパム送信元IPアドレス
    evidence: &str,   // スパム判定の根拠情報（フィルタールール名など）
    config: &Config,  // APIトークンやURLを含む設定情報
) -> Result<(), Box<dyn std::error::Error>> {
    // --- フェーズ1: 設定値の取得と検証 ---
    // SpamhausのAPIトークンとURLが設定されているか確認（Option -> Result変換）
    let api_token = config
        .spamhaus_api_token
        .as_ref()
        .ok_or("Spamhaus API token not configured")?;
    let api_url = config
        .spamhaus_api_url
        .as_ref()
        .ok_or("Spamhaus API URL not configured")?;

    // IP アドレスの形式を検証
    let parsed_ip = ip_address
        .parse::<std::net::IpAddr>()
        .map_err(|_| format!("Invalid IP address format: {}", ip_address))?;

    // 設定されたSpamhaus_safe_addressリストに該当するIPアドレスは報告対象から除外
    for safe_addr in &config.spamhaus_safe_addresses {
        if is_ip_in_network(&parsed_ip, safe_addr)? {
            crate::printdaytimeln!(
                LOG_INFO,
                "[spamhaus] Safe IP address: {} (matches {})",
                ip_address,
                safe_addr
            );
            return Ok(()); // ホワイトリストに該当するため処理を終了
        }
    }

    // --- フェーズ2: リクエストデータの準備 ---
    // レポートデータをJSON形式で構築（serde_jsonマクロ使用）
    let report_data = serde_json::json!({
        "ip": ip_address,                         // 報告対象IPアドレス
        "evidence": evidence,                     // スパム判定の根拠情報
        "reporter": "MilterAgent",               // レポート送信元の識別子
        "timestamp": chrono::Utc::now().to_rfc3339(), // UTC現在時刻（RFC3339形式）
    });

    // --- フェーズ3: HTTPSクライアントの準備 ---
    // reqwestクライアントを初期化（SSL/TLS設定は自動）
    let client = reqwest::Client::new();

    // --- フェーズ4: POSTリクエストの送信 ---
    // Bearer認証トークンとJSONボディを設定してリクエスト送信
    let response = client
        .post(api_url) // 送信先URLを指定
        .header("Authorization", format!("Bearer {}", api_token)) // 認証ヘッダを設定
        .json(&report_data) // リクエストボディを設定
        .send() // 非同期でリクエスト送信
        .await?; // 送信完了まで待機

    // --- フェーズ5: レスポンス処理 ---
    if response.status().is_success() {
        // 成功時（2xx系ステータスコード）の処理
        crate::printdaytimeln!(
            LOG_INFO,
            "[spamhaus] Report sending succeeded: IP={}, Evidence={}",
            ip_address,
            evidence
        );
        Ok(()) // 正常終了を返す
    } else {
        // エラー時（4xx系、5xx系ステータスコード）の処理
        let error_msg = format!(
            "Spamhaus API error: {} - {}", // ステータスコードとエラーメッセージを結合
            response.status(),
            response.text().await? // レスポンスボディを文字列として取得
        );
        // エラー内容をログ出力
        crate::printdaytimeln!(LOG_INFO, "[spamhaus] Report sending failed: {}", error_msg);
        Err(error_msg.into()) // エラー情報を返す
    }
}

/// IPアドレスが指定されたネットワーク範囲に含まれるかをチェックする関数
///
/// # 引数
/// - `ip`: チェック対象のIPアドレス
/// - `network_str`: ネットワーク範囲の文字列（IP単体またはCIDR記法）
///
/// # 戻り値
/// - `Result<bool, Box<dyn std::error::Error>>`: 含まれる場合はtrue、それ以外はfalse
///
/// # サポート形式
/// - 単一IPアドレス: "192.168.1.1", "::1"
/// - CIDR記法: "192.168.1.0/24", "2001:db8::/32"
fn is_ip_in_network(
    ip: &std::net::IpAddr,
    network_str: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let network_str = network_str.trim();

    // CIDR記法かどうかをチェック
    if network_str.contains('/') {
        // CIDR記法の場合
        let parts: Vec<&str> = network_str.split('/').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid CIDR notation: {}", network_str).into());
        }

        let network_ip: std::net::IpAddr = parts[0]
            .parse()
            .map_err(|_| format!("Invalid network IP: {}", parts[0]))?;
        let prefix_len: u8 = parts[1]
            .parse()
            .map_err(|_| format!("Invalid prefix length: {}", parts[1]))?;

        match (ip, network_ip) {
            (std::net::IpAddr::V4(ip4), std::net::IpAddr::V4(net4)) => {
                if prefix_len > 32 {
                    return Err("IPv4 prefix length must be 0-32".into());
                }
                let mask = if prefix_len == 0 {
                    0
                } else {
                    !0u32 << (32 - prefix_len)
                };
                Ok((u32::from(*ip4) & mask) == (u32::from(net4) & mask))
            }
            (std::net::IpAddr::V6(ip6), std::net::IpAddr::V6(net6)) => {
                if prefix_len > 128 {
                    return Err("IPv6 prefix length must be 0-128".into());
                }
                let ip_bytes = ip6.octets();
                let net_bytes = net6.octets();

                let full_bytes = prefix_len / 8;
                let remaining_bits = prefix_len % 8;

                // 完全なバイト部分の比較
                for i in 0..full_bytes as usize {
                    if ip_bytes[i] != net_bytes[i] {
                        return Ok(false);
                    }
                }

                // 残りのビット部分の比較
                if remaining_bits > 0 {
                    let byte_index = full_bytes as usize;
                    if byte_index < 16 {
                        let mask = !0u8 << (8 - remaining_bits);
                        if (ip_bytes[byte_index] & mask) != (net_bytes[byte_index] & mask) {
                            return Ok(false);
                        }
                    }
                }

                Ok(true)
            }
            _ => Ok(false), // IPv4とIPv6の混在は不一致
        }
    } else {
        // 単一IPアドレスの場合
        if let Ok(single_ip) = network_str.parse::<std::net::IpAddr>() {
            // 単一IPアドレスとの完全一致
            Ok(*ip == single_ip)
        } else {
            Err(format!("Invalid network specification: {}", network_str).into())
        }
    }
}
