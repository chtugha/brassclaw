# BrassClaw Architecture Guide

This document provides a comprehensive guide to every component of the BrassClaw system, designed for any future agent or developer to deeply understand the design.

## Overview

BrassClaw is a secure, local-first AI assistant built on the IronClaw Reborn architecture. It runs entirely on consumer hardware, optimized for small LLMs (7B-14B parameters) within an 8,192-token context window.

The system uses a **two-layer architecture**: a stable Rust kernel providing infrastructure (LLM calls, tool execution, safety, persistence) and a self-modifiable Python orchestrator (via Monty VM) providing execution logic.

## Architecture Diagram

```
                        ┌─────────────────────────────────┐
                        │       brassclaw-reborn CLI       │
                        │  (crates/brassclaw_reborn_cli)   │
                        └──────────────┬──────────────────┘
                                       │
                    ┌──────────────────▼──────────────────┐
                    │          Reborn Runtime              │
                    │    (crates/brassclaw_reborn)         │
                    │  ┌─────────┐  ┌──────────────────┐  │
                    │  │ Drivers │  │    Turn Runner    │  │
                    │  └────┬────┘  └────────┬─────────┘  │
                    └───────┼────────────────┼────────────┘
                            │                │
            ┌───────────────▼────────────────▼───────────┐
            │            Agent Loop                       │
            │     (crates/brassclaw_agent_loop)           │
            │  ┌────────┐ ┌──────────┐ ┌──────────────┐  │
            │  │Executor│ │ Planner  │ │  Strategies   │  │
            │  │        │ │          │ │ - Compaction  │  │
            │  │        │ │          │ │ - Budget      │  │
            │  │        │ │          │ │ - Capability  │  │
            │  └───┬────┘ └──────────┘ └──────────────┘  │
            └──────┼──────────────────────────────────────┘
                   │
    ┌──────────────▼──────────────────────┐
    │          Host Runtime               │
    │  (crates/brassclaw_host_runtime)    │
    │  ┌──────┐ ┌──────┐ ┌────────────┐  │
    │  │ LLM  │ │Tools │ │  Safety    │  │
    │  │Bridge│ │Bridge│ │  Layer     │  │
    │  └──┬───┘ └──┬───┘ └────────────┘  │
    └─────┼────────┼──────────────────────┘
          │        │
    ┌─────▼──┐ ┌───▼────────────────────────┐
    │  LLM   │ │     Tool Registry           │
    │Provider│ │  ┌────────┐ ┌───────────┐   │
    │        │ │  │Built-in│ │WASM Tools │   │
    │ vLLM   │ │  │ Tools  │ │(sandboxed)│   │
    │ Ollama │ │  └────────┘ └───────────┘   │
    │ OpenAI │ │  ┌────────┐ ┌───────────┐   │
    │  etc.  │ │  │  MCP   │ │   Skills  │   │
    │        │ │  │Servers │ │(knowledge)│   │
    └────────┘ │  └────────┘ └───────────┘   │
               └─────────────────────────────┘
```

## Core Crates

### brassclaw_reborn_cli
The main binary entry point. Provides CLI commands:
- `repl` - Interactive REPL session
- `run --message "..."` - Single-shot execution
- `serve` - WebUI + API server
- `config init/path` - Configuration management
- `models set-provider/status/list` - LLM provider management
- `doctor` - System diagnostics

### brassclaw_reborn
Owns the driver-side Reborn loop integration:
- **PlannedDriver**: Adapts agent loop families to the runner-facing contract
- **TextLoopDriver**: Legacy text-only driver
- **DriverRegistry**: Driver registration and readiness
- **LoopDriverHost**: Composes concrete loop host ports
- **LoopExitApplier**: Validates and applies loop exits
- **TurnRunner**: Manages individual conversation turns

### brassclaw_agent_loop
The core execution loop with pluggable strategies:
- **Executor**: Runs the main agent loop (LLM call -> tool dispatch -> repeat)
- **DefaultPlanner**: Plans execution with configurable strategies
- **Strategies**:
  - `CompactionStrategy` - Context window management (default: 8192 tokens)
  - `BudgetStrategy` - Iteration and time limits
  - `CapabilityStrategy` - Tool/action availability
  - `ModelStrategy` - Model selection and fallback
  - `StopConditionStrategy` - Loop termination detection
  - `RecoveryStrategy` - Error recovery

