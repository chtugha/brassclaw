<p align="center">
  <img src="brassclaw.png?v=2" alt="BrassClaw" width="200"/>
</p>

<h1 align="center">BrassClaw</h1>

<p align="center">
  <strong>あなたのハードウェアで完全に動作する、安全なパーソナルAIアシスタント</strong>
</p>

<p align="center">
  <a href="https://github.com/chtugha/brassclaw/releases/latest"><img src="https://img.shields.io/github/v/release/chtugha/brassclaw?label=最新リリース" alt="Latest Release" /></a>
  <a href="#license"><img src="https://img.shields.io/badge/license-MIT%20OR%20Apache%202.0-blue.svg" alt="License: MIT OR Apache-2.0" /></a>
</p>

<p align="center">
  <a href="README.md">English</a> |
  <a href="README.zh-CN.md">简体中文</a> |
  <a href="README.ru.md">Русский</a> |
  <a href="README.ja.md">日本語</a> |
  <a href="README.ko.md">한국어</a>
</p>

<p align="center">
  <a href="#フィロソフィー">フィロソフィー</a> •
  <a href="#機能">機能</a> •
  <a href="#クイックスタート">クイックスタート</a> •
  <a href="#設定">設定</a> •
  <a href="#アーキテクチャ">アーキテクチャ</a>
</p>

---

## フィロソフィー

BrassClawはシンプルな原則に基づいて構築されています：**あなたのAIアシスタントは、あなたのために働くべきです。**

- **100%ローカル動作** — vLLM、Ollama、またはOpenAI互換サーバーで動作。クラウドアカウント不要
- **データはあなたのもの** — すべてのデータはローカルの組み込みPostgresに暗号化されて保存されます
- **コンシューマーハードウェア対応** — 7Bモデルは4GB VRAMで良好に動作
- **多層防御** — プロセスサンドボックス、ケイパビリティリース、フックフレームワーク、プロンプトインジェクション対策
- **オープンソース** — 完全に監査可能。テレメトリやデータ収集なし
- **オーケストレーター優先** — MontyオーケストレーターがLLMを最小限に抑えて実行エンジンとして機能

---

## 機能

### オーケストレーター優先エンジン

- **Monty（Pythonオーケストレーター）** がレシピのステップを順番に実行し、Rustツールを直接呼び出す
- **Tier-0レシピ** — LLM呼び出しなしの完全に決定論的な実行パス
- **スキル** — マークダウンファイルがAPIの使用方法をシステムに教える。Rustのコンパイル不要
- **Sempai/Kohaiレビューループ** — 新しいコンポーネント（レシピ、スキル、ToolSkill）を自動的に作成し、検証キューに入れる

### セキュリティファースト

- **プロセスサンドボックス** — 信頼されていないツールのサブプロセスはスコープ付きファイルシステムとエンドポイント許可リストで実行
- **フックフレームワーク** — ケイパビリティ呼び出しとプロンプト変更に対する4つの信頼ティア（Builtin、Trusted、Installed、SelfAuthored）
- **ケイパビリティリース** — すべてのツール呼び出しに対する細粒度で取り消し可能な権限付与
- **認証情報の保護** — シークレットはツールに公開されない。ホスト境界でリーク検出付きで注入
- **プロンプトインジェクション防御** — パターン検出、コンテンツのサニタイズ、ポリシー適用
- **エンドポイント許可リスト** — HTTPリクエストは明示的に承認されたホストとパスのみに制限

### 常時利用可能

- **マルチチャネル** — REPL、WebUI（React SPAは`/v2`）、Slack、Telegram、HTTPウェブフック、APIサーバー
- **永続メモリ** — 組み込みPostgresによるReciprocal Rank Fusionを使用したハイブリッドのフルテキスト＋ベクター検索
- **ルーティン** — バックグラウンド自動化のためのcronスケジュール、イベントトリガー、ウェブフックハンドラー
- **サブエージェント** — 複雑なタスクのための専門的な子エージェントのスポーン
- **MCPプロトコル** — あらゆるModel Context Protocolサーバーへの接続

### 検証パイプライン

すべてのユーザー作成およびエージェント作成コンポーネントは2ゲートの検証パイプラインを通過します：

- **Q1** — 自動化されたオーケストレーテッドサンドボックスチェック
- **Q2** — 人間によるレビュー（オペレーターのみ、自動化不可）

---

## クイックスタート

**最小要件：** 約8GBのRAM、最新の64ビットCPU、約4GBの空きディスクスペース。

### オプションA：Linuxサーバー — ワンライン インストール（推奨）

