# MilterAgent

正規企業・組織を騙るフィッシングメールから保護するために設計された、Milterプロトコルを実装した高性能メールフィルタリングサーバーです。

## 概要

MilterAgentは、リアルタイムメールフィルタリングのためのMilterプロトコルを実装した包括的なメールセキュリティソリューションです。RustとTokioで構築され、低レイテンシと高スループットを維持しながら、高度なフィッシング検出機能を提供します。

## 特徴

- **高度なフィッシング検出**: 以下を騙るフィッシングメールを検出する包括的なフィルター:
  - 銀行・金融機関（三菱UFJ、みずほ、三井住友など）
  - 配送・物流会社（日本郵便、ヤマト運輸、佐川急便、DHLなど）
  - 交通機関（JR各社、航空会社、私鉄など）
  - ECプラットフォーム・オンラインサービス
- **Spamhaus API連携**: スパム送信者の自動報告機能:
  - フィッシングメール送信元IPの自動検出と報告
  - Spamhaus APIとの統合による脅威インテリジェンス共有
  - 設定可能なAPI認証とエンドポイント
  - 信頼できるネットワーク用のIPホワイトリスト（CIDR記法対応）
- **高度な正規表現サポート**: fancy-regexによる強力な機能:
  - 否定先読み（`(?!...)`）および肯定先読み（`(?=...)`）
  - 否定後読み（`(?<!...)`）および肯定後読み（`(?<=...)`）
  - 高度なメールフィルタリングのための複雑なパターンマッチング
- **Milterプロトコルサポート**: シームレスな統合のための完全なMilterプロトコル実装
- **リアルタイム処理**: 数千の同時接続を処理する非同期Rust実装
- **高速並列フィルタリング**: マルチスレッド処理によるフィルター評価の並列実行で高速処理を実現
- **柔軟な設定**: includeサポートを備えたモジュラー設定システム
- **シグナル処理**: SIGHUP/SIGTERMによる安全な終了とライブ設定リロード
- **クロスプラットフォーム**: UnixとWindowsの両環境をサポート
- **包括的なログ**: JSTタイムスタンプサポート付きの設定可能なログレベル

## アーキテクチャ

- **非同期ランタイム**: 複数の同時接続を処理するためのTokio構築
- **モジュラー設計**: パース、フィルタリング、プロトコル処理の独立モジュール
- **メモリセーフ**: 保証されたメモリ安全性とパフォーマンスのためのRust記述
- **並列フィルターエンジン**: 正規表現サポート付きの設定可能なルールベースフィルタリングシステム、マルチスレッドによる並列処理で高いスループットを実現

## インストール

### 前提条件

- Rust 1.80以降（Rust 2024 editionサポート付き）
- Cargoパッケージマネージャー
- 対応メールサーバー（Postfix、Sendmailなど）

### ソースからのビルド

```bash
cd /usr/src/
git clone https://github.com/disco-v8/MilterAgent.git
mv MilterAgent "MilterAgent-$(date +%Y%m%d)"
cd /usr/src/"MilterAgent-$(date +%Y%m%d)"

cargo build --release

/bin/cp -af ./target/release/milter_agent /usr/sbin/
rsync -av ./logrotate.d/milter_agent /etc/logrotate.d/
rsync -av ./systemd/milter_agent.service /usr/lib/systemd/system/
rsync -av ./etc/ /etc/
```

## 設定

サーバーはモジュラー設定システムを使用します：

### メイン設定ファイル

`MilterAgent.conf`を作成：

```
Listen 127.0.0.1:8898
Client_timeout 30
Log_file /var/log/milteragent.log
Log_level info

include MilterAgent.d
```

### フィルター設定

`MilterAgent.d/`ディレクトリにフィルターファイルを配置：

- `filter_bank.conf` - 銀行・金融サービスフィルター
- `filter_transport.conf` - 交通・物流フィルター
- 必要に応じて追加のフィルターファイル

REJECT（拒否）ではなくWARN（ヘッダー追加のみ）で開始したい場合は、以下の一括置換を実行してください。

```
chmod 700 /etc/MilterAgent.d/reject2warn.sh
cd /etc/MilterAgent.d/
./reject2warn.sh
```

### フィルター構文