### brassclaw_engine
The v2 engine with Python orchestrator:
- **ExecutionLoop**: Bootstraps Monty VM, loads orchestrator, runs step loop
- **Orchestrator** (`orchestrator/default.py`): Python execution loop
- **Host Functions**: Rust functions callable from Python (`__llm_complete__`, `__execute_action__`, etc.)
- **SkillSelector**: Deterministic skill scoring and injection
- **SkillTracker**: Confidence tracking with rollback
- **ThreadManager**: Spawn, stop, join threads
- **MissionManager**: Learning missions lifecycle

### brassclaw_turns
Runner and host contracts:
- **RunProfile**: Configuration for a loop execution
- **LoopRunContext**: Runtime context passed to strategies
- **Turn contracts**: Input/output types for conversation turns

### brassclaw_host_runtime
Bridges the agent loop to concrete infrastructure:
- **LLM adapters**: Connect to vLLM, Ollama, OpenAI, Anthropic, etc.
- **Effect adapters**: Tool execution with safety controls
- **Store adapters**: Persistence layer

### brassclaw_llm
LLM provider abstractions and implementations:
- Multi-provider support via `rig-core`
- OpenAI-compatible API support (for vLLM, llama.cpp, etc.)
- Ollama native support
- Token counting and budget management
- Streaming response handling

### brassclaw_skills
Shared skills system (used by both v1 and v2 engines):
- **types.rs**: SkillManifest, ActivationCriteria, LoadedSkill
- **selector.rs**: Deterministic 4-phase selection pipeline
- **parser.rs**: SKILL.md frontmatter parsing
- **validation.rs**: Name/content escaping
- **gating.rs**: Binary/env/config requirements checking

### brassclaw_safety
Security and safety layer:
- Prompt injection detection (pattern-based)
- Content sanitization and escaping
- Policy enforcement (Block/Warn/Review/Sanitize)
- Tool output wrapping

### brassclaw_wasm
WebAssembly sandbox for untrusted tools:
- Wasmtime-based execution with capability leases
- Endpoint allowlisting
- Credential injection at host boundary
- Leak detection (request/response scanning)
- Rate limiting per tool

### brassclaw_extensions
Extension discovery and management:
- Manifest parsing (TOML/JSON)
- Runtime kind detection (WASM, MCP, Script, FirstParty, System)
- Extension lifecycle management

### brassclaw_gateway
Web gateway for browser UI:
- SSE/WebSocket streaming
- Chat API endpoints
- Memory/jobs/extensions/routines management

### brassclaw_tui
Terminal user interface:
- Rich text rendering
- Interactive REPL
- Status display

## Skills System

Skills are markdown files with YAML frontmatter that provide knowledge injection into the LLM context. They are the v2 replacement for both WASM API wrapper tools and static prompt extensions.

### How Skills Work

1. **Activation**: When a user message arrives, the SkillSelector scores all available skills against the message content using keywords, patterns, and tags
2. **Selection**: Skills above a threshold score are selected, subject to the token budget (default: 2048 tokens for all skills combined)
3. **Injection**: Selected skill content is injected into the system prompt as `<skill>` XML blocks
4. **Execution**: The LLM reads the skill content and uses the knowledge to construct tool calls (e.g., HTTP requests)

### Deterministic Selection Pipeline

No LLM involvement in skill selection (prevents circular manipulation):

1. **Gating**: Check prerequisites (binary/env/config requirements)
2. **Scoring**: Keyword exact (10pts, cap 30) + substring (5pts) + tag (3pts, cap 15) + regex pattern (20pts, cap 40)
3. **Budget**: Fit within `max_context_tokens` budget
4. **Attenuation**: Trust-based confidence factor

### Skill File Format

```yaml
---
name: skill-name
version: "1.0.0"
description: What this skill does
activation:
  keywords: ["keyword1", "keyword2"]
  exclude_keywords: ["not-this"]
  patterns: ["(?i)regex.*pattern"]
  tags: ["tag1", "tag2"]
  max_context_tokens: 256
credentials:
  - name: api_token
    provider: service_name
    location:
      type: bearer
    hosts:
      - "api.example.com"
---

# Skill Content (Markdown)

Instructions for the LLM on how to use this skill's capabilities.
```

### Token Budgets

BrassClaw enforces strict token budgets for local LLM compatibility:

| Setting | Default | Description |
|---------|---------|-------------|
| `agent.max_prompt_tokens` | 8,192 | Total prompt token budget |
| `skills.max_context_tokens` | 2,048 | Skill injection budget |
| Compaction context limit | 8,192 | Agent loop context window |
| Compaction reserve | 2,048 | Reserved for new output |
| Compaction preserve tail | 1,024 | Recent messages to keep |