GitHubリリースから最新のビルド済みバイナリをダウンロードし、systemdサービスとして登録します：

```bash
curl -fsSL https://raw.githubusercontent.com/chtugha/brassclaw/main/install.sh | sudo bash
```

バージョンを指定してインストール：

```bash
sudo bash install.sh -v 0.9.1
```

**アンインストール：**

```bash
curl -fsSL https://raw.githubusercontent.com/chtugha/brassclaw/main/uninstall.sh | sudo bash
```

### オプションB：macOS — 手動バイナリインストール

```bash
# Apple Silicon (M1/M2/M3):
curl -fsSL -o brassclaw-reborn https://github.com/chtugha/brassclaw/releases/latest/download/brassclaw-macos-arm64
chmod +x brassclaw-reborn && sudo mv brassclaw-reborn /usr/local/bin/brassclaw-reborn

# Intel Mac:
curl -fsSL -o brassclaw-reborn https://github.com/chtugha/brassclaw/releases/latest/download/brassclaw-macos-amd64
chmod +x brassclaw-reborn && sudo mv brassclaw-reborn /usr/local/bin/brassclaw-reborn
```

### オプションC：ソースからビルド

[Rust 1.94+](https://rustup.rs)が必要です。

```bash
git clone https://github.com/chtugha/brassclaw.git
cd brassclaw
cargo build --release --bin brassclaw
```

バイナリは `target/release/brassclaw` にあります。

---

## 設定

### 設定ファイル

設定は `~/.brassclaw/reborn/config.toml` にあります。初回起動時に例が作成されます。

```toml
[llm.default]
provider_id = "openai_compatible"
model = "Qwen/Qwen2.5-7B-Instruct-AWQ"
base_url = "http://localhost:8000/v1"

[identity]
tenant = "my-instance"
```

### 環境変数

| 変数 | 説明 |
|------|------|
| `BRASSCLAW_REBORN_HOME` | データディレクトリ（デフォルト: `~/.brassclaw/reborn`） |
| `BRASSCLAW_RUNTIME_PROFILE` | セキュリティポリシー（デフォルト: `local_dev`）。有効な値: `local_dev`、`local_safe`、`local_yolo`、`hosted_safe` |
| `BRASSCLAW_PG_URL` | 外部PostgresのURL。省略時は組み込みPostgresを使用 |
| `BRASSCLAW_REBORN_WEBUI_TOKEN` | WebUI認証用ベアラートークン |
| `BRASSCLAW_REBORN_WEBUI_USER_ID` | セッションに注入されるユーザーID |

### LLMプロバイダーの設定

**Ollama（ホームユース向け推奨）：**

```bash
ollama serve
ollama pull qwen2.5:7b
```

`config.toml`:
```toml
[llm.default]
provider_id = "ollama"
model = "qwen2.5:7b"
```

**vLLM（GPUサーバー向け推奨）：**

```bash
vllm serve Qwen/Qwen2.5-7B-Instruct-AWQ --host 0.0.0.0 --port 8000
```

`config.toml`:
```toml
[llm.default]
provider_id = "openai_compatible"
model = "Qwen/Qwen2.5-7B-Instruct-AWQ"
base_url = "http://localhost:8000/v1"
```

---

## アーキテクチャ

BrassClawは3層モデルを使用しています：

- **製品層** — UXとサーフェス。CLI、WebUI（`/v2`のReact SPA）、Slack、Telegram
- **ループ層** — エージェントの動作。Montyオーケストレーターがレシピのステップを順番に実行し、LLMを呼び出し、Rustツールを名前で呼び出す
- **カーネル層** — 権限の所有。LLMプロバイダーの抽象化、サンドボックス化されたサブプロセス実行、認証情報の注入、セキュリティポリシーの適用

**コンポーネントカタログ**はPostgresにクラスコードで格納されます：

| クラス | タイプ | 説明 |
|--------|--------|------|
| 1–3 | スキル | タスクパターンを説明するオーケストレーター向けの散文 |
| 13 | ToolSkill | バインディング記述子 — パラメータスキーマ、前提条件 |
| 21 | レシピ | 完全なターンスクリプト：RecipeVariant、インテントサンプル、ステップリンク |
| 22 | PythonCode | エグゼキューター — `host.<tool>(...)` を呼び出してRustをディスパッチ |
| 23 | ExtensionCatalogue | ドメインの概要 |

---

## ライセンス

以下のいずれかのライセンスの下でライセンスされています：

- Apache License, Version 2.0（[LICENSE-APACHE](LICENSE-APACHE)）
- MIT License（[LICENSE-MIT](LICENSE-MIT)）
