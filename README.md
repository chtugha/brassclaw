<p align="center">
  <img src="brassclaw.png?v=2" alt="BrassClaw" width="200"/>
</p>

<h1 align="center">BrassClaw</h1>

<p align="center">
  <strong>Your secure personal AI assistant — runs entirely on your hardware</strong>
</p>

<p align="center">
  <a href="#license"><img src="https://img.shields.io/badge/license-MIT%20OR%20Apache%202.0-blue.svg" alt="License: MIT OR Apache-2.0" /></a>
</p>

<p align="center">
  <a href="#philosophy">Philosophy</a> •
  <a href="#features">Features</a> •
  <a href="#quick-start">Quick Start</a> •
  <a href="#local-llm-setup">Local LLM Setup</a> •
  <a href="#configuration">Configuration</a> •
  <a href="#skills">Skills</a> •
  <a href="#architecture">Architecture</a>
</p>

---

## Philosophy

BrassClaw is built on a simple principle: **your AI assistant should work for you, not against you**.

- **100% local operation** — runs on your own hardware with vLLM, Ollama, or any OpenAI-compatible server; no cloud account required
- **Your data stays yours** — all information stored locally, encrypted, never leaves your control
- **Fits consumer hardware** — tuned to work within an 8,192-token context window; models as small as 7B work well
- **Defense in depth** — multiple security layers protect against prompt injection and data exfiltration
- **Open source** — fully auditable, no telemetry or data harvesting

---

## Features

### Home-Use Optimised

- **Token-aware engine** — hard budget of 8,192 total prompt tokens; automatically trims skill context, memory docs, and history to fit any local model
- **Knowledge-driven tools** — Skills (markdown files) teach the LLM how to use APIs via the `http` tool; no WASM compilation needed for new integrations
- **Skill budgets** — each skill declares its token cost; the selector fits within the 2,048-token skill budget
- **Local tools bundled** — browser (Playwright MCP), CalDAV calendar, plain-text notes, workspace file search included as Skills

### Security First

- **WASM Sandbox** — untrusted tools run in isolated WebAssembly containers with capability-based permissions
- **Credential Protection** — secrets never exposed to tools; injected at the host boundary with leak detection
- **Prompt Injection Defense** — pattern detection, content sanitisation, and policy enforcement
- **Endpoint Allowlisting** — HTTP requests only to explicitly approved hosts and paths

### Always Available

- **Multi-channel** — REPL, WebUI, HTTP webhooks, and API server
- **Routines** — cron schedules, event triggers, webhook handlers for background automation
- **Persistent memory** — hybrid full-text + vector search with Reciprocal Rank Fusion
- **Sub-agents** — spawn specialised child agents for complex tasks

### Self-Expanding

- **Knowledge-driven integrations** — add new API integrations by writing a SKILL.md file (no code required)
- **MCP Protocol** — connect to any Model Context Protocol server
- **Learning missions** — automatic skill extraction, repair, and self-improvement

---

## Quick Start

**Minimum requirements:** any machine with ~8 GB RAM, a modern 64-bit CPU, and about 4 GB of free disk space for models.

### Option A: DietPi / Linux Server with vLLM

```bash
git clone https://github.com/chtugha/brassclaw.git
cd brassclaw
sudo bash deploy/dietpi-setup.sh
```

This installs vLLM with Qwen2.5-7B-Instruct-AWQ and registers both services.

### Option B: Build from source