```
filter[フィルター名] = 
    "decode_from:(?i)(パターン):AND",
    "decode_from:!@(.*\.)?正規ドメイン\.com:REJECT"
```

### 設定オプション

- `Listen`: サーバーバインドアドレスとポート
  - 形式: `IP:PORT`または`PORT`のみ
  - 例: `127.0.0.1:8898`または`8898`
- `Client_timeout`: クライアント非アクティブタイムアウト（秒）
- `Log_file`: ログファイルパス（オプション、デフォルトは標準出力）
- `Log_level`: ログ詳細度（`info`、`trace`、`debug`）
- `Spamhaus_report`: Spamhaus APIへの報告を有効にする（`yes`/`no`、デフォルト: `no`）
- `Spamhaus_api_token`: Spamhaus API認証トークン（オプション）
- `Spamhaus_api_url`: Spamhaus APIエンドポイントURL（オプション）
- `Spamhaus_safe_address`: Spamhaus報告から除外するIPアドレスまたはCIDRネットワーク（オプション、複数指定可能）
- `include`: 追加設定ディレクトリをインクルード

## 使用方法

### サーバー起動

```bash
systemctl daemon-reload
systemctl --no-pager status milter_agent

systemctl enable milter_agent
systemctl --no-pager status milter_agent

systemctl start milter_agent
systemctl --no-pager status milter_agent
```

### メールサーバー統合

#### Postfix統合

`/etc/postfix/main.cf`に追加：

```
smtpd_milters = inet:127.0.0.1:8898
milter_default_action = accept
milter_protocol = 6
```

Postfixを再起動：

```bash
sudo systemctl restart postfix
```

#### Sendmail統合

`/etc/mail/sendmail.mc`に追加：

```
INPUT_MAIL_FILTER(`milteragent', `S=inet:8898@127.0.0.1')
```

### シグナル処理

- **SIGHUP**: 設定ファイルのリロード
- **SIGTERM**: 安全な終了
- **Ctrl+C**（Windows）: 安全な終了

```bash
# 設定リロード
systemctl reload milter_agent
systemctl --no-pager status milter_agent

# 安全な終了
systemctl stop milter_agent
systemctl --no-pager status milter_agent
```

## ログ

サーバーは設定可能なログレベルをサポート：

- `info`（0）: 基本的な動作メッセージ
- `trace`（2）: 詳細なトレース情報
- `debug`（8）: 包括的なデバッグ出力

ログはJSTタイムスタンプを含み、設定に基づいてファイルまたは標準出力に出力されます。

## フィルター例

### 銀行フィルター

```
filter[phish_mufg] = 
    "decode_from:(?i)(三菱UFJ|MUFG):AND",
    "decode_from:!@(.*\.)?mufg\.jp:REJECT"
```

### 交通機関フィルター

```
filter[phish_jreast] = 
    "decode_from:(?i)(JR東日本|East Japan Railway):AND",
    "decode_from:!(@(.*\.)?jreast\.co\.jp|@(.*\.)?jre-vts\.com):REJECT"
```

### 高度な否定先読みフィルター

```
filter[phish_monex_html] = 
    "body:(?i)monex.*(?!.*\.?(monex\.co\.jp|monex\.com|on-compass\.com)\b):REJECT"
```

このフィルターは「monex」を含むが正規のMonexドメインからではないメールを検出し、金融サービスを騙るフィッシング攻撃を防ぎながら、正当な通信は許可します。

### 注意事項: `fancy-regex`における否定先読みの再帰深度制限

`fancy-regex`を使用したセマンティックフィルタリングにおいて、3～4個以上の `|` 分岐を含む否定先読み `(?!...)` は内部の再帰深度を超える可能性があることに注意してください。

これにより、特に複数のドメイン除外を含むURL フィルタリングで、部分マッチや不正確な評価が発生する場合があります。

**推奨される回避策:**
- 複雑な先読みを複数のフィルタールールに分割する
- `(?!...)` 内の深いネストした選択肢を避ける
- 先読みごとの分岐数を少なくしたシンプルなパターンを使用する

例:

```dsl
# これは避ける（失敗する可能性）:
decode_html:https?://(?!.*\.(a\.com|b\.com|c\.com|d\.com|e\.com)\b).+:REJECT

