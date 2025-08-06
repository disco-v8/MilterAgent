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

- Rust 1.70以降
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