Requires [Rust 1.92+](https://rustup.rs).

```bash
git clone https://github.com/chtugha/brassclaw.git
cd brassclaw
cargo build --release -p brassclaw_reborn_cli --bin brassclaw-reborn
```

### Option C: Interactive REPL

```bash
export LLM_BACKEND=openai_compatible
export LLM_BASE_URL=http://localhost:8000/v1
export LLM_API_KEY=none
export LLM_MODEL=Qwen/Qwen2.5-7B-Instruct-AWQ

./target/release/brassclaw-reborn repl
```

---

## Local LLM Setup

### vLLM (recommended for GPU servers)

```bash
pip install vllm
vllm serve Qwen/Qwen2.5-7B-Instruct-AWQ --host 0.0.0.0 --port 8000 --max-model-len 8192
```

```env
LLM_BACKEND=openai_compatible
LLM_BASE_URL=http://localhost:8000/v1
LLM_API_KEY=none
LLM_MODEL=Qwen/Qwen2.5-7B-Instruct-AWQ
```

### Ollama (recommended for home use)

```bash
ollama serve
ollama pull qwen2.5:7b
```

```env
LLM_BACKEND=ollama
OLLAMA_MODEL=qwen2.5:7b
```

### Any OpenAI-compatible server (llama.cpp, LM Studio, etc.)

```env
LLM_BACKEND=openai_compatible
LLM_BASE_URL=http://localhost:1234/v1
LLM_API_KEY=none
LLM_MODEL=my-model-name
```

### Model Recommendations

| Model | Size | VRAM / RAM | Notes |
|-------|------|-----------|-------|
| `qwen2.5:7b` | 7B | 6 GB | Recommended minimum |
| `Qwen/Qwen2.5-7B-Instruct-AWQ` | 7B | 4 GB | AWQ quantized, best for vLLM |
| `qwen2.5:14b` | 14B | 10 GB | Best quality within 8192 tokens |
| `phi4` | 14B | 10 GB | Strong at coding and reasoning |
| `llama3.2` | 3B | 4 GB | Fast, good for simple tasks |

BrassClaw's default token budget is **8,192 total tokens** (including 2,048 for skill context). All models above fit comfortably.

---

## Configuration

### Reborn Config

Configure via `~/.brassclaw/reborn/config.toml`:

```toml
[llm.default]
provider_id = "openai_compatible"
model = "Qwen/Qwen2.5-7B-Instruct-AWQ"
api_key_env = "BRASSCLAW_VLLM_KEY"
```

Or use environment variables as fallback (see [Local LLM Setup](#local-llm-setup)).

### Profiles

| Profile | Best for | Database | Sandbox |
|---------|---------|---------|---------|
| `local` | Home use (default) | libSQL embedded | Disabled |
| `local-sandbox` | Home use + Docker isolation | libSQL embedded | Enabled |
| `server` | Single-user server | PostgreSQL | Enabled |
| `server-multitenant` | Multi-user server | PostgreSQL | Enabled |

### Token Budget

| Setting | Default | Description |
|---------|---------|-------------|
| `agent.max_prompt_tokens` | 8,192 | Total prompt token budget |
| `skills.max_context_tokens` | 2,048 | Skill injection budget |

The **Token Guard** automatically drops content in priority order when budget is exceeded:
1. Low-scoring memory docs
2. Low-scoring skills
3. Tool descriptions (truncated)
4. Droppable system-prompt sections
5. Old conversation history

---

## Skills

Skills are markdown files that teach the LLM how to perform tasks. They are injected into context only when relevant keywords are detected.

### Bundled Skills

| Skill | Budget | Description |
|-------|--------|-------------|
| `caldav` | 384 tokens | CalDAV calendar management via HTTP |
| `notes` | 192 tokens | Local note storage in ~/.brassclaw/notes.md |
| `local-search` | 256 tokens | Workspace file search |
| `web-browse` | 320 tokens | Playwright browser automation |
| `github` | 2000 tokens | GitHub API integration |
| `plan-mode` | 2500 tokens | Structured planning and execution |
| `coding` | varies | Code review and development |

### Creating a Skill

Create `skills/my-skill/SKILL.md`:

```yaml
---
name: my-skill
version: "1.0.0"
description: What this skill does
activation:
  keywords: ["keyword1", "keyword2"]
  max_context_tokens: 256
---

# My Skill

Instructions for the LLM on how to use this skill.
```

See [docs/brassclaw-architecture.md](docs/brassclaw-architecture.md) for the full skill system reference.

---

## Architecture

BrassClaw uses a two-layer architecture:

- **Rust kernel** (stable): LLM calls, tool execution, safety, persistence
- **Python orchestrator** (self-modifiable): Step loop, tool dispatch, output formatting

```
┌─────────────────────────────────────────────┐
│              brassclaw-reborn                │
│  ┌────────────┐  ┌────────────────────────┐ │
│  │    CLI      │  │     WebUI Server      │ │
│  └─────┬──────┘  └──────────┬─────────────┘ │
│        └─────────┬──────────┘               │
│          ┌───────▼──────────┐               │
│          │   Agent Loop     │               │
│          │ (8192 tok budget)│               │
│          └───────┬──────────┘               │
│     ┌────────────┼────────────┐             │
│  ┌──▼──┐   ┌────▼────┐  ┌───▼────┐         │
│  │ LLM │   │  Tools  │  │Skills  │         │
│  │vLLM │   │Built-in │  │CalDAV  │         │
│  │Ollama│  │MCP/WASM │  │Notes   │         │
│  └──────┘  └─────────┘  │Search  │         │
│                          │Browse  │         │
│                          └────────┘         │
└─────────────────────────────────────────────┘
```

See [docs/brassclaw-architecture.md](docs/brassclaw-architecture.md) for the complete architecture guide.

---

## Heritage

BrassClaw is a fork of [IronClaw](https://github.com/nearai/ironclaw), optimised for local-first, privacy-respecting operation on consumer hardware. Key differences:

- **8,192-token context budget** (vs 128K) for small local LLMs
- **vLLM-first deployment** with AWQ quantised models
- **Knowledge-driven tools** via Skills instead of WASM-only extensions
- **DietPi deployment scripts** for headless server operation

---

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