## Configuration

### Reborn Config (`~/.brassclaw/reborn/config.toml`)

```toml
[llm.default]
provider_id = "openai_compatible"
model = "Qwen/Qwen2.5-7B-Instruct-AWQ"
api_key_env = "BRASSCLAW_VLLM_KEY"

[boot]
profile = "local-dev"

[webui]
listen_host = "127.0.0.1"
listen_port = 3000
```

### Profiles

| Profile | Database | Sandbox | Use Case |
|---------|----------|---------|----------|
| `local` | libSQL | Disabled | Home use |
| `local-sandbox` | libSQL | Docker | Home + isolation |
| `server` | PostgreSQL | Docker | Single-user server |
| `server-multitenant` | PostgreSQL | Docker | Multi-user |

### Environment Variables

| Variable | Purpose |
|----------|---------|
| `BRASSCLAW_REBORN_HOME` | State root (default: `~/.brassclaw/reborn`) |
| `BRASSCLAW_REBORN_PROFILE` | Boot profile (local-dev, production) |
| `BRASSCLAW_REBORN_LOG` | Tracing filter |
| `LLM_BACKEND` | LLM provider fallback |
| `LLM_BASE_URL` | OpenAI-compatible endpoint |
| `LLM_MODEL` | Model name |
| `LLM_API_KEY` | API key (or "none" for local) |

## Security Model

### Defense in Depth

1. **WASM Sandbox**: Untrusted tools run in isolated WebAssembly containers
2. **Capability Leases**: Scoped, time-bound, use-limited access grants
3. **Policy Engine**: Deterministic allow/deny/require-approval decisions
4. **Credential Injection**: Secrets injected at host boundary, never exposed to tools
5. **Leak Detection**: Request/response scanning for secret exfiltration
6. **Prompt Injection Defense**: Pattern detection + content sanitization
7. **Endpoint Allowlisting**: HTTP only to approved hosts/paths

### Effect Types

Every action declares its side effects:
- `ReadLocal`, `ReadExternal`
- `WriteLocal`, `WriteExternal`
- `CredentialedNetwork`
- `Compute`
- `Financial`

## Deployment

### DietPi + vLLM

BrassClaw is optimized for deployment on DietPi systems with vLLM:

1. **vLLM**: Serves the Qwen/Qwen2.5-7B-Instruct-AWQ model on port 8000
2. **BrassClaw**: Connects to vLLM as an OpenAI-compatible provider
3. **systemd**: Both services managed as systemd units

See `deploy/dietpi-setup.sh` for automated setup.

### Service Dependencies

```
vllm.service (port 8000, GPU inference)
    └── brassclaw.service (depends on vllm)
```

## Directory Structure

```
brassclaw/
├── crates/                    # Rust workspace crates
│   ├── brassclaw_reborn_cli/  # Main binary
│   ├── brassclaw_reborn/      # Reborn runtime
│   ├── brassclaw_agent_loop/  # Core agent loop
│   ├── brassclaw_engine/      # v2 engine + Python orchestrator
│   ├── brassclaw_host_runtime/# Infrastructure bridge
│   ├── brassclaw_llm/         # LLM providers
│   ├── brassclaw_skills/      # Skills system
│   ├── brassclaw_safety/      # Security layer
│   ├── brassclaw_wasm/        # WASM sandbox
│   ├── brassclaw_gateway/     # Web gateway
│   └── ...                    # ~60 more crates
├── skills/                    # Skill definitions (SKILL.md files)
│   ├── caldav/                # CalDAV calendar skill
│   ├── notes/                 # Local notes skill
│   ├── local-search/          # File search skill
│   ├── web-browse/            # Browser automation skill
│   ├── github/                # GitHub API skill
│   └── ...                    # More skills
├── profiles/                  # Runtime profiles
│   ├── local.toml             # Home use (8192 token budget)
│   ├── local-sandbox.toml     # Home + Docker
│   ├── server.toml            # Production server
│   └── server-multitenant.toml
├── deploy/                    # Deployment scripts
│   ├── dietpi-setup.sh        # DietPi automated setup
│   ├── vllm.service           # vLLM systemd unit
│   ├── brassclaw.service      # BrassClaw systemd unit
│   └── env.example            # Environment template
├── registry/                  # Extension registry
├── src/                       # Legacy v1 application code
├── docs/                      # Documentation
└── tests/                     # Integration tests
```
