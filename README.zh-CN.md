<p align="center">
  <img src="brassclaw.png?v=2" alt="BrassClaw" width="200"/>
</p>

<h1 align="center">BrassClaw</h1>

<p align="center">
  <strong>完全运行在你的硬件上的安全个人 AI 助手</strong>
</p>

<p align="center">
  <a href="https://github.com/chtugha/brassclaw/releases/latest"><img src="https://img.shields.io/github/v/release/chtugha/brassclaw?label=最新发布" alt="Latest Release" /></a>
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
  <a href="#设计理念">设计理念</a> •
  <a href="#功能特性">功能特性</a> •
  <a href="#快速开始">快速开始</a> •
  <a href="#配置">配置</a> •
  <a href="#架构">架构</a>
</p>

---

## 设计理念

BrassClaw 基于一个简单的原则：**你的 AI 助手应该为你服务。**

- **100% 本地运行** — 使用 vLLM、Ollama 或任何 OpenAI 兼容服务器；无需云账号
- **数据归你所有** — 所有状态存储在本地嵌入式 Postgres 中，加密保护，始终在你掌控之下
- **适配消费级硬件** — 7B 模型在 4GB 显存下运行良好
- **纵深防御** — 进程沙箱、能力租约、钩子框架、提示注入防御
- **完全开源** — 可完整审计，无遥测或数据收集
- **编排器优先** — Monty 编排器作为执行引擎，仅在真正需要时调用 LLM

---

## 功能特性

### 编排器优先引擎

- **Monty（Python 编排器）** 按顺序执行配方步骤并直接调用 Rust 工具
- **Tier-0 配方** — 零 LLM 调用的完全确定性执行路径
- **技能（Skills）** — Markdown 文件教导系统如何使用 API；无需编译 Rust
- **Sempai/Kohai 审查循环** — 自动编写新组件（配方、技能、ToolSkill）并加入验证队列

### 安全优先

- **进程沙箱** — 不受信任的工具子进程在有范围限制的文件系统和端点白名单下运行
- **钩子框架** — 能力调用和提示变更的 4 个信任层级（Builtin、Trusted、Installed、SelfAuthored）
- **能力租约** — 每次工具调用的细粒度可撤销权限授予
- **凭据保护** — 密钥永远不暴露给工具；在宿主边界注入并进行泄露检测
- **提示注入防御** — 模式检测、内容清理和策略执行
- **端点白名单** — HTTP 请求仅限于明确批准的主机和路径

### 随时可用

- **多渠道** — REPL、WebUI（React SPA，路径 `/v2`）、Slack、Telegram、HTTP Webhook、API 服务器
- **持久记忆** — 基于嵌入式 Postgres 的混合全文 + 向量搜索，采用倒数排名融合（RRF）
- **定时任务** — Cron 调度、事件触发器、Webhook 处理器，实现后台自动化
- **子代理** — 为复杂任务生成专门的子代理
- **MCP 协议** — 连接任意 Model Context Protocol 服务器

### 验证流水线

所有用户创建和代理创建的组件都经过两关验证流水线：

- **Q1** — 自动化编排沙箱检查
- **Q2** — 人工审核（仅操作员；永不自动化）

---

## 快速开始

**最低要求：** 约 8GB 内存，现代 64 位 CPU，约 4GB 可用磁盘空间。

### 选项 A：Linux 服务器 — 一行安装（推荐）

从 GitHub Releases 下载最新预构建二进制文件并注册为 systemd 服务：

```bash
curl -fsSL https://raw.githubusercontent.com/chtugha/brassclaw/main/install.sh | sudo bash
```

固定到指定版本：

```bash
sudo bash install.sh -v 0.9.1
```

**卸载：**

```bash
curl -fsSL https://raw.githubusercontent.com/chtugha/brassclaw/main/uninstall.sh | sudo bash
```

### 选项 B：macOS — 手动二进制安装

```bash
# Apple Silicon (M1/M2/M3):
curl -fsSL -o brassclaw-reborn https://github.com/chtugha/brassclaw/releases/latest/download/brassclaw-macos-arm64
chmod +x brassclaw-reborn && sudo mv brassclaw-reborn /usr/local/bin/brassclaw-reborn

# Intel Mac:
curl -fsSL -o brassclaw-reborn https://github.com/chtugha/brassclaw/releases/latest/download/brassclaw-macos-amd64
chmod +x brassclaw-reborn && sudo mv brassclaw-reborn /usr/local/bin/brassclaw-reborn
```

### 选项 C：从源码构建

需要 [Rust 1.94+](https://rustup.rs)。

```bash
git clone https://github.com/chtugha/brassclaw.git
cd brassclaw
cargo build --release --bin brassclaw
```

二进制文件位于 `target/release/brassclaw`。

---

## 配置

### 配置文件

主配置位于 `~/.brassclaw/reborn/config.toml`。首次运行时会创建示例文件。

```toml
[llm.default]
provider_id = "openai_compatible"
model = "Qwen/Qwen2.5-7B-Instruct-AWQ"
base_url = "http://localhost:8000/v1"

[identity]
tenant = "my-instance"
```

### 环境变量

| 变量 | 描述 |
|------|------|
| `BRASSCLAW_REBORN_HOME` | 数据目录（默认：`~/.brassclaw/reborn`） |
| `BRASSCLAW_RUNTIME_PROFILE` | 安全策略（默认：`local_dev`）。有效值：`local_dev`、`local_safe`、`local_yolo`、`hosted_safe` |
| `BRASSCLAW_PG_URL` | 外部 Postgres URL。省略时使用嵌入式 Postgres |
| `BRASSCLAW_REBORN_WEBUI_TOKEN` | WebUI 认证 Bearer 令牌 |
| `BRASSCLAW_REBORN_WEBUI_USER_ID` | 注入会话的用户标识 |

### LLM 提供商配置

**Ollama（家用推荐）：**

```bash
ollama serve
ollama pull qwen2.5:7b
```

```toml
[llm.default]
provider_id = "ollama"
model = "qwen2.5:7b"
```

**vLLM（GPU 服务器推荐）：**

```bash
vllm serve Qwen/Qwen2.5-7B-Instruct-AWQ --host 0.0.0.0 --port 8000
```

```toml
[llm.default]
provider_id = "openai_compatible"
model = "Qwen/Qwen2.5-7B-Instruct-AWQ"
base_url = "http://localhost:8000/v1"
```

---

## 架构

BrassClaw 使用三层模型，约 70 个 Rust crate：

- **产品层** — 拥有用户体验：CLI、WebUI（`/v2` 的 React SPA）、Slack、Telegram
- **循环层** — 拥有代理行为：Monty 编排器按顺序执行配方步骤，并按名称调用 Rust 工具
- **内核层** — 拥有权限：LLM 提供商抽象、沙箱子进程执行、凭据注入、安全策略执行

**组件目录**按类代码存储在 Postgres 中：

| 类 | 类型 | 描述 |
|----|------|------|
| 1–3 | Skill | 面向编排器的任务模式散文描述 |
| 13 | ToolSkill | 绑定描述符 — 参数模式、前置条件 |
| 21 | Recipe | 完整轮次脚本：RecipeVariant、意图示例、步骤链接 |
| 22 | PythonCode | 执行器 — 调用 `host.<tool>(...)` 分派 Rust |
| 23 | ExtensionCatalogue | 领域概览 |

---

## 许可证

以下任一许可证授权：

- Apache License, Version 2.0（[LICENSE-APACHE](LICENSE-APACHE)）
- MIT License（[LICENSE-MIT](LICENSE-MIT)）