# これを推奨（安定した評価）:
decode_html:https?://(?!.*\.(a\.com|b\.com|c\.com)\b).+:AND
decode_html:https?://(?!.*\.(d\.com|e\.com)\b).+:REJECT
```

## 依存関係

- [tokio](https://tokio.rs/): 非同期ランタイム
- [mail-parser](https://crates.io/crates/mail-parser): MIMEメール解析
- [fancy-regex](https://crates.io/crates/fancy-regex): 先読み・後読み対応の高機能正規表現エンジン
- [chrono](https://crates.io/crates/chrono): 日時処理
- [chrono-tz](https://crates.io/crates/chrono-tz): タイムゾーンサポート
- [reqwest](https://crates.io/crates/reqwest): API統合用HTTPクライアント
- [serde](https://crates.io/crates/serde): シリアライゼーションフレームワーク

## 変更履歴

### v0.2.1 (2025-08-13)
- フィルター定義の読み込みロジックを刷新し、より柔軟かつ堅牢なパース処理に変更
- フィルター定義でURL記述時のコロン（:）の扱いを修正し、エスケープ不要で記述可能に

### v0.2.0 (2025-08-08)
- **メジャーアップデート: Rust 2024 Editionサポート**
  - Rust 2021から2024 editionにアップグレード
  - パフォーマンス向上と最新言語機能の活用
  - 標準ライブラリ統合の強化

- **高度な正規表現エンジンのアップグレード**
  - 標準の`regex`から`fancy-regex` 0.16に移行
  - 否定先読み（`(?!...)`）および肯定先読み（`(?=...)`）のサポート追加
  - 否定後読み（`(?<!...)`）および肯定後読み（`(?<=...)`）のサポート追加
  - 高度なフィッシング検出パターンを実現

- **Spamhaus API連携機能の追加**
  - フィッシングメール送信元IPの自動検出と報告
  - 設定可能なSpamhaus API認証とエンドポイント
  - スパム判定されたメールの脅威インテリジェンス共有
  - `Spamhaus_report`、`Spamhaus_api_token`、`Spamhaus_api_url`設定オプション追加
  - IPホワイトリスト設定用の`Spamhaus_safe_address`を追加
  - ネットワーク範囲除外のためのCIDR記法をサポート

- **フィルターエンジンの改善**
  - 複雑な否定先読みでのfancy-regex再帰深度問題を修正
  - 再帰制限を回避するため複雑なORパターンを分割（3-4分岐超）
  - w3.orgおよびドメイン除外フィルターのセマンティック精度を向上
  - フィルター信頼性とパターンマッチング安定性を強化

- **依存関係の最適化**
  - 未使用の`lazy_static`依存関係を削除
  - `once_cell`を標準ライブラリの`std::sync::OnceLock`に置換
  - 外部依存関係を削減し、コンパイル時間を改善
  - 全依存関係を最新の互換バージョンに更新

- **コード品質の向上**
  - 包括的なコードドキュメントとコメントの追加
  - 正規表現マッチングでのエラーハンドリング改善
  - コードの可読性と保守性の向上
  - `cargo fmt`による一貫したフォーマット適用
  - 全ての`cargo clippy`警告を解決

- **設定システムの強化**
  - 無効なフィルター設定に対するより良いエラーメッセージ
  - 正規表現コンパイルエラー報告の改善
  - 設定ファイル解析の堅牢性向上

### v0.1.0
- 初期リリース
- 基本的なMilterプロトコル実装
- MIMEメール解析と出力
- 設定ファイルサポート
- シグナル処理
- JSTタイムスタンプログ

## 貢献

1. リポジトリをフォーク
2. フィーチャーブランチを作成（`git checkout -b feature/amazing-feature`）
3. 変更をコミット（`git commit -m 'Add amazing feature'`）
4. ブランチにプッシュ（`git push origin feature/amazing-feature`）
5. プルリクエストを開く

## ライセンス

このプロジェクトはMITライセンスの下でライセンスされています - 詳細は[LICENSE](LICENSE)ファイルをご覧ください。

## セキュリティに関する注意

このソフトウェアはフィッシングメールからの保護を支援するように設計されていますが、包括的なメールセキュリティ戦略の一部として使用されるべきです。新しいフィッシングキャンペーンに対応するため、フィルタールールの定期的な更新が推奨されます。

## サポート

問題、質問、または貢献については、GitHubイシュートラッカーをご利用ください。
