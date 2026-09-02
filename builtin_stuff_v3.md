# Built-in Functionality — v3 Artifact Plan (Revised)

> **Purpose:** For every built-in first-party capability this document defines the exact v3
> artifacts: class-0 Tools (full DB row spec), class-13 ToolSkills (executor-facing only),
> class-22 PythonCode (pure logic + orchestrator executor bodies), class-1–3 Skills (leaf +
> domain, orchestrator-facing narrative), class-21 Recipes (with `step_descriptions` JSONB +
> intent examples), and class-23 ExtensionCatalogues (24 total: 5 global domain catalogues +
> 19 per-tool catalogues, one per individual tool/capability section).
>
> **ExtensionCatalogue design:** Two tiers of catalogues are provided.
> The **5 global domain catalogues** (`builtin-filesystem`, `builtin-network`, `builtin-memory`,
> `builtin-process`, `builtin-management`) group all components of an entire domain and are
> loaded when the orchestrator needs full domain context.
> The **19 per-tool catalogues** (`ext-read-file`, `ext-write-file`, `ext-list-dir`, `ext-glob`,
> `ext-grep`, `ext-apply-patch`, `ext-http`, `ext-http-save`, `ext-memory-search`,
> `ext-memory-write`, `ext-memory-read`, `ext-memory-tree`, `ext-time`, `ext-json`, `ext-shell`,
> `ext-skill-management`, `ext-trigger-management`, `ext-spawn-subagent`, `ext-web-search`) each
> own exactly one tool's full component stack. Load a per-tool catalogue when the orchestrator
> needs only one tool's context — this reduces context size and improves routing precision.
>
> ---
>
> ## Core Design Principle: Orchestrator-First, LLM-Minimal
>
> **The orchestrator IS the execution engine. Rust makes tools available. PythonCode (class 22)
> in the orchestrator channel DECIDES when and how to call them via `__execute_action__()`.
> The LLM is consulted ONLY when the task requires creative reasoning, composition, or
> irreversible decisions the user must confirm. Every other operation is Tier 0.**
>
> ### The two-channel execution model (mandatory)
>
> ```
> channel: "rust"           → pre-loads the ToolSkill binding (does NOT execute)
> channel: "orchestrator"   → PythonCode calls __execute_action__() to actually run the tool
> ```
>
> A Tier-0 recipe MUST have both: a rust step to pre-load and an orchestrator PythonCode step
> to dispatch. A rust-only Tier-0 recipe is a Q1 hard error (§tier0-orchestrator-channel Rule 2).
>
> ### The orchestrator-first hierarchy
>
> 1. **Tier 0 first**: Can the task be done deterministically? Author a Tier-0 recipe.
> 2. **Split by variant**: Each distinct invocation pattern gets its own recipe + intents.
> 3. **Tier 1 only when necessary**: LLM involvement ONLY for creative/compose/confirm tasks.
> 4. **One leaf skill per approach**: If a tool has 3 common usage patterns, author 3 leaf skills.
> 5. **10+ intent examples per recipe**: More examples = better routing precision.
>
> ### What Tier 0 means in practice
> - **Tier 0 is the default target** for all built-in capabilities that do not require creative
>   reasoning, content composition, or disambiguation.
> - **Every distinct variant of a tool call gets its own Tier-0 recipe.** Three recipes covering
>   `glob` by extension, by name, and in a subdir are better than one Tier-1 recipe that asks the
>   LLM to figure out which pattern to use.
> - **More intent examples = better routing.** Each recipe should have 10+ intent examples
>   covering the full natural-language range a user would express for that task. Include exact
>   command-line-style inputs (e.g. `"git status"`, `"ls -l"`) as well as natural language.
> - **One function per leaf skill.** A leaf skill describes exactly one approach to one tool.
>   If a tool has three common usage patterns, author three leaf skills — not one monolithic skill
>   that covers all patterns.
> - **The orchestrator NEVER calls Rust directly.** Rust is not an agent. The orchestrator
>   (PythonCode in the orchestrator channel) calls Rust tools via `__execute_action__()`.
>   The `channel: "rust"` recipe step only pre-loads the ToolSkill binding — it does NOT execute.
> - **Combine what must be combined; split what can be split.** A combined recipe like
>   `json-parse-and-query` or `memory-search-and-read` is valid when both steps always happen
>   together. But a recipe that conditionally does one thing OR another needs an LLM to decide —
>   and is better split into two Tier-0 recipes with their own intents.
>
> ---
>
> **Architectural principle — Orchestrator drives Rust, always:**
> The orchestrator is ALWAYS the supervisory layer. Rust makes tools *available* via the
> rust channel; PythonCode in the orchestrator channel DECIDES when and how to call them.
> This applies to every Tier-0 recipe: the `channel: "rust"` step pre-loads a ToolSkill
> binding, and the `channel: "orchestrator"` PythonCode step calls `__execute_action__()`
> to actually dispatch it. A Tier-0 recipe with a rust step but no orchestrator PythonCode
> step **fails Q1 §tier0-orchestrator-channel Rule 2** and is rejected.
>
> The `channel: "rust"` step does NOT execute the tool — it pre-loads the ToolSkill binding
> into the thread execution context so the executor knows which tool is available. The actual
> tool invocation happens ONLY in the `channel: "orchestrator"` PythonCode step via
> `__execute_action__()`. This two-channel separation is mandatory and enforced at Q1.
>
> **Q1 §tier0-orchestrator-channel rules (hard errors):**
> - Rule 1: Tier-0 `orchestrator_steps` may ONLY contain PythonCode (class 22). Skill
>   bodies are LLM prose — unexecutable without an LLM. Found Skill in Tier-0 orchestrator
>   channel → promote to Tier 1 or replace with PythonCode.
> - Rule 2: If `llm_call_required == false` AND `rust_steps` has tool_bindings, then
>   `orchestrator_steps` MUST contain ≥1 PythonCode UUID. Empty orchestrator channel with
>   tool bindings in a Tier-0 recipe → hard Q1 error.
>
> **§shell-guard (custom commands):** Any Recipe using `builtin.shell` where the command
> string is user-supplied, user-composed, or contains non-constant parts is
> `llm_call_required: true`. The LLM must validate every *custom or user-composed* shell
> command before dispatch. This guard exists to prevent prompt-injection into shell execution.
>
> **§shell-safe-fixed (pre-validated commands):** A Recipe using `builtin.shell` with a
> *fully pre-validated, compile-time-constant command string* (no user-supplied parts, no
> slot interpolation of command content) MAY be `llm_call_required: false` (Tier 0). The
> command must be a fixed literal — e.g. `"git status"`, `"df -h"`, `"uname -a"`. The
> PythonCode executor must hardcode the exact command string (not read it from any slot).
> Pre-validated Tier-0 shell recipes are safe because there is no injection surface.
>
> **§spawn_subagent-guard:** Any Recipe referencing `builtin.spawn_subagent` is
> `llm_call_required: true` — NEVER Tier 0.
>
> **Skill granularity rule (one approach per skill):**
> Author ONE leaf skill per *approach* to a tool, not one skill per tool. Three skills
> covering three use-case approaches to `grep` (by file list, by content, by count) are
> better than one monolithic grep skill. Domain skills reference leaves by name — they
> never duplicate content. When in doubt: split.
>
> **Recipe variant rule (one recipe per variant):**
> Author ONE recipe per distinct invocation pattern. A `file-list-recursive` recipe is
> better than a `file-list` recipe with an LLM deciding whether to recurse. The intent
> system routes to the right recipe; the recipe executes deterministically at Tier 0.
> Target: 3–5 Tier-0 recipes per tool, each covering a distinct common use case.
>
> **PythonCode executor pattern (the canonical Tier-0 body):**
> ```python
> # Channel: orchestrator | Class: 22 | No I/O, no imports, no network, no DB.
> # IBS bakes in {{vars.slotN}} values before execution.
> # __execute_action__ is provided by the runtime sandbox — not imported.
> result = __execute_action__("tool_name", {"param": "{{vars.slot0}}"})
> ```
> The PythonCode body calls `__execute_action__` — this is the ONLY way a Tier-0 recipe
> actually dispatches a Rust tool. The rust channel step pre-loads the ToolSkill binding;
> the PythonCode step drives execution.
>
> **When to use Tier 1 (LLM in the loop):**
> - The operation requires creative content composition (write_file, apply_patch)
> - The operation requires a user-composed or user-supplied shell command (§shell-guard-custom)
> - The operation requires interpreting ambiguous user intent into tool parameters
> - The operation has irreversible effects and benefits from LLM confirmation
> - The operation spans multiple tools in a non-deterministic sequence
>
> **When shell CAN be Tier 0 (§shell-safe-fixed):**
> - The command is a fixed literal string known at recipe authoring time
> - No part of the command is derived from user input or slot interpolation
> - The command is read-only or non-destructive (git status, df, ps, env, uname, pwd, which)
> - Examples: `git status`, `git log --oneline -20`, `git diff --name-only HEAD`, `df -h`, `uname -a`, `ps aux`, `git config --list`
>
> **The pure-logic PythonCode helper pattern:**
> PythonCode helpers (class 22) that do NOT call `__execute_action__()` are pure-logic
> post-processors (string split, CSV parse, list filter, dict pick, etc.). They use only
> built-in Python operations, no imports, no I/O, no network. They transform data that
> a preceding tool call returned. Name them `pc-<operation>-<noun>` (e.g. `pc-string-split`,
> `pc-list-unique`). A recipe that needs two pure-logic steps chains two helpers — it never
> builds a monolithic helper.
>
> **Review corrections applied:** F-01 PythonCode I/O removed; F-02 Tool rows fully
> specified; F-03 five catalogues; F-04 leaf Skills per tool; F-05 ToolSkill bodies
> executor-only; F-07 shell/subagent Tier-1 enforced; F-08 step_descriptions JSONB per
> recipe; F-09 tool_name = Tool row name not capability ID; F-10 Q1/Q2 bypass corrected;
> F-11 LLM call as type:llm step; F-12 Action artifacts added; F-13 intent_examples on all
> recipes; **F-14 CRITICAL: all Tier-0 recipes now have PythonCode executor steps (channel:
> orchestrator) — no bare rust-only Tier-0 recipes; F-15 skill granularity: overly broad
> skills split into per-approach leaf skills; F-16 Orchestrator-first: maximum Tier-0
> coverage, one recipe per variant, 6–10 intent examples each.**
>
> **Prerequisite phases:** A–C (V050–V053), L (V057 adds `source='system'`).
> `reborn_python_code` (V052) and `reborn_extension_catalogues` (V053) must exist.
>
> **Reference implementation for extensions:** `tomedo_v3.md` applies every
> principle in this document to a real external API (tomedo EMR). Read it as
> the canonical example of how to structure an extension's full component stack:
> Tools → ToolSkills → PythonCode → Leaf Skills → Domain Skills → Recipes →
> ExtensionCatalogues, with Tier-0 coverage of all deterministic operations and
> Tier-1 only where the LLM genuinely adds value.

---

## Step 1 — `builtin.shell` (Shell Command Execution)

> **Capability:** `builtin.shell` · **Effect:** `mixed` · **Permission:** Ask
>
> **Two tiers of shell recipes:**
> - **§shell-guard-custom (Tier 1):** Any recipe where the command string contains
>   user-supplied or user-composed parts. The LLM must validate and compose the command.
>   These recipes always have `llm_call_required: true`.
> - **§shell-safe-fixed (Tier 0):** Recipes with a fully pre-validated, fixed-literal
>   command (e.g. `"git status"`, `"df -h"`) where no user input enters the command string.
>   These recipes have `llm_call_required: false` and use a dedicated PythonCode executor
>   that hardcodes the command — `pc-exec-shell-fixed-<name>` or `pc-exec-shell-fixed`.
>   Safe because there is zero injection surface.

### Step 1.1 — Tool row (class 0)

```
name:            "shell"
description:     "Execute a shell command or script in the sandboxed process executor.
                  Returns {output, exit_code, success, sandboxed}. When stdout+stderr
                  exceeds the inline cap, the full output is saved to a scoped workspace
                  file and the response body contains the saved path."
capability_id:   "builtin.shell"
effect_type:     "mixed"
param_schema: {
  "type": "object",
  "properties": {
    "command":      {"type": "string", "description": "Shell command or multi-line script body"},
    "workdir":      {"type": "string", "description": "Working directory (must be a backed scoped path)"},
    "timeout_secs": {"type": "number", "description": "Wall-clock timeout, max 120"},
    "extra_env":    {"type": "object", "description": "Additional environment variables"}
  },
  "required": ["command"]
}
param_template:  {"command": ""}
preconditions:   ""
error_handling:  ""
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

### Step 1.2 — ToolSkill: `ts-shell-run` (class 13)

> Executor-facing only. The orchestrator never reads this body.

```
name:          "ts-shell-run"
tool_name:     "shell"
description:   "Run a shell command via builtin.shell. Accepts command (required), optional
                workdir (must be a backed scoped path), optional timeout_secs (1–120).
                Returns {output, exit_code, success, sandboxed}. When output exceeds the
                inline cap, a saved_file path is returned — call read_file to retrieve it."
param_schema:  [
  {name: "command",      param_type: "string",  required: true,
   description: "Shell command or multi-line script"},
  {name: "workdir",      param_type: "string",  required: false,
   description: "Backed scoped working directory path"},
  {name: "timeout_secs", param_type: "number",  required: false,
   description: "Timeout in seconds, max 120"}
]
param_template: {"command": "{{command}}"}
preconditions:  "No interactive TTY. workdir must be a mount-backed path with execute
                 permission. Unbacked scoped paths are rejected."
error_handling: "exit_code != 0: surface to orchestrator for decision.
                 output contains saved_file path: orchestrator must call read_file.
                 RuntimeDispatchErrorKind::Resource: timeout exceeded."
category:       "process"
source:         "system"
validation_status: "validated"
```

### Step 1.3 — Leaf Skill: `skill-shell-run` (class 1)

> Orchestrator narrative. One tool, one concern: how to run a shell command.

```
name:        "skill-shell-run"
class_code:  1
description: "Leaf skill: how to drive the executor to run a single shell command."
body: |
  Use `ts-shell-run` when you need to execute one shell command. Pass the command
  string verbatim; do NOT construct it from unvalidated user input without escaping.
  Check `success` in the result; a false value means the command returned a non-zero
  exit code — inspect `output` for details and decide whether to retry, report, or
  continue. When the result contains a `saved_file` path (large output was saved),
  call `skill-read-file` on that path to retrieve the full content before proceeding.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 1.4 — Leaf Skill: `skill-shell-safe-check` (class 1)

> Separate grain: how to decide whether a command is safe to run.

```
name:        "skill-shell-safe-check"
class_code:  1
description: "Leaf skill: safety rules for shell command execution."
body: |
  Before dispatching any command via `ts-shell-run`, apply these rules:
  - Never pass user-supplied strings directly into the command without escaping.
  - Never run a command that modifies security-critical system files (/etc, /bin, etc.).
  - Prefer scoped filesystem tools (skill-read-file, skill-list-dir, skill-grep) over
    shell equivalents (cat, ls, grep) when the structured tool covers the need.
  - When output may exceed 1 MiB, add output-limiting flags (e.g. `head -n 200`).
  - `builtin.shell` requires user approval (PermissionMode::Ask) — the LLM must present
    the command to the user and wait for confirmation before dispatch.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 1.5 — Domain Skill: `skill-shell` (class 2)

> References leaf skills. No duplication of content.

```
name:        "skill-shell"
class_code:  2
description: "Domain skill: when and how to use shell execution — two tiers."
body: |
  Shell execution is the most powerful and most dangerous builtin. Use it only when no
  higher-level tool covers the need (prefer filesystem domain tools for file operations;
  prefer skill-http-fetch for network work).

  TWO TIERS OF SHELL EXECUTION:

  Tier 0 — Fixed pre-validated commands (§shell-safe-fixed):
  Use when the command is a fixed literal with no user input. Zero injection surface.

  Git inspection (status / diff / history):
  — skill-shell-git-status:         'git status'
  — skill-shell-git-log:            'git log --oneline -20'
  — skill-shell-git-log-stat:       'git log --stat --oneline -5' (per-file change counts)
  — skill-shell-git-diff-stat:      'git diff --stat'
  — skill-shell-git-diff-name-only: 'git diff --name-only HEAD' (changed filenames only)
  — skill-shell-git-branch:         'git branch -a'
  — skill-shell-git-stash-list:     'git stash list'
  — skill-shell-git-stash-show:     'git stash show' (diff summary of latest stash)
  — skill-shell-git-remote:         'git remote -v'
  — skill-shell-git-show-stat:      'git show --stat HEAD'
  — skill-shell-git-tag-list:       'git tag --list'
  — skill-shell-git-config-list:    'git config --list' (all active git config)

  System information:
  — skill-shell-pwd: run 'pwd'
  — skill-shell-df: run 'df -h'
  — skill-shell-ps: run 'ps aux'
  — skill-shell-env: run 'env'
  — skill-shell-uname: run 'uname -a'
  — skill-shell-which: run 'which <tool>' (tool name is a fixed slot, not user-composed)
  — skill-shell-date: run 'date -u' (UTC date/time)
  — skill-shell-hostname: run 'hostname'
  — skill-shell-whoami: run 'whoami'
  — skill-shell-uptime: run 'uptime'
  — skill-shell-free: run 'free -h' (Linux only)
  — skill-shell-wc-l: run 'wc -l <file>' (line count, path validated)

  Read-only git commands (fetch):
  — skill-shell-git-fetch: 'git fetch --all' (Tier 0 — §shell-safe-fixed)

  Decision guide for git work:
  • What changed since last commit (names only) → skill-shell-git-diff-name-only (Tier 0)
  • What changed in detail → shell-run 'git diff HEAD' (Tier 1 — custom)
  • Recent commit history with stats → skill-shell-git-log-stat (Tier 0)
  • What is in the stash → skill-shell-git-stash-show (Tier 0)
  • Git identity/config check → skill-shell-git-config-list (Tier 0)
  • Fetch latest remote refs without merging → skill-shell-git-fetch (Tier 0)

  Tier 1 — Custom/user-composed commands (§shell-guard-custom):
  Use when the command string involves user intent, user-supplied paths, or composition.
  — skill-shell-run: run a single composed command (LLM validates and composes)
  — skill-shell-safe-check: safety rules before composing any command

  Git write operations (always Tier 1 — §shell-guard-custom):
  — skill-shell-git-commit: 'git commit -m <msg>' (LLM composes message, user confirms)
  — skill-shell-git-push: 'git push <remote> <branch>' (user confirms remote/branch)
  — skill-shell-git-pull: 'git pull <remote> [branch]' (user confirms; LLM handles conflicts)

  Safety rules before running any command → skill-shell-safe-check.
  NEVER run a git commit/push/pull without explicit user confirmation.
  NEVER run a command that the user supplied without LLM validation first.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 1.6 — Recipe: `shell-run` (class 21)

> **Tier:** 1 (`llm_call_required: true`) — §shell-guard hard rule. Never Tier 0.

```
name:        "shell-run"
description: "Run a single shell command and return its output."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-shell>", "<uuid:skill-shell-run>", "<uuid:skill-shell-safe-check>"],
    "label":   "Load shell domain + run + safety-check leaf skills"
  },
  {
    "step_id": "step-1",
    "type":    "llm",
    "label":   "LLM validates safety, composes the exact command, gets user approval"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Executor pre-loads ts-shell-run binding"
  }
]
intent_examples: [
  {"input": "run a command",                        "class": 2},
  {"input": "execute a shell command",              "class": 2},
  {"input": "run ls in the project dir",            "class": 3},
  {"input": "check git status",                     "class": 3},
  {"input": "shell",                                "class": 1},
  {"input": "run this command in the project root", "class": 3},
  {"input": "execute git pull",                     "class": 3},
  {"input": "run a quick system command",           "class": 2},
  {"input": "shell execute this",                   "class": 1},
  {"input": "run this CLI command",                 "class": 2}
]
source: "system"
validation_status: "validated"
```

### Step 1.7 — Recipe: `shell-script` (class 21)

> **Tier:** 1 — same §shell-guard applies.

```
name:        "shell-script"
description: "Execute a multi-line shell script authored by the LLM."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-shell>", "<uuid:skill-shell-safe-check>"],
    "label":   "Load shell domain + safety-check context"
  },
  {
    "step_id": "step-1",
    "type":    "llm",
    "label":   "LLM writes the full script body, validates safety, gets user approval"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Executor pre-loads ts-shell-run binding"
  }
]
intent_examples: [
  {"input": "run a bash script",                                    "class": 2},
  {"input": "execute a script",                                     "class": 2},
  {"input": "write and run a shell script that backs up my files",  "class": 3},
  {"input": "bash script",                                          "class": 1},
  {"input": "create and run a multi-step shell script",             "class": 2},
  {"input": "write a script to process these log files",            "class": 3},
  {"input": "run a shell script with these steps",                  "class": 2},
  {"input": "execute a batch script",                               "class": 2}
]
source: "system"
validation_status: "validated"
```

---

## Step 1.x — Shell Tier-0 Infrastructure (§shell-safe-fixed)

> These components implement the §shell-safe-fixed tier: fixed-literal, pre-validated shell
> commands that the orchestrator runs deterministically WITHOUT LLM involvement.
> The invariant: **every PythonCode below hardcodes the exact command string** — it is never
> derived from user input or slot interpolation. No injection surface → safe for Tier 0.

### Step 1.x.1 — PythonCode: `pc-exec-shell-git-status` (class 22)

```
name:        "pc-exec-shell-git-status"
description: "Orchestrator executor (§shell-safe-fixed): runs 'git status' in the workspace
              root via builtin.shell. Command is a fixed literal. No user input enters the
              command string. Output: {output, exit_code, success}."
content: |
  # §shell-safe-fixed: command is a compile-time constant — no injection surface.
  result = __execute_action__("shell", {"command": "git status"})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 1.x.2 — PythonCode: `pc-exec-shell-git-log` (class 22)

```
name:        "pc-exec-shell-git-log"
description: "Orchestrator executor (§shell-safe-fixed): runs 'git log --oneline -20'
              to get the last 20 commits. Fixed literal command."
content: |
  # §shell-safe-fixed: fixed command, no user input, no injection surface.
  result = __execute_action__("shell", {"command": "git log --oneline -20"})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 1.x.3 — PythonCode: `pc-exec-shell-git-diff-stat` (class 22)

```
name:        "pc-exec-shell-git-diff-stat"
description: "Orchestrator executor (§shell-safe-fixed): runs 'git diff --stat' to show
              changed file summary. Fixed literal command."
content: |
  # §shell-safe-fixed: fixed command, no user input, no injection surface.
  result = __execute_action__("shell", {"command": "git diff --stat"})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 1.x.4 — PythonCode: `pc-exec-shell-git-branch` (class 22)

```
name:        "pc-exec-shell-git-branch"
description: "Orchestrator executor (§shell-safe-fixed): runs 'git branch -a' to list all
              local and remote branches. Fixed literal command."
content: |
  # §shell-safe-fixed: fixed command, no user input, no injection surface.
  result = __execute_action__("shell", {"command": "git branch -a"})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 1.x.5 — PythonCode: `pc-exec-shell-git-stash-list` (class 22)

```
name:        "pc-exec-shell-git-stash-list"
description: "Orchestrator executor (§shell-safe-fixed): runs 'git stash list' to show
              the stash stack. Fixed literal command."
content: |
  # §shell-safe-fixed: fixed command, no user input, no injection surface.
  result = __execute_action__("shell", {"command": "git stash list"})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 1.x.6 — PythonCode: `pc-exec-shell-pwd` (class 22)

```
name:        "pc-exec-shell-pwd"
description: "Orchestrator executor (§shell-safe-fixed): runs 'pwd' to show the current
              working directory. Fixed literal command."
content: |
  # §shell-safe-fixed: fixed command, no user input.
  result = __execute_action__("shell", {"command": "pwd"})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 1.x.7 — PythonCode: `pc-exec-shell-df` (class 22)

```
name:        "pc-exec-shell-df"
description: "Orchestrator executor (§shell-safe-fixed): runs 'df -h' to show disk usage
              in human-readable format. Fixed literal command."
content: |
  # §shell-safe-fixed: fixed command, no user input.
  result = __execute_action__("shell", {"command": "df -h"})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 1.x.8 — PythonCode: `pc-exec-shell-ps` (class 22)

```
name:        "pc-exec-shell-ps"
description: "Orchestrator executor (§shell-safe-fixed): runs 'ps aux' to list running
              processes. Fixed literal command."
content: |
  # §shell-safe-fixed: fixed command, no user input.
  result = __execute_action__("shell", {"command": "ps aux"})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 1.x.9 — PythonCode: `pc-exec-shell-env` (class 22)

```
name:        "pc-exec-shell-env"
description: "Orchestrator executor (§shell-safe-fixed): runs 'env' to list all environment
              variables in the current session. Fixed literal command."
content: |
  # §shell-safe-fixed: fixed command, no user input.
  result = __execute_action__("shell", {"command": "env"})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 1.x.10 — PythonCode: `pc-exec-shell-uname` (class 22)

```
name:        "pc-exec-shell-uname"
description: "Orchestrator executor (§shell-safe-fixed): runs 'uname -a' to show OS/kernel
              information. Fixed literal command."
content: |
  # §shell-safe-fixed: fixed command, no user input.
  result = __execute_action__("shell", {"command": "uname -a"})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 1.x.11 — PythonCode: `pc-exec-shell-which` (class 22)

> Semi-fixed: the tool name is a single-token slot variable (only letters/numbers/hyphen
> allowed). The command is `which <toolname>` where toolname is a safe identifier.
> The PythonCode validates the tool name against a safe identifier regex before dispatch.

```
name:        "pc-exec-shell-which"
description: "Orchestrator executor (§shell-safe-fixed variant): runs 'which <toolname>'
              to locate a binary. Input: tool_name (string, must be a safe identifier
              matching [a-zA-Z0-9_-]+). Validates before dispatch."
content: |
  import re as _re
  _tool = "{{vars.slot0}}"
  # Validate: only safe identifiers allowed (no injection surface)
  if not _re.match(r'^[a-zA-Z0-9_\-]{1,64}$', _tool):
      result = {"error": "Invalid tool name — must be a safe identifier", "success": False}
  else:
      result = __execute_action__("shell", {"command": f"which {_tool}"})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 1.x.12 — PythonCode: `pc-exec-shell-git-log-n` (class 22)

> Semi-fixed: the count N is a numeric slot variable. Validated to be a safe integer.

```
name:        "pc-exec-shell-git-log-n"
description: "Orchestrator executor (§shell-safe-fixed variant): runs 'git log --oneline -N'
              where N is a validated integer (1–100). Input: count (int, 1–100)."
content: |
  _n = {{vars.slot0}}
  # Validate: only safe integer in 1–100 range
  if not isinstance(_n, int) or not (1 <= _n <= 100):
      _n = 20
  result = __execute_action__("shell", {"command": f"git log --oneline -{_n}"})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 1.x.13 — Leaf Skills: Git / System (class 1)

> One skill per distinct §shell-safe-fixed command approach.

```
name:        "skill-shell-git-status"
class_code:  1
description: "Leaf skill (§shell-safe-fixed): run 'git status' to inspect the working tree."
body: |
  Use `pc-exec-shell-git-status` to get the current git working tree status. The command
  is a fixed literal — no LLM required. Check `success` and `output`. If exit_code is 128,
  the directory is not a git repository — report this to the orchestrator.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

```
name:        "skill-shell-git-log"
class_code:  1
description: "Leaf skill (§shell-safe-fixed): run 'git log --oneline -20' for recent commits."
body: |
  Use `pc-exec-shell-git-log` to retrieve the 20 most recent commit hashes and messages.
  The command is a fixed literal — no LLM required. Parse the output lines to show commit
  history. Each line is '<hash> <message>'. Use pc-exec-shell-git-log-n for a custom count.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

```
name:        "skill-shell-git-diff-stat"
class_code:  1
description: "Leaf skill (§shell-safe-fixed): run 'git diff --stat' for changed-file summary."
body: |
  Use `pc-exec-shell-git-diff-stat` to see which files have uncommitted changes and how
  many lines changed per file. The command is a fixed literal — no LLM required. Use
  this before a commit or patch to understand what has changed.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

```
name:        "skill-shell-git-branch"
class_code:  1
description: "Leaf skill (§shell-safe-fixed): run 'git branch -a' to list all branches."
body: |
  Use `pc-exec-shell-git-branch` to list local and remote branches. The current branch is
  prefixed with '*'. The command is a fixed literal — no LLM required.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

```
name:        "skill-shell-git-stash-list"
class_code:  1
description: "Leaf skill (§shell-safe-fixed): run 'git stash list' to show the stash stack."
body: |
  Use `pc-exec-shell-git-stash-list` to list all stashed changes. The command is a fixed
  literal — no LLM required. Empty output means no stashes exist.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

```
name:        "skill-shell-pwd"
class_code:  1
description: "Leaf skill (§shell-safe-fixed): run 'pwd' to get the current working directory."
body: |
  Use `pc-exec-shell-pwd` to obtain the absolute working directory path. Fixed command — no
  LLM required. Useful when constructing paths for other tools that need an absolute base.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

```
name:        "skill-shell-df"
class_code:  1
description: "Leaf skill (§shell-safe-fixed): run 'df -h' for human-readable disk usage."
body: |
  Use `pc-exec-shell-df` to check available disk space on all mounted filesystems. The
  command is a fixed literal — no LLM required. Parse output for 'Use%' to detect
  filesystems approaching capacity.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

```
name:        "skill-shell-ps"
class_code:  1
description: "Leaf skill (§shell-safe-fixed): run 'ps aux' to list running processes."
body: |
  Use `pc-exec-shell-ps` to see all running processes with CPU/memory usage. Fixed command
  — no LLM required. Useful for checking if a service is running, finding a PID, or
  diagnosing resource consumption. Output may be large; consider head_limit if needed.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

```
name:        "skill-shell-env"
class_code:  1
description: "Leaf skill (§shell-safe-fixed): run 'env' to list environment variables."
body: |
  Use `pc-exec-shell-env` to inspect the current environment variables. Fixed command — no
  LLM required. Useful for verifying that expected env vars are set (e.g. PATH, EDITOR,
  LANG, API endpoints). Never log credentials visible in env output to permanent memory.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

```
name:        "skill-shell-uname"
class_code:  1
description: "Leaf skill (§shell-safe-fixed): run 'uname -a' for OS and kernel information."
body: |
  Use `pc-exec-shell-uname` to identify the OS, kernel version, and hardware architecture.
  Fixed command — no LLM required. Useful for confirming the execution environment before
  running platform-specific commands.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

```
name:        "skill-shell-which"
class_code:  1
description: "Leaf skill (§shell-safe-fixed variant): run 'which <tool>' to locate a binary."
body: |
  Use `pc-exec-shell-which` with a safe tool name (letters/numbers/hyphens only) to find
  the binary path. The PythonCode validates the tool name before dispatch — no injection
  risk. If exit_code is 1, the tool is not on PATH.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

---

### Step 1.x.14 — Tier-0 Recipes: Git commands (§shell-safe-fixed)

#### Recipe: `shell-git-status` (class 21)

> **Tier:** 0 — §shell-safe-fixed. Fixed literal command, no LLM needed.

```
name:        "shell-git-status"
description: "Run 'git status' and return the working tree state."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-git-status>"],
    "label":   "PythonCode calls __execute_action__(shell, {command:'git status'}) — fixed literal"
  }
]
intent_examples: [
  {"input": "git status",                                      "class": 1},
  {"input": "what is the current git status",                  "class": 1},
  {"input": "show me uncommitted changes",                     "class": 1},
  {"input": "what files have changed",                         "class": 1},
  {"input": "check git working tree",                          "class": 1},
  {"input": "are there any staged changes",                    "class": 2},
  {"input": "what is modified in the repo",                    "class": 2},
  {"input": "show me the repo status",                         "class": 1},
  {"input": "any untracked files",                             "class": 2},
  {"input": "is my working directory clean",                   "class": 2}
]
source: "system"
validation_status: "validated"
```

#### Recipe: `shell-git-log` (class 21)

> **Tier:** 0 — §shell-safe-fixed.

```
name:        "shell-git-log"
description: "Show the last 20 commits as one-line summaries ('git log --oneline -20')."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-git-log>"],
    "label":   "PythonCode calls __execute_action__(shell, {command:'git log --oneline -20'})"
  }
]
intent_examples: [
  {"input": "show me recent commits",                          "class": 1},
  {"input": "git log",                                         "class": 1},
  {"input": "what were the last commits",                      "class": 1},
  {"input": "show commit history",                             "class": 1},
  {"input": "list recent git commits",                         "class": 1},
  {"input": "what was the last change merged",                 "class": 2},
  {"input": "show me the git log",                             "class": 1},
  {"input": "recent commit hashes and messages",               "class": 2},
  {"input": "what commits have been made",                     "class": 2},
  {"input": "git history last 20",                             "class": 1}
]
source: "system"
validation_status: "validated"
```

#### Recipe: `shell-git-diff-stat` (class 21)

> **Tier:** 0 — §shell-safe-fixed.

```
name:        "shell-git-diff-stat"
description: "Show a summary of which files have changed and how many lines ('git diff --stat')."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-git-diff-stat>"],
    "label":   "PythonCode calls __execute_action__(shell, {command:'git diff --stat'})"
  }
]
intent_examples: [
  {"input": "what files changed",                              "class": 1},
  {"input": "git diff stat",                                   "class": 1},
  {"input": "show me which files are modified",                "class": 1},
  {"input": "how many lines changed",                          "class": 2},
  {"input": "diff summary",                                    "class": 1},
  {"input": "what is the scope of my changes",                 "class": 2},
  {"input": "show file change counts",                         "class": 2},
  {"input": "git diff summary",                                "class": 1},
  {"input": "which files are dirty",                           "class": 2}
]
source: "system"
validation_status: "validated"
```

#### Recipe: `shell-git-branch` (class 21)

> **Tier:** 0 — §shell-safe-fixed.

```
name:        "shell-git-branch"
description: "List all local and remote git branches ('git branch -a')."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-git-branch>"],
    "label":   "PythonCode calls __execute_action__(shell, {command:'git branch -a'})"
  }
]
intent_examples: [
  {"input": "list git branches",                               "class": 1},
  {"input": "what branch am I on",                             "class": 1},
  {"input": "show all branches",                               "class": 1},
  {"input": "git branch",                                      "class": 1},
  {"input": "what remote branches exist",                      "class": 2},
  {"input": "list all local and remote branches",              "class": 1},
  {"input": "which branches are available",                    "class": 2},
  {"input": "show me the branch list",                         "class": 1},
  {"input": "what is the current branch",                      "class": 2},
  {"input": "git branch listing",                              "class": 1}
]
source: "system"
validation_status: "validated"
```

#### Recipe: `shell-git-stash-list` (class 21)

> **Tier:** 0 — §shell-safe-fixed.

```
name:        "shell-git-stash-list"
description: "List the git stash stack ('git stash list')."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-git-stash-list>"],
    "label":   "PythonCode calls __execute_action__(shell, {command:'git stash list'})"
  }
]
intent_examples: [
  {"input": "list git stashes",                                "class": 1},
  {"input": "show stash contents",                             "class": 1},
  {"input": "what is in the stash",                            "class": 1},
  {"input": "git stash list",                                  "class": 1},
  {"input": "how many stashes do I have",                      "class": 2},
  {"input": "do I have any stashed changes",                   "class": 2},
  {"input": "show me the stash",                               "class": 1},
  {"input": "stash entries",                                   "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Step 1.x.15 — Tier-0 Recipes: System information (§shell-safe-fixed)

#### Recipe: `shell-pwd` (class 21)

> **Tier:** 0 — §shell-safe-fixed.

```
name:        "shell-pwd"
description: "Print the current working directory ('pwd')."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-pwd>"],
    "label":   "PythonCode calls __execute_action__(shell, {command:'pwd'})"
  }
]
intent_examples: [
  {"input": "what is the current directory",                   "class": 1},
  {"input": "pwd",                                             "class": 1},
  {"input": "show me the working directory",                   "class": 1},
  {"input": "what is my cwd",                                  "class": 1},
  {"input": "what directory am I in",                          "class": 1},
  {"input": "print working directory",                         "class": 1},
  {"input": "where am I in the filesystem",                    "class": 2},
  {"input": "show current path",                               "class": 2}
]
source: "system"
validation_status: "validated"
```

#### Recipe: `shell-df` (class 21)

> **Tier:** 0 — §shell-safe-fixed.

```
name:        "shell-df"
description: "Show disk usage for all mounted filesystems in human-readable format ('df -h')."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-df>"],
    "label":   "PythonCode calls __execute_action__(shell, {command:'df -h'})"
  }
]
intent_examples: [
  {"input": "check disk space",                                "class": 1},
  {"input": "how much disk is free",                           "class": 1},
  {"input": "df -h",                                           "class": 1},
  {"input": "disk usage",                                      "class": 1},
  {"input": "is the disk full",                                "class": 2},
  {"input": "show filesystem space",                           "class": 1},
  {"input": "how much storage is available",                   "class": 2},
  {"input": "storage status",                                  "class": 2},
  {"input": "show mounted disk space",                         "class": 2}
]
source: "system"
validation_status: "validated"
```

#### Recipe: `shell-ps` (class 21)

> **Tier:** 0 — §shell-safe-fixed.

```
name:        "shell-ps"
description: "List all running processes with CPU and memory usage ('ps aux')."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-ps>"],
    "label":   "PythonCode calls __execute_action__(shell, {command:'ps aux'})"
  }
]
intent_examples: [
  {"input": "list running processes",                          "class": 1},
  {"input": "what processes are running",                      "class": 1},
  {"input": "ps aux",                                          "class": 1},
  {"input": "show all processes",                              "class": 1},
  {"input": "is this service running",                         "class": 2},
  {"input": "check process list",                              "class": 1},
  {"input": "what is consuming CPU",                           "class": 2},
  {"input": "show me the process table",                       "class": 2},
  {"input": "list system processes",                           "class": 1}
]
source: "system"
validation_status: "validated"
```

#### Recipe: `shell-env` (class 21)

> **Tier:** 0 — §shell-safe-fixed.

```
name:        "shell-env"
description: "List all current environment variables ('env')."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-env>"],
    "label":   "PythonCode calls __execute_action__(shell, {command:'env'})"
  }
]
intent_examples: [
  {"input": "show environment variables",                      "class": 1},
  {"input": "list env vars",                                   "class": 1},
  {"input": "what environment variables are set",              "class": 1},
  {"input": "env",                                             "class": 1},
  {"input": "show me the PATH",                                "class": 2},
  {"input": "what is the current environment",                 "class": 1},
  {"input": "check environment configuration",                 "class": 2},
  {"input": "dump environment",                                "class": 1},
  {"input": "what env vars does the session have",             "class": 2}
]
source: "system"
validation_status: "validated"
```

#### Recipe: `shell-uname` (class 21)

> **Tier:** 0 — §shell-safe-fixed.

```
name:        "shell-uname"
description: "Show OS and kernel information ('uname -a')."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-uname>"],
    "label":   "PythonCode calls __execute_action__(shell, {command:'uname -a'})"
  }
]
intent_examples: [
  {"input": "what OS is this",                                 "class": 1},
  {"input": "uname -a",                                        "class": 1},
  {"input": "show kernel version",                             "class": 1},
  {"input": "what is the system architecture",                 "class": 2},
  {"input": "show system info",                                "class": 1},
  {"input": "is this Linux or macOS",                          "class": 2},
  {"input": "show OS details",                                 "class": 1},
  {"input": "kernel info",                                     "class": 1},
  {"input": "what platform am I running on",                   "class": 2}
]
source: "system"
validation_status: "validated"
```

#### Recipe: `shell-which` (class 21)

> **Tier:** 0 — §shell-safe-fixed variant (tool name validated as safe identifier).

```
name:        "shell-which"
description: "Locate a binary on PATH ('which <toolname>') — toolname must be a safe identifier."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-which>"],
    "label":   "PythonCode validates tool name then calls __execute_action__(shell, {command:'which <tool>'})"
  }
]
intent_examples: [
  {"input": "where is git installed",                          "class": 2},
  {"input": "which python",                                    "class": 1},
  {"input": "is docker installed",                             "class": 2},
  {"input": "find the path to node",                           "class": 2},
  {"input": "which cargo",                                     "class": 1},
  {"input": "is this tool on PATH",                            "class": 2},
  {"input": "locate the binary",                               "class": 2},
  {"input": "which command",                                   "class": 1},
  {"input": "find the tool path",                              "class": 2}
]
source: "system"
validation_status: "validated"
```

---

## Step 1.x.16 — Additional Shell Tier-0 Infrastructure (§shell-safe-fixed)

> More fixed-literal shell commands that are universally safe. Every one maps to a
> dedicated PythonCode executor, a leaf skill, and a Tier-0 recipe.

### PythonCode: `pc-exec-shell-date` (class 22)

```
name:        "pc-exec-shell-date"
description: "Orchestrator executor (§shell-safe-fixed): runs 'date -u +%Y-%m-%dT%H:%M:%SZ'
              to print the current UTC date/time as ISO-8601. Fixed literal command."
content: |
  # §shell-safe-fixed: fixed command, no user input.
  result = __execute_action__("shell", {"command": "date -u +%Y-%m-%dT%H:%M:%SZ"})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### PythonCode: `pc-exec-shell-hostname` (class 22)

```
name:        "pc-exec-shell-hostname"
description: "Orchestrator executor (§shell-safe-fixed): runs 'hostname' to print the
              machine hostname. Fixed literal command."
content: |
  # §shell-safe-fixed: fixed command, no user input.
  result = __execute_action__("shell", {"command": "hostname"})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### PythonCode: `pc-exec-shell-whoami` (class 22)

```
name:        "pc-exec-shell-whoami"
description: "Orchestrator executor (§shell-safe-fixed): runs 'whoami' to print the
              current user account name. Fixed literal command."
content: |
  # §shell-safe-fixed: fixed command, no user input.
  result = __execute_action__("shell", {"command": "whoami"})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### PythonCode: `pc-exec-shell-uptime` (class 22)

```
name:        "pc-exec-shell-uptime"
description: "Orchestrator executor (§shell-safe-fixed): runs 'uptime' to show system
              uptime, load average, and logged-in user count. Fixed literal command."
content: |
  # §shell-safe-fixed: fixed command, no user input.
  result = __execute_action__("shell", {"command": "uptime"})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### PythonCode: `pc-exec-shell-free` (class 22)

```
name:        "pc-exec-shell-free"
description: "Orchestrator executor (§shell-safe-fixed): runs 'free -h' to show memory
              usage in human-readable format. Fixed literal command."
content: |
  # §shell-safe-fixed: fixed command, no user input.
  result = __execute_action__("shell", {"command": "free -h"})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### PythonCode: `pc-exec-shell-git-remote` (class 22)

```
name:        "pc-exec-shell-git-remote"
description: "Orchestrator executor (§shell-safe-fixed): runs 'git remote -v' to list
              all configured remote repositories and their URLs. Fixed literal command."
content: |
  # §shell-safe-fixed: fixed command, no user input.
  result = __execute_action__("shell", {"command": "git remote -v"})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### PythonCode: `pc-exec-shell-git-show-stat` (class 22)

```
name:        "pc-exec-shell-git-show-stat"
description: "Orchestrator executor (§shell-safe-fixed): runs 'git show --stat HEAD' to
              show the last commit's changed files and line counts. Fixed literal command."
content: |
  # §shell-safe-fixed: fixed command, no user input.
  result = __execute_action__("shell", {"command": "git show --stat HEAD"})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### PythonCode: `pc-exec-shell-git-tag-list` (class 22)

```
name:        "pc-exec-shell-git-tag-list"
description: "Orchestrator executor (§shell-safe-fixed): runs 'git tag --list' to enumerate
              all tags in the repository. Fixed literal command."
content: |
  # §shell-safe-fixed: fixed command, no user input.
  result = __execute_action__("shell", {"command": "git tag --list"})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### PythonCode: `pc-exec-shell-wc-l` (class 22)

> Semi-fixed: file path is a single safe identifier-style slot. Validated before dispatch.

```
name:        "pc-exec-shell-wc-l"
description: "Orchestrator executor (§shell-safe-fixed variant): runs 'wc -l <filepath>'
              to count lines in a file. Input: filepath (string — must be a safe scoped
              workspace path matching no special characters). Validates before dispatch."
content: |
  import re as _re
  _filepath = "{{vars.slot0}}"
  # Validate: only allow safe relative paths (no shell metacharacters)
  if not _re.match(r'^[a-zA-Z0-9_\-./]{1,256}$', _filepath):
      result = {"error": "Invalid filepath — must be a safe relative path", "success": False}
  else:
      result = __execute_action__("shell", {"command": f"wc -l {_filepath}"})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Leaf Skills: Additional Fixed Shell Commands (class 1)

```
name:        "skill-shell-date"
class_code:  1
description: "Leaf skill (§shell-safe-fixed): run 'date -u' to get current UTC date/time."
body: |
  Use `pc-exec-shell-date` to obtain the current UTC date/time as ISO-8601. Fixed command
  — no LLM required. Prefer skill-time-now for runtime clock; use this only when the shell
  date output format is specifically required.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

```
name:        "skill-shell-hostname"
class_code:  1
description: "Leaf skill (§shell-safe-fixed): run 'hostname' to get the machine name."
body: |
  Use `pc-exec-shell-hostname` to get the current machine's hostname. Fixed command — no
  LLM required. Useful when building environment-specific configuration or diagnosing
  which machine the agent is running on.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

```
name:        "skill-shell-whoami"
class_code:  1
description: "Leaf skill (§shell-safe-fixed): run 'whoami' to get the current user name."
body: |
  Use `pc-exec-shell-whoami` to identify the current OS user account. Fixed command — no
  LLM required. Useful for confirming permissions before running user-specific operations.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

```
name:        "skill-shell-uptime"
class_code:  1
description: "Leaf skill (§shell-safe-fixed): run 'uptime' to check system load and uptime."
body: |
  Use `pc-exec-shell-uptime` to get the system uptime and current load average. Fixed
  command — no LLM required. Useful for diagnosing whether the host is under load before
  triggering resource-intensive operations.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

```
name:        "skill-shell-free"
class_code:  1
description: "Leaf skill (§shell-safe-fixed): run 'free -h' to check available memory."
body: |
  Use `pc-exec-shell-free` to inspect available and used memory on Linux hosts. Fixed
  command — no LLM required. Useful before spawning memory-intensive processes. Note:
  'free' is a Linux-specific command; it may not be available on macOS — use 'vm_stat'
  via shell-run instead if on macOS.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

```
name:        "skill-shell-git-remote"
class_code:  1
description: "Leaf skill (§shell-safe-fixed): run 'git remote -v' to list remote repos."
body: |
  Use `pc-exec-shell-git-remote` to see all configured git remotes and their fetch/push
  URLs. Fixed command — no LLM required. Useful before push/pull operations to confirm
  which remote URL is active.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

```
name:        "skill-shell-git-show-stat"
class_code:  1
description: "Leaf skill (§shell-safe-fixed): run 'git show --stat HEAD' for last commit details."
body: |
  Use `pc-exec-shell-git-show-stat` to see which files the last commit changed and how
  many lines were added/removed. Fixed command — no LLM required. More informative than
  git-diff-stat (which shows unstaged changes); this shows the committed diff.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

```
name:        "skill-shell-git-tag-list"
class_code:  1
description: "Leaf skill (§shell-safe-fixed): run 'git tag --list' to enumerate all tags."
body: |
  Use `pc-exec-shell-git-tag-list` to list all tags in the repository. Fixed command — no
  LLM required. Useful for checking the current version tags before creating a release.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

```
name:        "skill-shell-wc-l"
class_code:  1
description: "Leaf skill (§shell-safe-fixed variant): run 'wc -l <file>' to count lines."
body: |
  Use `pc-exec-shell-wc-l` with a safe workspace-relative file path to count the number
  of lines in a file. The PythonCode validates the path before dispatch — no injection
  risk. Faster than reading the file with skill-read-file when only the line count is
  needed.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

---

### Tier-0 Recipes: Additional Fixed Shell Commands (§shell-safe-fixed)

#### Recipe: `shell-date` (class 21)

```
name:        "shell-date"
description: "Print the current UTC date/time in ISO-8601 format ('date -u +...')."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-date>"],
    "label":   "PythonCode calls __execute_action__(shell, {command:'date -u +...'})"
  }
]
intent_examples: [
  {"input": "what is today's date",                  "class": 1},
  {"input": "print the current date",                "class": 1},
  {"input": "date command",                          "class": 1},
  {"input": "current date in ISO format",            "class": 1},
  {"input": "system date",                           "class": 1},
  {"input": "what date is it",                       "class": 1},
  {"input": "show the current date and time",        "class": 1},
  {"input": "get the date from the shell",           "class": 2}
]
source: "system"
validation_status: "validated"
```

#### Recipe: `shell-hostname` (class 21)

```
name:        "shell-hostname"
description: "Print the machine hostname."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-hostname>"],
    "label":   "PythonCode calls __execute_action__(shell, {command:'hostname'})"
  }
]
intent_examples: [
  {"input": "what is this machine called",           "class": 1},
  {"input": "hostname",                              "class": 1},
  {"input": "what is the server name",               "class": 1},
  {"input": "show me the machine hostname",          "class": 1},
  {"input": "what host is this",                     "class": 1},
  {"input": "machine name",                          "class": 1},
  {"input": "get the hostname",                      "class": 1},
  {"input": "show hostname of this server",          "class": 2}
]
source: "system"
validation_status: "validated"
```

#### Recipe: `shell-whoami` (class 21)

```
name:        "shell-whoami"
description: "Print the current OS user account name ('whoami')."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-whoami>"],
    "label":   "PythonCode calls __execute_action__(shell, {command:'whoami'})"
  }
]
intent_examples: [
  {"input": "who am I running as",                  "class": 1},
  {"input": "what user is this",                    "class": 1},
  {"input": "whoami",                               "class": 1},
  {"input": "current user",                         "class": 1},
  {"input": "what is my username",                  "class": 1},
  {"input": "which user account is active",         "class": 1},
  {"input": "show the current user",                "class": 1},
  {"input": "am I running as root",                 "class": 2}
]
source: "system"
validation_status: "validated"
```

#### Recipe: `shell-uptime` (class 21)

```
name:        "shell-uptime"
description: "Show system uptime and current load average ('uptime')."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-uptime>"],
    "label":   "PythonCode calls __execute_action__(shell, {command:'uptime'})"
  }
]
intent_examples: [
  {"input": "how long has this server been running", "class": 1},
  {"input": "uptime",                                "class": 1},
  {"input": "system uptime",                         "class": 1},
  {"input": "when was this server last rebooted",    "class": 2},
  {"input": "what is the load average",              "class": 1},
  {"input": "is the system under load",              "class": 2},
  {"input": "check server uptime",                   "class": 1},
  {"input": "show me uptime and load",               "class": 1}
]
source: "system"
validation_status: "validated"
```

#### Recipe: `shell-free` (class 21)

```
name:        "shell-free"
description: "Show memory usage in human-readable format ('free -h')."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-free>"],
    "label":   "PythonCode calls __execute_action__(shell, {command:'free -h'})"
  }
]
intent_examples: [
  {"input": "check memory usage",                    "class": 1},
  {"input": "how much RAM is available",             "class": 1},
  {"input": "free -h",                               "class": 1},
  {"input": "memory usage",                          "class": 1},
  {"input": "how much memory does this process use", "class": 2},
  {"input": "is RAM running low",                    "class": 2},
  {"input": "show memory stats",                     "class": 1},
  {"input": "available and used RAM",                "class": 1}
]
source: "system"
validation_status: "validated"
```

#### Recipe: `shell-git-remote` (class 21)

```
name:        "shell-git-remote"
description: "List all configured git remotes and their URLs ('git remote -v')."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-git-remote>"],
    "label":   "PythonCode calls __execute_action__(shell, {command:'git remote -v'})"
  }
]
intent_examples: [
  {"input": "list git remotes",                      "class": 1},
  {"input": "what is the git remote URL",            "class": 1},
  {"input": "git remote -v",                         "class": 1},
  {"input": "show remote repositories",              "class": 1},
  {"input": "what origin URL is configured",         "class": 2},
  {"input": "list all configured remotes",           "class": 1},
  {"input": "what remotes does this repo have",      "class": 2},
  {"input": "show git remote configuration",         "class": 1}
]
source: "system"
validation_status: "validated"
```

#### Recipe: `shell-git-show-stat` (class 21)

```
name:        "shell-git-show-stat"
description: "Show changed files and line counts for the last commit ('git show --stat HEAD')."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-git-show-stat>"],
    "label":   "PythonCode calls __execute_action__(shell, {command:'git show --stat HEAD'})"
  }
]
intent_examples: [
  {"input": "what did the last commit change",       "class": 1},
  {"input": "git show --stat HEAD",                  "class": 1},
  {"input": "show files changed in last commit",     "class": 1},
  {"input": "what was in the previous commit",       "class": 2},
  {"input": "show stat for most recent commit",      "class": 1},
  {"input": "what was the last thing committed",     "class": 2},
  {"input": "show HEAD commit diff summary",         "class": 1},
  {"input": "git show stat last commit",             "class": 1}
]
source: "system"
validation_status: "validated"
```

#### Recipe: `shell-git-tag-list` (class 21)

```
name:        "shell-git-tag-list"
description: "List all tags in the repository ('git tag --list')."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-git-tag-list>"],
    "label":   "PythonCode calls __execute_action__(shell, {command:'git tag --list'})"
  }
]
intent_examples: [
  {"input": "list git tags",                         "class": 1},
  {"input": "what tags exist in this repo",          "class": 1},
  {"input": "show all release tags",                 "class": 1},
  {"input": "git tag --list",                        "class": 1},
  {"input": "what versions are tagged",              "class": 2},
  {"input": "list all git version tags",             "class": 1},
  {"input": "show me the tags in this repository",   "class": 1},
  {"input": "what is the latest git tag",            "class": 2}
]
source: "system"
validation_status: "validated"
```

#### Recipe: `shell-wc-l` (class 21)

```
name:        "shell-wc-l"
description: "Count the number of lines in a file ('wc -l <filepath>')."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-wc-l>"],
    "label":   "PythonCode validates path then calls __execute_action__(shell, {command:'wc -l <file>'})"
  }
]
intent_examples: [
  {"input": "how many lines in this file",           "class": 1},
  {"input": "line count of this file",               "class": 1},
  {"input": "wc -l on this file",                    "class": 1},
  {"input": "count lines in the log file",           "class": 2},
  {"input": "how long is this file",                 "class": 2},
  {"input": "how many rows does this CSV have",      "class": 2},
  {"input": "get line count without reading file",   "class": 2},
  {"input": "count the lines in this source file",   "class": 2}
]
source: "system"
validation_status: "validated"
```

---

## Step 1.x.17 — Extended Shell Tier-0 Infrastructure (§shell-safe-fixed continued)

> Additional fixed-literal git and filesystem commands that the orchestrator commonly
> needs for workflow context. Every one is §shell-safe-fixed: no user input in the
> command string, no slot interpolation of command content.

### PythonCode: `pc-exec-shell-git-diff-name-only` (class 22)

```
name:        "pc-exec-shell-git-diff-name-only"
description: "Orchestrator executor (§shell-safe-fixed): runs 'git diff --name-only HEAD'
              to list only the names of files changed since the last commit. No content shown.
              Fixed literal command — no slot interpolation."
content: |
  # §shell-safe-fixed: fixed command, no user input, no injection surface.
  result = __execute_action__("shell", {"command": "git diff --name-only HEAD"})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### PythonCode: `pc-exec-shell-git-log-stat` (class 22)

```
name:        "pc-exec-shell-git-log-stat"
description: "Orchestrator executor (§shell-safe-fixed): runs 'git log --stat --oneline -5'
              to show the last 5 commits with file-change counts per commit. Fixed literal."
content: |
  # §shell-safe-fixed: fixed command, no user input, no injection surface.
  result = __execute_action__("shell", {"command": "git log --stat --oneline -5"})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### PythonCode: `pc-exec-shell-git-stash-show` (class 22)

```
name:        "pc-exec-shell-git-stash-show"
description: "Orchestrator executor (§shell-safe-fixed): runs 'git stash show' to show
              the diff summary of the most recent stash entry. Fixed literal command."
content: |
  # §shell-safe-fixed: fixed command, no user input, no injection surface.
  result = __execute_action__("shell", {"command": "git stash show"})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### PythonCode: `pc-exec-shell-git-config-list` (class 22)

```
name:        "pc-exec-shell-git-config-list"
description: "Orchestrator executor (§shell-safe-fixed): runs 'git config --list' to show
              all active git configuration values. Fixed literal command."
content: |
  # §shell-safe-fixed: fixed command, no user input, no injection surface.
  result = __execute_action__("shell", {"command": "git config --list"})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Leaf Skills: Extended Git Fixed Commands (class 1)

#### `skill-shell-git-diff-name-only` (class 1)

```
name:        "skill-shell-git-diff-name-only"
class_code:  1
description: "Leaf skill: how to list only filenames changed since the last commit (no diff content)."
body: |
  Use pc-exec-shell-git-diff-name-only (§shell-safe-fixed) to run 'git diff --name-only HEAD'.
  Returns only the names of modified files — no content, no diff hunks. This is the fastest
  way to discover which files are dirty before deciding which ones to inspect or read.
  For full diff content, use shell-run with a custom git diff command (Tier 1 — §shell-guard).
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

#### `skill-shell-git-log-stat` (class 1)

```
name:        "skill-shell-git-log-stat"
class_code:  1
description: "Leaf skill: how to view recent commit history with file-change statistics."
body: |
  Use pc-exec-shell-git-log-stat (§shell-safe-fixed) to run 'git log --stat --oneline -5'.
  Shows the last 5 commits with their changed-file counts and insertions/deletions summary.
  For more commits or a different format, use shell-run (Tier 1 — §shell-guard).
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

#### `skill-shell-git-stash-show` (class 1)

```
name:        "skill-shell-git-stash-show"
class_code:  1
description: "Leaf skill: how to inspect the diff summary of the most recent stash entry."
body: |
  Use pc-exec-shell-git-stash-show (§shell-safe-fixed) to run 'git stash show'.
  Returns a short summary of files and line counts in the top stash entry. Does NOT pop
  or apply the stash. To see the full patch or apply it, use shell-run (Tier 1).
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

#### `skill-shell-git-config-list` (class 1)

```
name:        "skill-shell-git-config-list"
class_code:  1
description: "Leaf skill: how to inspect all active git configuration values."
body: |
  Use pc-exec-shell-git-config-list (§shell-safe-fixed) to run 'git config --list'.
  Returns all effective git config key=value pairs (local, global, system). Useful for
  checking user.email, user.name, remote settings, or merge strategy before making commits.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Tier-0 Recipes: Extended Git Fixed Commands (§shell-safe-fixed)

#### Recipe: `shell-git-diff-name-only` (class 21)

> **Tier:** 0 — §shell-safe-fixed. Returns only the list of changed filenames.
> Use before reading files to discover which ones are modified.

```
name:        "shell-git-diff-name-only"
description: "List only the names of files changed since the last commit (git diff --name-only HEAD)."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-git-diff-name-only>"],
    "label":   "PythonCode calls __execute_action__(shell, {command:'git diff --name-only HEAD'})"
  }
]
intent_examples: [
  {"input": "what files have changed since last commit",              "class": 1},
  {"input": "which files are modified",                               "class": 1},
  {"input": "list changed files",                                     "class": 1},
  {"input": "show modified filenames only",                           "class": 1},
  {"input": "git diff name only",                                     "class": 1},
  {"input": "what did I change in this working directory",            "class": 2},
  {"input": "files with uncommitted changes",                         "class": 1},
  {"input": "what is dirty in the working tree",                      "class": 2},
  {"input": "list unstaged or staged changed files",                  "class": 2},
  {"input": "show only the names of changed files no content",        "class": 1}
]
source: "system"
validation_status: "validated"
```

---

#### Recipe: `shell-git-log-stat` (class 21)

> **Tier:** 0 — §shell-safe-fixed. Shows last 5 commits with per-file change counts.

```
name:        "shell-git-log-stat"
description: "Show the last 5 commits with file-change statistics (git log --stat --oneline -5)."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-git-log-stat>"],
    "label":   "PythonCode calls __execute_action__(shell, {command:'git log --stat --oneline -5'})"
  }
]
intent_examples: [
  {"input": "show recent commits with file changes",                  "class": 1},
  {"input": "git log with stats",                                     "class": 1},
  {"input": "what files changed in the last few commits",             "class": 2},
  {"input": "show commit history with change counts",                 "class": 1},
  {"input": "git log stat",                                           "class": 1},
  {"input": "which files were touched in recent commits",             "class": 2},
  {"input": "last 5 commits with file statistics",                    "class": 1},
  {"input": "show me what changed in recent git history",             "class": 2},
  {"input": "recent commit file change summary",                      "class": 2},
  {"input": "git history with additions deletions per file",          "class": 2}
]
source: "system"
validation_status: "validated"
```

---

#### Recipe: `shell-git-stash-show` (class 21)

> **Tier:** 0 — §shell-safe-fixed. Shows the diff summary of the latest stash entry.

```
name:        "shell-git-stash-show"
description: "Show the diff summary of the most recent git stash entry (git stash show)."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-git-stash-show>"],
    "label":   "PythonCode calls __execute_action__(shell, {command:'git stash show'})"
  }
]
intent_examples: [
  {"input": "what is in my git stash",                                "class": 1},
  {"input": "show the latest stash entry",                            "class": 1},
  {"input": "git stash show",                                         "class": 1},
  {"input": "what changes did I stash",                               "class": 2},
  {"input": "preview the stash without applying it",                  "class": 2},
  {"input": "what files are in the current stash",                    "class": 2},
  {"input": "inspect the stash summary",                              "class": 1},
  {"input": "show stash diff summary",                                "class": 1}
]
source: "system"
validation_status: "validated"
```

---

#### Recipe: `shell-git-config-list` (class 21)

> **Tier:** 0 — §shell-safe-fixed. Lists all active git configuration key-value pairs.

```
name:        "shell-git-config-list"
description: "List all active git configuration values (git config --list)."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-git-config-list>"],
    "label":   "PythonCode calls __execute_action__(shell, {command:'git config --list'})"
  }
]
intent_examples: [
  {"input": "show git configuration",                                 "class": 1},
  {"input": "what is my git user.name",                               "class": 2},
  {"input": "git config list",                                        "class": 1},
  {"input": "show all git settings",                                  "class": 1},
  {"input": "what email is configured for git commits",               "class": 2},
  {"input": "check git config",                                       "class": 1},
  {"input": "show current git identity settings",                     "class": 2},
  {"input": "list active git configuration",                          "class": 1}
]
source: "system"
validation_status: "validated"
```

---



## Step 3.x — Additional write_file Tier-0 Recipe

> `file-write-template` is a Tier-0 variant where the content is fully baked into the
> recipe vars by IBS — no LLM needed. This is useful when a recipe knows in advance
> what to write (e.g. a fixed config stub, a standard header file, an empty init file).

### Step 3.x.1 — Leaf Skill: `skill-write-file-template` (class 1)

```
name:        "skill-write-file-template"
class_code:  1
description: "Leaf skill: how to write a file using a pre-baked template content from vars."
body: |
  Use `ts-write-file` (via pc-exec-write-file) when the content to write is fully
  pre-determined and baked into the recipe vars by IBS — no LLM authorship needed.
  Examples: creating an empty __init__.py, writing a fixed .gitignore stub, creating
  a minimal config file with pre-set default values. The path and content both come
  from vars, not from user input that needs interpretation.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 3.x.2 — Recipe: `file-write-template` (class 21)

> **Tier:** 0 — the content is a pre-composed template in the recipe vars. No LLM needed.
> This is the distinction from `file-write` (Tier 1): here IBS pre-bakes both path and
> content from known template vars; the orchestrator dispatches without any LLM.

```
name:        "file-write-template"
description: "Write a file using a fully pre-baked template content from recipe vars (no LLM)."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-write-file>"],
    "label":   "Pre-load ts-write-file ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-write-file>"],
    "label":   "PythonCode calls __execute_action__(write_file, {path:slot0, content:slot1}) — both pre-baked"
  }
]
intent_examples: [
  {"input": "create an empty __init__.py",                     "class": 2},
  {"input": "create a default .gitignore",                     "class": 2},
  {"input": "write a minimal config file",                     "class": 2},
  {"input": "initialize this file with a template",            "class": 2},
  {"input": "create a stub file",                              "class": 2},
  {"input": "write a file from a template",                    "class": 1},
  {"input": "scaffold a new config file",                      "class": 2},
  {"input": "create a default settings file",                  "class": 2},
  {"input": "file write from template vars",                   "class": 1}
]
source: "system"
validation_status: "validated"
```

---

## Step 7.x — Additional apply_patch Tier-0 Recipe

> `file-patch-replace-all` is a Tier-0 variant for cases where the old_string and
> new_string are both fully pre-baked from vars. If both strings are known in advance
> (e.g. a well-known version string bump, a config key rename), no LLM is needed.

### Step 7.x.1 — PythonCode: `pc-exec-apply-patch` (class 22)

```
name:        "pc-exec-apply-patch"
description: "Orchestrator executor: calls __execute_action__ to apply a targeted patch via
              builtin.apply_patch. Input: path (string), old_string (string), new_string
              (string), replace_all (optional bool). Output: {path, replacements_made}."
content: |
  # Orchestrator executor body.
  _path = "{{vars.slot0}}"
  _old = "{{vars.slot1}}"
  _new = "{{vars.slot2}}"
  _replace_all = {{vars.slot3}}
  _params = {"path": _path, "old_string": _old, "new_string": _new}
  if _replace_all:
      _params["replace_all"] = True
  result = __execute_action__("apply_patch", _params)
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 7.x.2 — Recipe: `file-patch-replace-all` (class 21)

> **Tier:** 0 — old_string and new_string are both pre-baked from vars. No LLM needed.
> This is appropriate when a well-known global replacement is being performed (e.g. version
> bump across a file, config key rename). The Q1 validation checks that all slots are baked.

```
name:        "file-patch-replace-all"
description: "Replace every occurrence of a pre-known string in a file (no LLM — vars pre-baked)."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-apply-patch>"],
    "label":   "Pre-load ts-apply-patch ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-apply-patch>"],
    "label":   "PythonCode calls __execute_action__(apply_patch, {path, old_string, new_string, replace_all:true})"
  }
]
intent_examples: [
  {"input": "replace all occurrences of this string in the file", "class": 1},
  {"input": "global find and replace in this file",              "class": 1},
  {"input": "replace every instance of this text",               "class": 1},
  {"input": "rename this symbol throughout the file",            "class": 2},
  {"input": "patch all occurrences",                             "class": 1},
  {"input": "replace all matches in file",                       "class": 1},
  {"input": "bulk replace in file",                              "class": 2},
  {"input": "apply replace-all patch",                           "class": 1},
  {"input": "change every occurrence of this value",             "class": 2}
]
source: "system"
validation_status: "validated"
```

---

## Step 5.x.2 — Recipe: `file-glob-recent` (class 21)

> **Tier:** 0 — glob sorted by modification time, capped to 10 most recently changed files.
> Glob results are already sorted by mtime by default; limiting to max_results=10 gives
> a focused "recently modified" view without LLM involvement.

```
name:        "file-glob-recent"
description: "Find the 10 most recently modified files matching a glob pattern."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-glob>"],
    "label":   "Pre-load ts-glob ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-glob>"],
    "label":   "PythonCode calls __execute_action__(glob, {pattern, max_results:10}) — sorted by mtime"
  }
]
intent_examples: [
  {"input": "what files were recently modified",               "class": 1},
  {"input": "show the most recently changed files",            "class": 1},
  {"input": "what changed recently in this project",           "class": 2},
  {"input": "recently modified TypeScript files",              "class": 2},
  {"input": "last 10 modified files",                          "class": 1},
  {"input": "what did I change recently",                      "class": 2},
  {"input": "most recently touched files",                     "class": 2},
  {"input": "find recently edited source files",               "class": 2},
  {"input": "show recently modified files in src",             "class": 2}
]
source: "system"
validation_status: "validated"
```

---



## Step 2 — `builtin.read_file` (File Read)

> **Capability:** `builtin.read_file` · **Effect:** `read` · **Permission:** Allow

### Step 2.1 — Tool row (class 0)

```
name:            "read_file"
description:     "Read the full contents of a scoped-workspace file. Supports an optional
                  line-range selector (start-end, 1-based inclusive). Returns {content,
                  line_count, path}."
capability_id:   "builtin.read_file"
effect_type:     "read"
param_schema: {
  "type": "object",
  "properties": {
    "path":  {"type": "string", "description": "Scoped workspace path to the file"},
    "range": {"type": "string", "description": "Optional line range, format: start-end (1-based)"}
  },
  "required": ["path"]
}
param_template:  {"path": "{{path}}"}
preconditions:   "Path must resolve within a scoped mount with read permission."
error_handling:  "FilesystemDenied: path outside mounts. File not found: surface to user."
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

### Step 2.2 — ToolSkill: `ts-read-file` (class 13)

```
name:          "ts-read-file"
tool_name:     "read_file"
description:   "Read a file from the scoped workspace via builtin.read_file. Optional range
                narrows to specific lines (format: start-end, e.g. '10-50')."
param_schema:  [
  {name: "path",  param_type: "string", required: true,
   description: "Workspace-relative scoped path"},
  {name: "range", param_type: "string", required: false,
   description: "Line range start-end, e.g. '10-50'"}
]
param_template: {"path": "{{path}}"}
preconditions:  "Path must resolve within a scoped mount with read permission.
                 Absolute host paths and traversal sequences (..) are rejected."
error_handling: "FilesystemDenied: path outside mounts — report to orchestrator.
                 File not found: surface path to user for confirmation."
category:       "filesystem"
source:         "system"
validation_status: "validated"
```

### Step 2.3 — PythonCode: `pc-exec-read-file` (class 22)

> **Orchestrator executor pattern.** This PythonCode calls `__execute_action__` to dispatch
> the read_file tool. It is the body that actually drives execution in every Tier-0 recipe
> that reads a file.

```
name:        "pc-exec-read-file"
description: "Orchestrator executor: calls __execute_action__ to read a file via
              builtin.read_file. Input: path (string), range (optional string, e.g. '1-50').
              Output: tool result dict {content, line_count, path}."
content: |
  # Orchestrator executor body. __execute_action__ is provided by the runtime sandbox.
  # IBS bakes in path and range values as {{vars.slot0}} / {{vars.slot1}} before execution.
  # No I/O, no imports — pure orchestrator dispatch.
  _path = "{{vars.slot0}}"
  _range = "{{vars.slot1}}"
  _params = {"path": _path}
  if _range and _range != "":
      _params["range"] = _range
  result = __execute_action__("read_file", _params)
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 2.4 — Leaf Skill: `skill-read-file` (class 1)

```
name:        "skill-read-file"
class_code:  1
description: "Leaf skill: how to read a file from the workspace."
body: |
  Use `ts-read-file` (via pc-exec-read-file) when you need to inspect a file's content.
  Always read a file before editing it — never overwrite blindly.
  For large files, use the `range` parameter (e.g. '1-100') to read specific line spans
  rather than loading the entire file at once.
  If the path is unknown, call skill-list-dir or skill-glob first to discover valid paths.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 2.5 — Leaf Skill: `skill-read-file-range` (class 1)

> Separate grain: paginated reading via line-range selector.

```
name:        "skill-read-file-range"
class_code:  1
description: "Leaf skill: how to read a specific line range from a large file."
body: |
  When a file is too large to read in full, use the `range` parameter of `ts-read-file`
  (e.g. range='100-200') to read only the needed lines. Check `line_count` in the first
  read result to know the file length, then paginate through sections. Each range call
  returns only those lines. Use this pattern to avoid the 1 MiB output cap.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 2.6 — Recipe: `file-read` (class 21)

> **Tier:** 0 — orchestrator reads the file deterministically via PythonCode executor.
> **Corrected from previous version:** orchestrator step now uses PythonCode (pc-exec-read-file),
> NOT a Skill body (which would be LLM prose, violating §tier0-orchestrator-channel Rule 1).

```
name:        "file-read"
description: "Read a file from the workspace and return its contents."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-read-file>"],
    "label":   "Pre-load ts-read-file ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-read-file>"],
    "label":   "PythonCode calls __execute_action__(read_file, {path, range}) and returns result"
  }
]
intent_examples: [
  {"input": "read a file",                          "class": 1},
  {"input": "show me the contents of",              "class": 1},
  {"input": "open file",                            "class": 1},
  {"input": "what is in config.toml",               "class": 2},
  {"input": "read the file at this path",           "class": 1},
  {"input": "show me this file",                    "class": 1},
  {"input": "load file contents",                   "class": 1},
  {"input": "display the file",                     "class": 1},
  {"input": "cat this file",                        "class": 2},
  {"input": "inspect the configuration file",       "class": 2},
  {"input": "file read",                            "class": 1}
]
source: "system"
validation_status: "validated"
```

---

### Step 2.7 — Recipe: `file-read-range` (class 21)

> **Tier:** 0 — deterministic ranged read. Variant for large files where only a line span is needed.
> One recipe per variant: the intent system routes here when the user specifies a line range.

```
name:        "file-read-range"
description: "Read a specific line range from a file (for large files or targeted inspection)."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-read-file>"],
    "label":   "Pre-load ts-read-file ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-read-file>"],
    "label":   "PythonCode calls __execute_action__(read_file, {path, range}) — range slot is required"
  }
]
intent_examples: [
  {"input": "read lines 10 to 50 of main.rs",                "class": 1},
  {"input": "show me line 100 to 200 of this file",          "class": 1},
  {"input": "read the first 30 lines",                       "class": 1},
  {"input": "read lines 500 to 600 of the log",              "class": 1},
  {"input": "show only the top 20 lines",                    "class": 2},
  {"input": "read the middle section of this file",          "class": 2},
  {"input": "show lines starting from 150",                  "class": 2},
  {"input": "paginate through a large file",                 "class": 2},
  {"input": "read just this specific section of the file",   "class": 2},
  {"input": "show me the function body starting at line 80", "class": 2}
]
source: "system"
validation_status: "validated"
```

---


## Step 3 — `builtin.write_file` (File Write)

> **Capability:** `builtin.write_file` · **Effect:** `write` · **Permission:** Allow
> Content size limit: 6 MiB. Overwrites the entire file.

### Step 3.1 — Tool row (class 0)

```
name:            "write_file"
description:     "Write or overwrite a file in the scoped workspace. The entire content is
                  replaced. Returns {path, bytes_written}. For targeted edits prefer
                  apply_patch — it is safer and does not require a full read-back."
capability_id:   "builtin.write_file"
effect_type:     "write"
param_schema: {
  "type": "object",
  "properties": {
    "path":    {"type": "string", "description": "Scoped workspace path"},
    "content": {"type": "string", "description": "Full file content to write"}
  },
  "required": ["path", "content"]
}
param_template:  {"path": "{{path}}", "content": "{{content}}"}
preconditions:   "Path must resolve within a scoped mount with write permission. Content ≤ 6 MiB."
error_handling:  "FilesystemDenied: path outside mounts. Resource limit: content too large."
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

### Step 3.2 — ToolSkill: `ts-write-file` (class 13)

```
name:          "ts-write-file"
tool_name:     "write_file"
description:   "Write or overwrite a file via builtin.write_file. Replaces the entire file
                content. Content limit: 6 MiB. Returns {path, bytes_written}."
param_schema:  [
  {name: "path",    param_type: "string", required: true,
   description: "Workspace-relative scoped path"},
  {name: "content", param_type: "string", required: true,
   description: "Complete new file content"}
]
param_template: {"path": "{{path}}", "content": "{{content}}"}
preconditions:  "Path must resolve within a scoped mount with write permission. Content ≤ 6 MiB."
error_handling: "FilesystemDenied: path outside mounts — report to orchestrator.
                 Resource limit: content too large — split or compress."
category:       "filesystem"
source:         "system"
validation_status: "validated"
```

### Step 3.3 — PythonCode: `pc-exec-write-file` (class 22)

> Orchestrator executor: writes a file via `__execute_action__`. Used in Tier-1 recipes
> after the LLM has determined the content; may also be used standalone in automation.

```
name:        "pc-exec-write-file"
description: "Orchestrator executor: calls __execute_action__ to write a file via
              builtin.write_file. Input: path (string), content (string). Output: tool
              result dict {path, bytes_written}."
content: |
  # Orchestrator executor body. __execute_action__ is provided by the runtime sandbox.
  # IBS bakes in path and content as {{vars.slot0}} / {{vars.slot1}} before execution.
  # No I/O, no imports — pure orchestrator dispatch.
  _path = "{{vars.slot0}}"
  _content = "{{vars.slot1}}"
  result = __execute_action__("write_file", {"path": _path, "content": _content})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 3.4 — Leaf Skill: `skill-write-file-new` (class 1)

> One grain: creating a new file.

```
name:        "skill-write-file-new"
class_code:  1
description: "Leaf skill: how to create a new file in the workspace."
body: |
  Use `ts-write-file` (via pc-exec-write-file) when creating a file that does not yet
  exist. Provide the full intended content. The path must be within the scoped workspace
  mount. The file is created immediately — there is no confirmation step unless the
  orchestrator adds one.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 3.5 — Leaf Skill: `skill-write-file-replace` (class 1)

> Separate grain: replacing an existing file's full content.

```
name:        "skill-write-file-replace"
class_code:  1
description: "Leaf skill: how to fully replace an existing file's content."
body: |
  Use `ts-write-file` to completely replace a file's content when the entire file must
  be rewritten. IMPORTANT: read the file first with skill-read-file before overwriting —
  never discard existing content without seeing it. For small, targeted edits (a few lines),
  prefer skill-apply-patch — it is safer because it requires matching the current content.
  Use write_file only when you genuinely intend to replace the full content.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 3.6 — Recipe: `file-write` (class 21)

> **Tier:** 1 — the LLM must compose the content to write.

```
name:        "file-write"
description: "Read current file content (if it exists), then write new content authored by LLM."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-read-file>", "<uuid:skill-write-file-replace>", "<uuid:skill-write-file-new>"],
    "label":   "Load read + write leaf skill context"
  },
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-read-file>"],
    "label":   "Pre-load ts-read-file binding (for optional pre-read)"
  },
  {
    "step_id": "step-2",
    "type":    "llm",
    "label":   "LLM optionally reads current content, then composes new file content"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-write-file>"],
    "label":   "Pre-load ts-write-file binding"
  }
]
intent_examples: [
  {"input": "write a file",                           "class": 1},
  {"input": "create a file",                          "class": 1},
  {"input": "save content to a file",                 "class": 1},
  {"input": "write a README for this project",        "class": 2},
  {"input": "create config.toml with these values",   "class": 2},
  {"input": "make a new file with this content",      "class": 1},
  {"input": "create a new document",                  "class": 1},
  {"input": "write this content to disk",             "class": 1},
  {"input": "overwrite this file with new content",   "class": 2},
  {"input": "file write",                             "class": 1}
]
source: "system"
validation_status: "validated"
```

---

## Step 4 — `builtin.list_dir` (Directory Listing)

> **Capability:** `builtin.list_dir` · **Effect:** `read_filesystem` · **Permission:** Allow

### Step 4.1 — Tool row (class 0)

```
name:            "list_dir"
description:     "List the contents of a directory through scoped mounts. Returns entry names,
                  types, and sizes. Supports optional recursive listing with a depth cap."
capability_id:   "builtin.list_dir"
effect_type:     "read_filesystem"
param_schema: {
  "type": "object",
  "properties": {
    "path":      {"type": "string",  "description": "Scoped directory path. Defaults to workspace root."},
    "recursive": {"type": "boolean", "description": "Whether to list recursively"},
    "max_depth": {"type": "integer", "minimum": 0, "description": "Maximum recursive depth"}
  },
  "additionalProperties": false
}
param_template:  {"path": "{{path}}"}
preconditions:   "path must be within the active workspace mount"
error_handling:  "path-not-found or permission denied → tool error; output capped at 1 MiB"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

### Step 4.2 — ToolSkill: `ts-list-dir` (class 13)

```
name:        "ts-list-dir"
tool_name:   "list_dir"
description: "Executor binding for list_dir. Lists directory contents through scoped mounts.
              Optional recursive flag and max_depth limit. path defaults to workspace root."
param_schema: {
  "type": "object",
  "properties": {
    "path":      {"type": "string",  "description": "Scoped directory path (omit for workspace root)"},
    "recursive": {"type": "boolean", "description": "Recurse into subdirectories"},
    "max_depth": {"type": "integer", "minimum": 0, "description": "Depth cap for recursive listing"}
  },
  "additionalProperties": false
}
param_template:  {"path": "{{path}}"}
preconditions:   "path within workspace mount scope"
error_handling:  "path-not-found → tool error; permission denied → tool error; output truncated at 1 MiB"
category:        "filesystem"
source:          "system"
validation_status: "validated"
```

### Step 4.3 — PythonCode: `pc-exec-list-dir` (class 22)

```
name:        "pc-exec-list-dir"
description: "Orchestrator executor: calls __execute_action__ to list a directory via
              builtin.list_dir. Input: path (string, omit for workspace root), recursive
              (bool, default false), max_depth (int, optional)."
content: |
  # Orchestrator executor body. __execute_action__ provided by runtime sandbox.
  # IBS bakes in path/recursive/max_depth as slot0/slot1/slot2.
  _path = "{{vars.slot0}}"
  _recursive = {{vars.slot1}}
  _max_depth = {{vars.slot2}}
  _params = {}
  if _path and _path != "":
      _params["path"] = _path
  if _recursive:
      _params["recursive"] = True
  if _max_depth and _max_depth > 0:
      _params["max_depth"] = _max_depth
  result = __execute_action__("list_dir", _params)
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 4.4 — Leaf Skill: `skill-list-dir` (class 1)

> One grain: listing a single directory level.

```
name:        "skill-list-dir"
class_code:  1
description: "Leaf skill: how to list the contents of a single directory."
body: |
  Use `ts-list-dir` (via pc-exec-list-dir) to enumerate the files and folders in a
  directory. Provide the scoped path; omit it to default to the workspace root. The
  result includes entry names, types (file/directory), and sizes. Interpret and present
  the entries relevant to the task. If the listing is large, summarise by grouping.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 4.5 — Leaf Skill: `skill-list-dir-recursive` (class 1)

> Separate grain: recursive directory scan.

```
name:        "skill-list-dir-recursive"
class_code:  1
description: "Leaf skill: how to recursively scan a directory tree."
body: |
  Use `ts-list-dir` with `recursive=true` and a `max_depth` limit when you need to see
  the full subtree of a directory. Keep max_depth at 3 or less for large projects to
  avoid output truncation. If the root listing is too large, narrow the path first.
  For pattern-based searching, skill-glob is more precise.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 4.6 — Recipe: `file-list` (class 21)

> **Tier:** 0 — deterministic, no LLM needed. PythonCode drives the dispatch.

```
name:        "file-list"
description: "List the contents of a directory."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-list-dir>"],
    "label":   "Pre-load ts-list-dir ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-list-dir>"],
    "label":   "PythonCode calls __execute_action__(list_dir, {path, recursive, max_depth})"
  }
]
intent_examples: [
  {"input": "list files in this directory",         "class": 1},
  {"input": "show directory contents",              "class": 1},
  {"input": "what files are in the project root",   "class": 1},
  {"input": "show me what is in this folder",       "class": 1},
  {"input": "ls",                                   "class": 1},
  {"input": "what is in the src directory",         "class": 2},
  {"input": "explore this folder",                  "class": 2},
  {"input": "directory listing",                    "class": 1}
]
source: "system"
validation_status: "validated"
```

---

### Step 4.7 — Recipe: `file-list-recursive` (class 21)

> **Tier:** 0 — deterministic recursive directory scan. One recipe per variant.
> Recursive listing is a different dispatch pattern from shallow listing — the
> orchestrator can route here directly when the intent includes "all files" or "tree".

```
name:        "file-list-recursive"
description: "Recursively list all files and directories under a path."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-list-dir>"],
    "label":   "Pre-load ts-list-dir ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-list-dir>"],
    "label":   "PythonCode calls __execute_action__(list_dir, {path, recursive:true, max_depth:3})"
  }
]
intent_examples: [
  {"input": "list all files recursively",              "class": 1},
  {"input": "show me the full directory tree",         "class": 1},
  {"input": "list all files in this project",          "class": 1},
  {"input": "recursive directory listing",             "class": 1},
  {"input": "show all files and folders",              "class": 1},
  {"input": "tree view of this directory",             "class": 2},
  {"input": "list every file under this path",         "class": 1},
  {"input": "what files exist in this whole project",  "class": 2},
  {"input": "ls -r",                                   "class": 1},
  {"input": "recursive ls",                            "class": 1}
]
source: "system"
validation_status: "validated"
```



## Step 5 — `builtin.glob` (Glob File Search)

> **Capability:** `builtin.glob` · **Effect:** `read_filesystem` · **Permission:** Allow

### Step 5.1 — Tool row (class 0)

```
name:            "glob"
description:     "Find files matching a glob pattern under a scoped root. Returns matching
                  file paths sorted by modification time, capped at max_results."
capability_id:   "builtin.glob"
effect_type:     "read_filesystem"
param_schema: {
  "type": "object",
  "properties": {
    "pattern":     {"type": "string",  "description": "Glob pattern relative to path"},
    "path":        {"type": "string",  "description": "Scoped root path. Defaults to workspace root."},
    "max_results": {"type": "integer", "minimum": 0, "description": "Maximum number of results"}
  },
  "required": ["pattern"],
  "additionalProperties": false
}
param_template:  {"pattern": "{{pattern}}"}
preconditions:   "pattern required; path must be within the active workspace mount"
error_handling:  "invalid pattern or path outside mount → tool error; empty match → empty list"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

### Step 5.2 — ToolSkill: `ts-glob` (class 13)

```
name:        "ts-glob"
tool_name:   "glob"
description: "Executor binding for glob. Required: pattern (glob expression e.g. '**/*.rs').
              Optional: path (scoped root, defaults to workspace root), max_results (cap on
              returned paths). Returns a list of matching paths sorted by modification time."
param_schema: {
  "type": "object",
  "properties": {
    "pattern":     {"type": "string"},
    "path":        {"type": "string"},
    "max_results": {"type": "integer", "minimum": 0}
  },
  "required": ["pattern"],
  "additionalProperties": false
}
param_template:  {"pattern": "{{pattern}}"}
preconditions:   "pattern must not be empty"
error_handling:  "invalid pattern → tool error; empty result → empty list (not an error)"
category:        "filesystem"
source:          "system"
validation_status: "validated"
```

### Step 5.3 — PythonCode: `pc-exec-glob` (class 22)

```
name:        "pc-exec-glob"
description: "Orchestrator executor: calls __execute_action__ to find files via builtin.glob.
              Input: pattern (string), path (optional string), max_results (optional int).
              Output: tool result with list of matching paths."
content: |
  # Orchestrator executor body.
  _pattern = "{{vars.slot0}}"
  _path = "{{vars.slot1}}"
  _max_results = {{vars.slot2}}
  _params = {"pattern": _pattern}
  if _path and _path != "":
      _params["path"] = _path
  if _max_results and _max_results > 0:
      _params["max_results"] = _max_results
  result = __execute_action__("glob", _params)
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 5.4 — Leaf Skill: `skill-glob-by-extension` (class 1)

> One grain: find all files of a specific extension.

```
name:        "skill-glob-by-extension"
class_code:  1
description: "Leaf skill: how to find all files of a specific file extension."
body: |
  Use `ts-glob` with a pattern like `**/*.rs` or `**/*.ts` to find all files of a given
  extension across the workspace. The `**` prefix searches recursively into all
  subdirectories. Use `path` to restrict the search to a specific subdirectory. Use
  `max_results` when you only need a sample.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 5.5 — Leaf Skill: `skill-glob-by-name` (class 1)

> Separate grain: find files by name pattern.

```
name:        "skill-glob-by-name"
class_code:  1
description: "Leaf skill: how to find files by name pattern (not extension)."
body: |
  Use `ts-glob` with a pattern like `**/config*.toml` or `**/README*` to find files
  whose names match a specific pattern. Combine `*` (any chars in one directory level)
  and `**` (any number of directory levels) to build the right pattern. The results
  are sorted by modification time — most recently changed first.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 5.6 — Leaf Skill: `skill-glob-in-subdir` (class 1)

> Separate grain: restrict a glob search to a subdirectory.

```
name:        "skill-glob-in-subdir"
class_code:  1
description: "Leaf skill: how to restrict a glob search to a specific subdirectory."
body: |
  Use `ts-glob` with the `path` parameter set to a specific subdirectory to restrict the
  search scope (e.g. path='src/', pattern='**/*.test.ts'). This is faster and more
  precise than a workspace-root glob when the files of interest are in a known subtree.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 5.7 — Recipe: `file-glob` (class 21)

> **Tier:** 0 — deterministic pattern search, PythonCode drives dispatch.

```
name:        "file-glob"
description: "Find files matching a glob pattern."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-glob>"],
    "label":   "Pre-load ts-glob ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-glob>"],
    "label":   "PythonCode calls __execute_action__(glob, {pattern, path, max_results})"
  }
]
intent_examples: [
  {"input": "find all TypeScript files",              "class": 1},
  {"input": "find files matching *.rs",               "class": 1},
  {"input": "search for test files in src",           "class": 2},
  {"input": "glob pattern **/*.json",                 "class": 1},
  {"input": "find all config files in this repo",     "class": 2},
  {"input": "find all files with this extension",     "class": 1},
  {"input": "list all .py files in the project",      "class": 1},
  {"input": "find files by name pattern",             "class": 2},
  {"input": "glob search",                            "class": 1}
]
source: "system"
validation_status: "validated"
```

### Step 5.8 — Recipe: `file-glob-by-extension` (class 21)

> **Tier:** 0 — find all files of a given extension. One recipe per variant: extension-based
> search is the most common glob use case and deserves its own dedicated recipe.

```
name:        "file-glob-by-extension"
description: "Find all files of a specific file extension (e.g. all .rs or .ts files)."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-glob>"],
    "label":   "Pre-load ts-glob ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-glob>"],
    "label":   "PythonCode calls __execute_action__(glob, {pattern: '**/*.ext'})"
  }
]
intent_examples: [
  {"input": "find all Rust files",                           "class": 1},
  {"input": "list all .ts files",                            "class": 1},
  {"input": "show me all Python files",                      "class": 1},
  {"input": "find every .json config",                       "class": 1},
  {"input": "find all TypeScript files in the project",      "class": 1},
  {"input": "list .rs files",                                "class": 1},
  {"input": "which .md files exist",                         "class": 1},
  {"input": "find all test files by extension",              "class": 2},
  {"input": "list every .toml file in the project",          "class": 1},
  {"input": "show me all YAML files",                        "class": 1}
]
source: "system"
validation_status: "validated"
```

### Step 5.9 — Recipe: `file-glob-by-name` (class 21)

> **Tier:** 0 — find files whose names match a pattern. Variant for name-pattern searches.

```
name:        "file-glob-by-name"
description: "Find files whose names match a glob pattern (e.g. config*.toml, README*)."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-glob>"],
    "label":   "Pre-load ts-glob ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-glob>"],
    "label":   "PythonCode calls __execute_action__(glob, {pattern: '**/name-pattern'})"
  }
]
intent_examples: [
  {"input": "find the Makefile",                              "class": 1},
  {"input": "find all README files",                          "class": 1},
  {"input": "locate the config files",                        "class": 1},
  {"input": "find files named settings*",                     "class": 1},
  {"input": "where is the docker-compose file",               "class": 2},
  {"input": "find all files starting with test_",             "class": 1},
  {"input": "locate any .env files",                          "class": 2},
  {"input": "find files by name pattern",                     "class": 1},
  {"input": "where is the package.json",                      "class": 2},
  {"input": "find all files that start with index",           "class": 1}
]
source: "system"
validation_status: "validated"
```

### Step 5.10 — Recipe: `file-glob-in-subdir` (class 21)

> **Tier:** 0 — restrict glob to a specific subdirectory. Variant for scoped searches.

```
name:        "file-glob-in-subdir"
description: "Find files matching a pattern within a specific subdirectory."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-glob>"],
    "label":   "Pre-load ts-glob ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-glob>"],
    "label":   "PythonCode calls __execute_action__(glob, {pattern, path: subdir})"
  }
]
intent_examples: [
  {"input": "find all test files in the src folder",              "class": 1},
  {"input": "list .ts files in the components directory",          "class": 1},
  {"input": "search for config files in crates/",                  "class": 2},
  {"input": "find .rs files only in the migrations dir",           "class": 2},
  {"input": "glob in a subdirectory",                              "class": 1},
  {"input": "show all Python files under the lib folder",          "class": 2},
  {"input": "find all markdown docs inside the docs directory",    "class": 2},
  {"input": "list all .json files under the config subfolder",     "class": 1},
  {"input": "restrict file search to the tests subdirectory",      "class": 2},
  {"input": "find every YAML file in the deployment directory",    "class": 2}
]
source: "system"
validation_status: "validated"
```

---

## Step 6 — `builtin.grep` (Content Search)

> **Capability:** `builtin.grep` · **Effect:** `read_filesystem` · **Permission:** Allow

### Step 6.1 — Tool row (class 0)

```
name:            "grep"
description:     "Search file contents using a regular expression within scoped mounts.
                  Supports content, files_with_matches, and count output modes. Optional
                  glob filter, context lines, case-insensitive matching, and result pagination."
capability_id:   "builtin.grep"
effect_type:     "read_filesystem"
param_schema: {
  "type": "object",
  "properties": {
    "pattern":         {"type": "string",  "description": "Regular expression to search for"},
    "path":            {"type": "string",  "description": "Scoped file or directory path. Defaults to workspace root."},
    "glob":            {"type": "string",  "description": "Optional glob filter relative to path"},
    "output_mode":     {"type": "string",  "enum": ["content","files_with_matches","count"],
                        "description": "Output mode. Defaults to files_with_matches."},
    "case_insensitive":{"type": "boolean"},
    "multiline":       {"type": "boolean"},
    "context":         {"type": "integer", "minimum": 0},
    "before_context":  {"type": "integer", "minimum": 0},
    "after_context":   {"type": "integer", "minimum": 0},
    "head_limit":      {"type": "integer", "minimum": 0},
    "offset":          {"type": "integer", "minimum": 0}
  },
  "required": ["pattern"],
  "additionalProperties": false
}
param_template:  {"pattern": "{{pattern}}"}
preconditions:   "pattern required; path must be within the active workspace mount"
error_handling:  "invalid regex → tool error; empty results → empty list; output truncated at 1 MiB"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

### Step 6.2 — ToolSkill: `ts-grep` (class 13)

```
name:        "ts-grep"
tool_name:   "grep"
description: "Executor binding for grep. Required: pattern (regex). Optional: path (scoped
              file or directory, defaults to workspace root), glob (file filter), output_mode
              (content | files_with_matches | count, default files_with_matches),
              case_insensitive, context/before_context/after_context (lines of context),
              head_limit (cap results), offset (pagination start)."
param_schema: {
  "type": "object",
  "properties": {
    "pattern":         {"type": "string"},
    "path":            {"type": "string"},
    "glob":            {"type": "string"},
    "output_mode":     {"type": "string", "enum": ["content","files_with_matches","count"]},
    "case_insensitive":{"type": "boolean"},
    "multiline":       {"type": "boolean"},
    "context":         {"type": "integer", "minimum": 0},
    "before_context":  {"type": "integer", "minimum": 0},
    "after_context":   {"type": "integer", "minimum": 0},
    "head_limit":      {"type": "integer", "minimum": 0},
    "offset":          {"type": "integer", "minimum": 0}
  },
  "required": ["pattern"],
  "additionalProperties": false
}
param_template:  {"pattern": "{{pattern}}"}
preconditions:   "pattern must be a valid regex"
error_handling:  "invalid regex → tool error; no matches → empty result; output capped at 1 MiB"
category:        "filesystem"
source:          "system"
validation_status: "validated"
```

### Step 6.3 — PythonCode: `pc-exec-grep` (class 22)

```
name:        "pc-exec-grep"
description: "Orchestrator executor: calls __execute_action__ to search content via
              builtin.grep. Input: pattern (string), path (optional), output_mode (optional,
              default files_with_matches), glob (optional), case_insensitive (optional bool)."
content: |
  # Orchestrator executor body.
  _pattern = "{{vars.slot0}}"
  _path = "{{vars.slot1}}"
  _output_mode = "{{vars.slot2}}"
  _glob = "{{vars.slot3}}"
  _case_insensitive = {{vars.slot4}}
  _params = {"pattern": _pattern}
  if _path and _path != "":
      _params["path"] = _path
  if _output_mode and _output_mode != "":
      _params["output_mode"] = _output_mode
  if _glob and _glob != "":
      _params["glob"] = _glob
  if _case_insensitive:
      _params["case_insensitive"] = True
  result = __execute_action__("grep", _params)
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 6.4 — Leaf Skill: `skill-grep-files` (class 1)

> One grain: find which files contain a pattern (files_with_matches mode).

```
name:        "skill-grep-files"
class_code:  1
description: "Leaf skill: how to find which files contain a regex pattern."
body: |
  Use `ts-grep` with `output_mode='files_with_matches'` when you only need to know
  WHICH files contain the pattern — not the matching lines. This is the fastest mode
  and produces compact output. Use `glob` to restrict the file types searched (e.g.
  glob='*.rs' to search only Rust files). Use `case_insensitive=true` when the match
  should be case-independent.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 6.5 — Leaf Skill: `skill-grep-content` (class 1)

> Separate grain: find matching lines with context (content mode).

```
name:        "skill-grep-content"
class_code:  1
description: "Leaf skill: how to retrieve matching lines (with context) from files."
body: |
  Use `ts-grep` with `output_mode='content'` when you need the actual matching lines,
  not just which files match. Add `context` (symmetric) or `before_context`/`after_context`
  (asymmetric) to include surrounding lines — useful when the surrounding code helps
  understand the match. Use `head_limit` to cap the number of results when the pattern
  appears frequently. Use `offset` to paginate through large result sets.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 6.6 — Leaf Skill: `skill-grep-count` (class 1)

> Separate grain: count occurrences without returning content.

```
name:        "skill-grep-count"
class_code:  1
description: "Leaf skill: how to count pattern occurrences without returning the matching lines."
body: |
  Use `ts-grep` with `output_mode='count'` when you only need to know how many times
  a pattern appears, not the actual lines. This is efficient for large codebases where
  you want a frequency signal (e.g. how many TODO comments exist) without reading all
  the matching content. The result contains per-file counts.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 6.7 — Recipe: `file-grep` (class 21)

> **Tier:** 0 — deterministic content search, PythonCode drives dispatch.

```
name:        "file-grep"
description: "Search file contents using a regular expression."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-grep>"],
    "label":   "Pre-load ts-grep ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-grep>"],
    "label":   "PythonCode calls __execute_action__(grep, {pattern, path, output_mode, ...})"
  }
]
intent_examples: [
  {"input": "find all uses of function foo",          "class": 1},
  {"input": "search for TODO comments in src",        "class": 1},
  {"input": "which files import React",               "class": 1},
  {"input": "find all occurrences of FIXME",          "class": 1},
  {"input": "grep this pattern",                      "class": 1},
  {"input": "search for this string in the codebase", "class": 1},
  {"input": "find files containing this text",        "class": 1},
  {"input": "how many places use this function",      "class": 2}
]
source: "system"
validation_status: "validated"
```

### Step 6.8 — Recipe: `file-grep-files` (class 21)

> **Tier:** 0 — find WHICH files contain a pattern. One recipe per output mode.
> `files_with_matches` mode: compact, fast, returns only file paths.

```
name:        "file-grep-files"
description: "Find which files contain a regex pattern (returns file paths only, no line content)."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-grep>"],
    "label":   "Pre-load ts-grep ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-grep>"],
    "label":   "PythonCode calls __execute_action__(grep, {pattern, output_mode: 'files_with_matches'})"
  }
]
intent_examples: [
  {"input": "which files use this function",                  "class": 1},
  {"input": "which files import this module",                 "class": 1},
  {"input": "find files containing this string",              "class": 1},
  {"input": "which files have TODO",                          "class": 1},
  {"input": "what files reference this constant",             "class": 1},
  {"input": "show me files with this error pattern",          "class": 2},
  {"input": "which .rs files contain async",                  "class": 2},
  {"input": "find files matching this regex",                 "class": 1},
  {"input": "list every file that has this keyword",          "class": 1},
  {"input": "show me all files that define this class",       "class": 2}
]
source: "system"
validation_status: "validated"
```

### Step 6.9 — Recipe: `file-grep-content` (class 21)

> **Tier:** 0 — find matching LINES with context. One recipe per output mode.
> `content` mode: returns actual matching lines + surrounding context lines.

```
name:        "file-grep-content"
description: "Search file contents and return matching lines with surrounding context."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-grep>"],
    "label":   "Pre-load ts-grep ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-grep>"],
    "label":   "PythonCode calls __execute_action__(grep, {pattern, output_mode: 'content', context})"
  }
]
intent_examples: [
  {"input": "show me the lines that contain this error",          "class": 1},
  {"input": "find all uses of this function with context",        "class": 1},
  {"input": "search for this pattern and show surrounding code",  "class": 1},
  {"input": "grep with context lines",                            "class": 1},
  {"input": "find this variable declaration",                     "class": 2},
  {"input": "show matching lines in the source files",            "class": 1},
  {"input": "grep content mode",                                  "class": 1},
  {"input": "see the code around each match",                     "class": 2},
  {"input": "show 3 lines before and after each match",           "class": 2},
  {"input": "grep and show surrounding context",                  "class": 1}
]
source: "system"
validation_status: "validated"
```

### Step 6.10 — Recipe: `file-grep-count` (class 21)

> **Tier:** 0 — count occurrences only. One recipe per output mode.
> `count` mode: compact per-file occurrence counts, no line content returned.

```
name:        "file-grep-count"
description: "Count occurrences of a pattern across files without returning the matching lines."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-grep>"],
    "label":   "Pre-load ts-grep ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-grep>"],
    "label":   "PythonCode calls __execute_action__(grep, {pattern, output_mode: 'count'})"
  }
]
intent_examples: [
  {"input": "how many TODO comments are there",               "class": 1},
  {"input": "count occurrences of this pattern",              "class": 1},
  {"input": "how many times does this appear",                "class": 1},
  {"input": "count all uses of this function",                "class": 2},
  {"input": "how many errors in these log files",             "class": 2},
  {"input": "count grep matches",                             "class": 1},
  {"input": "how many files contain this string",             "class": 2},
  {"input": "give me a count not the lines themselves",       "class": 1},
  {"input": "how many FIXME markers in the codebase",        "class": 2},
  {"input": "count how many times this import appears",      "class": 2}
]
source: "system"
validation_status: "validated"
```

---

## Step 7 — `builtin.apply_patch` (Targeted File Edit)

> **Capability:** `builtin.apply_patch` · **Effect:** `mixed` · **Permission:** Ask
> Input cap: 21 MiB. Tier 1 always — the LLM must compose old_string and new_string.

### Step 7.1 — Tool row (class 0)

```
name:            "apply_patch"
description:     "Apply a targeted search-replace edit to a scoped file. Finds old_string
                  in the file and replaces it with new_string. Exact match required by default;
                  replace_all replaces every occurrence. Reads and writes through scoped mounts."
capability_id:   "builtin.apply_patch"
effect_type:     "mixed"
param_schema: {
  "type": "object",
  "properties": {
    "path":        {"type": "string",  "description": "Scoped file path to patch"},
    "old_string":  {"type": "string",  "description": "Exact text to replace"},
    "new_string":  {"type": "string",  "description": "Replacement text"},
    "replace_all": {"type": "boolean", "description": "Replace every match instead of exactly one"}
  },
  "required": ["path", "old_string", "new_string"],
  "additionalProperties": false
}
param_template:  {"path": "{{path}}", "old_string": "{{old_string}}", "new_string": "{{new_string}}"}
preconditions:   "path within workspace mount scope; old_string must appear exactly once unless replace_all"
error_handling:  "old_string not found → tool error; multiple matches without replace_all → tool error"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

### Step 7.2 — ToolSkill: `ts-apply-patch` (class 13)

```
name:        "ts-apply-patch"
tool_name:   "apply_patch"
description: "Executor binding for apply_patch. Required: path (scoped file), old_string
              (exact text to replace), new_string (replacement). Optional: replace_all
              (replaces every occurrence; default: exactly one match, error if multiple).
              old_string must include enough surrounding context to be unique in the file."
param_schema: {
  "type": "object",
  "properties": {
    "path":        {"type": "string"},
    "old_string":  {"type": "string"},
    "new_string":  {"type": "string"},
    "replace_all": {"type": "boolean"}
  },
  "required": ["path", "old_string", "new_string"],
  "additionalProperties": false
}
param_template:  {"path": "{{path}}", "old_string": "{{old_string}}", "new_string": "{{new_string}}"}
preconditions:   "old_string must be unique in file unless replace_all is set; path within mount scope"
error_handling:  "not-found → tool error; ambiguous match without replace_all → tool error"
category:        "filesystem"
source:          "system"
validation_status: "validated"
```

### Step 7.3 — Leaf Skill: `skill-apply-patch-single` (class 1)

> One grain: replace exactly one unique occurrence.

```
name:        "skill-apply-patch-single"
class_code:  1
description: "Leaf skill: how to replace a single unique occurrence in a file."
body: |
  Use `ts-apply-patch` with a unique `old_string` to replace exactly one occurrence of
  text in a file. old_string must include enough surrounding lines (3–5) to be unambiguous.
  If the string appears more than once, the tool will error — use skill-apply-patch-all
  instead, or narrow old_string to include unique context. Always read the file first with
  skill-read-file when uncertain of the exact current text.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 7.4 — Leaf Skill: `skill-apply-patch-all` (class 1)

> Separate grain: replace all occurrences.

```
name:        "skill-apply-patch-all"
class_code:  1
description: "Leaf skill: how to replace every occurrence of a string in a file."
body: |
  Use `ts-apply-patch` with `replace_all=true` when the same string appears multiple
  times and ALL occurrences should be changed (e.g. renaming a symbol throughout a file).
  Verify the replacement is correct for ALL occurrences before dispatching — this is
  irreversible without re-reading and re-patching.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 7.5 — Recipe: `file-patch` (class 21)

> **Tier:** 1 — LLM must compose old_string and new_string from file content.

```
name:        "file-patch"
description: "Apply a targeted search-replace edit to a file."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-read-file>", "<uuid:skill-apply-patch-single>"],
    "label":   "Load read + patch leaf skill context"
  },
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-read-file>"],
    "label":   "Pre-load ts-read-file binding"
  },
  {
    "step_id": "step-2",
    "type":    "llm",
    "label":   "LLM reads file, determines exact old_string and new_string for the change"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-apply-patch>"],
    "label":   "Pre-load ts-apply-patch binding"
  }
]
intent_examples: [
  {"input": "fix this bug in the function",              "class": 3},
  {"input": "rename variable foo to bar in utils",       "class": 3},
  {"input": "update the default timeout value",          "class": 2},
  {"input": "replace the old error message",             "class": 2},
  {"input": "apply patch to file",                       "class": 2},
  {"input": "edit this line in the file",                "class": 2},
  {"input": "change this string to something else",      "class": 2},
  {"input": "search and replace in this file",           "class": 2},
  {"input": "patch this specific section of the file",   "class": 2},
  {"input": "make a targeted edit to this file",         "class": 2}
]
source: "system"
validation_status: "validated"
```

---

## Step 7.x — Domain Skill `skill-filesystem` (class 2)

> References all filesystem leaf skills by name. No duplicated content.

```
name:        "skill-filesystem"
class_code:  2
description: "The filesystem domain provides six scoped tools for working with the workspace.
              Decision guide — use the right skill for each approach:

              READING:
              — skill-read-file: Read a file's full content.
              — skill-read-file-range: Read a specific line range from a large file.

              LISTING / FINDING:
              — skill-list-dir: List contents of a single directory level.
              — skill-list-dir-recursive: Recursively scan a directory tree.
              — skill-list-dir-files-only: List only regular files (no subdirs).
              — skill-list-dir-dirs-only: List only subdirectories.
              — skill-glob-by-extension: Find all files of a given extension.
              — skill-glob-by-name: Find files whose names match a pattern.
              — skill-glob-in-subdir: Restrict a glob to a specific subdirectory.

              SEARCHING CONTENT:
              — skill-grep-files: Find which files contain a pattern (fast, compact output).
              — skill-grep-content: Retrieve matching lines with surrounding context.
              — skill-grep-count: Count occurrences without returning content.
              — skill-grep-case-insensitive: Case-insensitive grep (add case_insensitive=true).
              — skill-grep-type-filtered: Grep only specific file types via glob filter.
              — skill-grep-invert: Find files/lines that do NOT match (invert_match=true).

              Decision for grep approach:
              • Which files contain pattern → skill-grep-files
              • What exactly matches with context → skill-grep-content
              • How many occurrences → skill-grep-count
              • Pattern in any case → skill-grep-case-insensitive
              • Only in .rs / .ts / etc. files → skill-grep-type-filtered
              • Files MISSING a pattern → skill-grep-invert

              WRITING / EDITING:
              — skill-write-file-new: Create a new file with full content.
              — skill-write-file-replace: Replace an existing file's entire content.
              — skill-write-file-template: Write a file from pre-baked template vars (no LLM).
              — skill-apply-patch-single: Replace one unique occurrence in a file.
              — skill-apply-patch-all: Replace every occurrence of a string in a file.

              All paths are scoped to the workspace mount. Output is capped at 1 MiB per call."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```


## Step 8 — `builtin.http` (HTTP Request, Inline Response)

> **Capability:** `builtin.http` · **Effect:** `network_egress` · **Permission:** Ask
> Timeout: 30 s · Response body cap: 256 KiB inline.

### Step 8.1 — Tool row (class 0)

```
name:            "http"
description:     "Perform an HTTP or HTTPS request and return the response inline. Supports
                  GET, POST, PUT, PATCH, DELETE, HEAD. Response body capped at 256 KiB inline;
                  larger responses should use builtin.http.save."
capability_id:   "builtin.http"
effect_type:     "network_egress"
param_schema: {
  "type": "object",
  "properties": {
    "url":                 {"type": "string",  "description": "Absolute HTTP or HTTPS URL"},
    "method":              {"type": "string",  "enum": ["get","post","put","patch","delete","head"],
                            "description": "HTTP method. Defaults to get."},
    "headers":             {"description": "HTTP headers as an object or [{name,value}] array"},
    "body":                {"description": "String or JSON request body"},
    "body_base64":         {"type": "string",  "description": "Base64-encoded request body"},
    "response_body_limit": {"type": "integer", "minimum": 1, "maximum": 262144,
                            "description": "Max inline response bytes, capped at 256 KiB."},
    "timeout_ms":          {"type": "integer", "minimum": 1, "maximum": 30000, "default": 10000}
  },
  "required": ["url"],
  "additionalProperties": false
}
param_template:  {"url": "{{url}}"}
preconditions:   "url must be absolute http/https; network egress must be permitted by policy"
error_handling:  "connection failure → tool error; body over limit → truncated with guidance; non-2xx → in output (not a tool error)"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

### Step 8.2 — ToolSkill: `ts-http-fetch` (class 13)

```
name:        "ts-http-fetch"
tool_name:   "http"
description: "Executor binding for builtin.http. Required: url. Optional: method (default
              get), headers, body, body_base64, response_body_limit (max 256 KiB),
              timeout_ms (max 30 000). Non-2xx status codes are returned in output — not errors."
param_schema: {
  "type": "object",
  "properties": {
    "url":                 {"type": "string"},
    "method":              {"type": "string", "enum": ["get","post","put","patch","delete","head"]},
    "headers":             {},
    "body":                {},
    "body_base64":         {"type": "string"},
    "response_body_limit": {"type": "integer", "minimum": 1, "maximum": 262144},
    "timeout_ms":          {"type": "integer", "minimum": 1, "maximum": 30000}
  },
  "required": ["url"],
  "additionalProperties": false
}
param_template:  {"url": "{{url}}"}
preconditions:   "url must begin with http:// or https://"
error_handling:  "network failure → tool error; body truncation → status field in output; timeout_ms capped at 30 000"
category:        "network"
source:          "system"
validation_status: "validated"
```

### Step 8.3 — PythonCode: `pc-exec-http-get` (class 22)

> Orchestrator executor for simple GET requests. The most common HTTP use case.

```
name:        "pc-exec-http-get"
description: "Orchestrator executor: calls __execute_action__ for an HTTP GET request via
              builtin.http. Input: url (string), response_body_limit (optional int).
              Output: tool result with status, headers, body."
content: |
  # Orchestrator executor body.
  _url = "{{vars.slot0}}"
  _limit = {{vars.slot1}}
  _params = {"url": _url, "method": "get"}
  if _limit and _limit > 0:
      _params["response_body_limit"] = _limit
  result = __execute_action__("http", _params)
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 8.4 — PythonCode: `pc-exec-http-post` (class 22)

> Orchestrator executor for POST requests with a JSON body.

```
name:        "pc-exec-http-post"
description: "Orchestrator executor: calls __execute_action__ for an HTTP POST request via
              builtin.http. Input: url (string), body (JSON value), headers (optional dict).
              Output: tool result with status, headers, body."
content: |
  # Orchestrator executor body.
  _url = "{{vars.slot0}}"
  _body = {{vars.slot1}}
  _headers = {{vars.slot2}}
  _params = {"url": _url, "method": "post", "body": _body}
  if _headers:
      _params["headers"] = _headers
  result = __execute_action__("http", _params)
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 8.5 — Leaf Skill: `skill-http-get` (class 1)

> One grain: fetch a URL via GET.

```
name:        "skill-http-get"
class_code:  1
description: "Leaf skill: how to fetch a URL via HTTP GET and receive the response inline."
body: |
  Use `ts-http-fetch` with method='get' (via pc-exec-http-get) to fetch a URL and receive
  the response body inline. The body is capped at 256 KiB. If a larger response is needed,
  use skill-http-save instead. Non-2xx status codes appear in the result's status field —
  they are not tool errors. Always inspect the status code after the call.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 8.6 — Leaf Skill: `skill-http-post` (class 1)

> Separate grain: POST with a body.

```
name:        "skill-http-post"
class_code:  1
description: "Leaf skill: how to make an HTTP POST request with a body."
body: |
  Use `ts-http-fetch` with method='post' and a `body` (string or JSON) to submit data
  to an API or webhook. Add an `Authorization` or `Content-Type` header when required.
  For JSON bodies the server typically expects `Content-Type: application/json`. Non-2xx
  responses are not tool errors — check the status field and handle error responses.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 8.7 — Leaf Skill: `skill-http-authenticated` (class 1)

> Separate grain: authenticated requests with bearer tokens or API keys.

```
name:        "skill-http-authenticated"
class_code:  1
description: "Leaf skill: how to make an authenticated HTTP request."
body: |
  Use `ts-http-fetch` with a `headers` parameter to attach authentication. Common patterns:
  - Bearer token: headers={'Authorization': 'Bearer <token>'}
  - API key: headers={'X-Api-Key': '<key>'}
  - Basic auth: headers={'Authorization': 'Basic <base64(user:pass)>'}
  Never hardcode credentials in the skill body — always receive them from the session
  context or memory. Use skill-http-get or skill-http-post as the base dispatch pattern.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 8.8 — Recipe: `http-get` (class 21)

> **Tier:** 0 — deterministic GET dispatch, PythonCode drives execution.

```
name:        "http-get"
description: "Fetch a URL via HTTP GET and return the response."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-http-fetch>"],
    "label":   "Pre-load ts-http-fetch ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-http-get>"],
    "label":   "PythonCode calls __execute_action__(http, {url, method:get})"
  }
]
intent_examples: [
  {"input": "fetch this URL",                         "class": 1},
  {"input": "GET https://api.example.com/data",       "class": 1},
  {"input": "download the JSON from this endpoint",   "class": 1},
  {"input": "make an HTTP GET request",               "class": 1},
  {"input": "check if this URL is reachable",         "class": 1},
  {"input": "fetch the contents of this page",        "class": 1},
  {"input": "HTTP GET this endpoint",                 "class": 1},
  {"input": "call this REST API endpoint",            "class": 2},
  {"input": "retrieve data from this URL",            "class": 1},
  {"input": "ping this endpoint",                     "class": 2}
]
source: "system"
validation_status: "validated"
```

### Step 8.9 — Recipe: `http-get-json` (class 21)

> **Tier:** 0 — deterministic JSON GET with correct Accept header. Variant for API calls
> that specifically return JSON. The intent pattern "call a JSON API" is distinct from
> generic "fetch a URL" and warrants its own recipe with the right header preset.

```
name:        "http-get-json"
description: "Fetch a JSON API endpoint via HTTP GET with Accept: application/json header."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-http-fetch>"],
    "label":   "Pre-load ts-http-fetch ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-http-get>"],
    "label":   "PythonCode calls __execute_action__(http, {url, method:get, headers:{Accept:application/json}})"
  }
]
intent_examples: [
  {"input": "call this JSON API",                     "class": 1},
  {"input": "fetch JSON from this endpoint",          "class": 1},
  {"input": "GET this REST API and parse JSON",        "class": 1},
  {"input": "retrieve JSON data from this URL",        "class": 1},
  {"input": "call the GitHub API",                     "class": 2},
  {"input": "fetch the OpenAPI spec",                  "class": 2},
  {"input": "GET this webhook URL and parse result",   "class": 2},
  {"input": "HTTP GET with JSON accept header",        "class": 1}
]
source: "system"
validation_status: "validated"
```

### Step 8.10 — Recipe: `http-post` (class 21)

> **Tier:** 1 — LLM must compose the POST URL, headers, and body from user intent.
> POST requests are inherently variable: body content, URL, and headers all need
> to be constructed from user instructions — this cannot be Tier 0 for open-ended calls.

```
name:        "http-post"
description: "Send an HTTP POST request with a JSON body."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-http-post>", "<uuid:skill-http-authenticated>"],
    "label":   "Load http-post + auth leaf skill context"
  },
  {
    "step_id": "step-1",
    "type":    "llm",
    "label":   "LLM constructs the POST URL, headers, and body from user instructions"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-http-fetch>"],
    "label":   "Pre-load ts-http-fetch binding"
  }
]
intent_examples: [
  {"input": "POST this data to the API",           "class": 1},
  {"input": "send a webhook notification",         "class": 1},
  {"input": "submit a form to this endpoint",      "class": 2},
  {"input": "call this API with a JSON body",      "class": 2},
  {"input": "create a GitHub issue via API",       "class": 2},
  {"input": "HTTP POST to this endpoint",          "class": 1},
  {"input": "send JSON payload to webhook",        "class": 1},
  {"input": "POST request with body",              "class": 1}
]
source: "system"
validation_status: "validated"
```

---

## Step 9 — `builtin.http.save` (HTTP Request, Response Saved to File)

> **Capability:** `builtin.http.save` · **Effect:** `network_egress` + `write_filesystem` · **Permission:** Ask
> Timeout: 30 s · Response body cap: 10 MiB saved.

### Step 9.1 — Tool row (class 0)

```
name:            "http.save"
description:     "Perform an HTTP or HTTPS request and save the sanitized response body to a
                  scoped file path. Accepts up to 10 MiB of response body. Used when the
                  response is too large for inline delivery or must be persisted."
capability_id:   "builtin.http.save"
effect_type:     "mixed"
param_schema: {
  "type": "object",
  "properties": {
    "url":                 {"type": "string",  "description": "Absolute HTTP or HTTPS URL"},
    "save_to":             {"type": "string",  "description": "Scoped path to save the response body"},
    "method":              {"type": "string",  "enum": ["get","post","put","patch","delete","head"]},
    "headers":             {"description": "HTTP headers as an object or [{name,value}] array"},
    "body":                {"description": "String or JSON request body"},
    "body_base64":         {"type": "string"},
    "response_body_limit": {"type": "integer", "minimum": 1, "maximum": 10485760,
                            "description": "Max response body bytes to save. Default 10 MiB."},
    "timeout_ms":          {"type": "integer", "minimum": 1, "maximum": 30000, "default": 10000}
  },
  "required": ["url", "save_to"],
  "additionalProperties": false
}
param_template:  {"url": "{{url}}", "save_to": "{{save_to}}"}
preconditions:   "url must be absolute http/https; save_to must be within workspace mount"
error_handling:  "connection failure → tool error; save_to outside mount → tool error"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

### Step 9.2 — ToolSkill: `ts-http-save` (class 13)

```
name:        "ts-http-save"
tool_name:   "http.save"
description: "Executor binding for builtin.http.save. Required: url, save_to (scoped path).
              Optional: method, headers, body, body_base64, response_body_limit (default and
              max 10 MiB), timeout_ms (max 30 000). Returns metadata (status, bytes_saved)."
param_schema: {
  "type": "object",
  "properties": {
    "url":                 {"type": "string"},
    "save_to":             {"type": "string"},
    "method":              {"type": "string", "enum": ["get","post","put","patch","delete","head"]},
    "headers":             {},
    "body":                {},
    "body_base64":         {"type": "string"},
    "response_body_limit": {"type": "integer", "minimum": 1, "maximum": 10485760},
    "timeout_ms":          {"type": "integer", "minimum": 1, "maximum": 30000}
  },
  "required": ["url", "save_to"],
  "additionalProperties": false
}
param_template:  {"url": "{{url}}", "save_to": "{{save_to}}"}
preconditions:   "save_to within workspace mount scope"
error_handling:  "network failure → tool error; save_to outside mount → tool error"
category:        "network"
source:          "system"
validation_status: "validated"
```

### Step 9.3 — PythonCode: `pc-exec-http-save` (class 22)

```
name:        "pc-exec-http-save"
description: "Orchestrator executor: calls __execute_action__ for builtin.http.save.
              Input: url (string), save_to (string — scoped path). Output: metadata dict
              with status code and bytes_saved."
content: |
  # Orchestrator executor body.
  _url = "{{vars.slot0}}"
  _save_to = "{{vars.slot1}}"
  result = __execute_action__("http.save", {"url": _url, "save_to": _save_to})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 9.4 — Leaf Skill: `skill-http-save-download` (class 1)

> One grain: download and save a URL response to a file.

```
name:        "skill-http-save-download"
class_code:  1
description: "Leaf skill: how to download an HTTP response and save it to a file."
body: |
  Use `ts-http-save` (via pc-exec-http-save) when the expected response exceeds 256 KiB
  or when the content must be persisted to disk. Provide the url and a scoped save_to
  path. After the call, use skill-read-file to inspect the saved content or report the
  file path to the user. The response is saved without decoding binary content.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 9.5 — Leaf Skill: `skill-http-save-api` (class 1)

> Separate grain: save a large API response for later processing.

```
name:        "skill-http-save-api"
class_code:  1
description: "Leaf skill: how to save a large API response for subsequent parsing."
body: |
  When an API returns more data than can be processed inline (>256 KiB), use
  `ts-http-save` to write the full response to a temp file, then use skill-read-file
  or pc-json-extract-field to extract the needed fields from the saved file. This is
  the recommended pattern for paginated or bulk API responses.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 9.6 — Recipe: `http-save` (class 21)

> **Tier:** 0 — deterministic save dispatch, PythonCode drives execution.

```
name:        "http-save"
description: "Fetch a URL and save the response body to a file."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-http-save>"],
    "label":   "Pre-load ts-http-save ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-http-save>"],
    "label":   "PythonCode calls __execute_action__(http.save, {url, save_to})"
  }
]
intent_examples: [
  {"input": "download this file and save it",            "class": 1},
  {"input": "fetch the API response and write to disk",  "class": 1},
  {"input": "save the download to workspace",            "class": 1},
  {"input": "GET this URL and save the result",          "class": 1},
  {"input": "download a large JSON response",            "class": 1},
  {"input": "save this URL response to a file",          "class": 1},
  {"input": "download the binary and store it",          "class": 2},
  {"input": "fetch and persist this large response",     "class": 1},
  {"input": "save API result to workspace file",         "class": 1}
]
source: "system"
validation_status: "validated"
```

---

## Step 9.x — HTTP Domain Skill + PythonCode Helpers

### Step 9.x.0 — Additional HTTP Variant: PATCH method

> `http-patch` covers the PATCH HTTP method — partial resource update. Distinct from PUT
> (full replacement). The PATCH body is partial: only the fields to be updated. Tier 1
> because the LLM must compose the partial update body from user intent.

#### Leaf Skill: `skill-http-patch` (class 1)

```
name:        "skill-http-patch"
class_code:  1
description: "Leaf skill: how to make an HTTP PATCH request for partial resource update."
body: |
  Use `ts-http-fetch` with method='patch' and a `body` containing only the fields to
  update (via a custom pc-exec-http-patch-like call). PATCH is idempotent partial update
  — unlike PUT which replaces the full resource. Include Content-Type: application/json
  and Authorization headers when required. Non-2xx responses are not tool errors.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

#### PythonCode: `pc-exec-http-patch` (class 22)

```
name:        "pc-exec-http-patch"
description: "Orchestrator executor: calls __execute_action__ for an HTTP PATCH request via
              builtin.http. Input: url (string), body (JSON value), headers (optional dict).
              Output: status + body."
content: |
  # Orchestrator executor body.
  _url = "{{vars.slot0}}"
  _body = {{vars.slot1}}
  _headers = {{vars.slot2}}
  _params = {"url": _url, "method": "patch", "body": _body}
  if _headers:
      _params["headers"] = _headers
  result = __execute_action__("http", _params)
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

#### Recipe: `http-patch` (class 21)

> **Tier:** 1 — LLM must compose the PATCH URL, headers, and partial update body.

```
name:        "http-patch"
description: "Send an HTTP PATCH request to partially update a resource."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-http-patch>", "<uuid:skill-http-authenticated>"],
    "label":   "Load http-patch + auth leaf skill context"
  },
  {
    "step_id": "step-1",
    "type":    "llm",
    "label":   "LLM constructs the PATCH URL, headers, and partial update body from user instructions"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-http-fetch>"],
    "label":   "Pre-load ts-http-fetch binding"
  }
]
intent_examples: [
  {"input": "partially update this resource via PATCH",        "class": 1},
  {"input": "PATCH request to update one field",               "class": 1},
  {"input": "send a PATCH to change the status field",         "class": 2},
  {"input": "HTTP PATCH this endpoint",                        "class": 1},
  {"input": "update this resource partially via REST",         "class": 2},
  {"input": "patch this record with new values",               "class": 2},
  {"input": "PATCH the user email in the API",                 "class": 2},
  {"input": "partial update via HTTP PATCH",                   "class": 1}
]
source: "system"
validation_status: "validated"
```

---

### Step 9.x.0b — Additional HTTP-Save Variant: Large Response

> `http-save-large` explicitly caps the response at 5 MiB. Useful when fetching large
> datasets that could otherwise silently truncate. Tier 0 — slots supply url and path.

#### Recipe: `http-save-large` (class 21)

> **Tier:** 0 — deterministic large-response save with explicit 5 MiB cap.

```
name:        "http-save-large"
description: "Fetch a URL and save up to 5 MiB of the response body to a file."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-http-save>"],
    "label":   "Pre-load ts-http-save ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-http-save>"],
    "label":   "PythonCode calls __execute_action__(http.save, {url, save_to, response_body_limit: 5242880})"
  }
]
intent_examples: [
  {"input": "download a large file and save it",               "class": 1},
  {"input": "fetch a large API response and store it",         "class": 1},
  {"input": "download this dataset to workspace",              "class": 2},
  {"input": "save a large response up to 5MB",                 "class": 2},
  {"input": "fetch and save this big response body",           "class": 1},
  {"input": "download and persist this large JSON dataset",    "class": 2},
  {"input": "http save large response",                        "class": 1},
  {"input": "save 5mb response body to file",                  "class": 1}
]
source: "system"
validation_status: "validated"
```

---

### Step 9.x.1 — Domain Skill `skill-http` (class 2)

```
name:        "skill-http"
class_code:  2
description: "The HTTP domain provides two tools for outbound HTTP requests:

              INLINE RESPONSE (≤256 KiB):
              — skill-http-get: GET request, response inline.
              — skill-http-post: POST request with body, response inline.
              — skill-http-authenticated: Any method with auth headers.
              — skill-http-head: HEAD request — metadata only, no body.
              — skill-http-put: PUT request — full resource replacement.
              — skill-http-patch: PATCH request — partial resource update.
              — skill-http-delete: DELETE request — remove a resource.

              SAVED RESPONSE (>256 KiB or must persist):
              — skill-http-save-download: Download and save to a workspace file.
              — skill-http-save-api: Save a large API response for later parsing.

              Decision guide:
              • Small response needed immediately → skill-http-get
              • POST with body → skill-http-post
              • Authenticated request → skill-http-authenticated (combine with above)
              • Existence/metadata check only → skill-http-head (no body returned)
              • Full resource replace → skill-http-put
              • Partial update → skill-http-patch
              • Delete a resource → skill-http-delete
              • Response >256 KiB or must be saved → skill-http-save-download
              • Large API response for later parsing → skill-http-save-api

              Non-2xx HTTP responses are NOT tool errors. Always inspect the status field.
              Use pc-http-status-check to test success programmatically."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 9.x.2 — PythonCode `pc-http-status-check` (class 22)

> Pure logic: takes a status code, returns a success boolean.

```
name:        "pc-http-status-check"
description: "Pure-logic helper: returns True when the HTTP status code indicates success
              (2xx range), False otherwise. Input: status_code (integer). Output: {is_success,
              status_code}."
content: |
  # No I/O, no imports. IBS bakes in status_code as {{vars.slot0}} before execution.
  status_code = {{vars.slot0}}
  is_success = 200 <= status_code < 300
  result = {"is_success": is_success, "status_code": status_code}
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 9.x.3 — PythonCode `pc-json-extract-field` (class 22)

> Pure logic: dot-path field extractor. No I/O, no imports.

```
name:        "pc-json-extract-field"
description: "Pure-logic helper: extracts a value from a JSON object by dot-separated path.
              Input: data (dict), path (dot-separated string e.g. 'result.items.0').
              Output: {value, path, found}."
content: |
  # No I/O, no imports. IBS bakes in 'data' and 'path' before execution.
  data = {{vars.slot0}}
  path = "{{vars.slot1}}"
  parts = path.split(".")
  current = data
  for part in parts:
      if isinstance(current, dict) and part in current:
          current = current[part]
      elif isinstance(current, list):
          try:
              current = current[int(part)]
          except (ValueError, IndexError):
              current = None
              break
      else:
          current = None
          break
  result = {"value": current, "path": path, "found": current is not None}
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```


## Step 10 — `builtin.memory_search` (Persistent Memory Search)

> **Capability:** `builtin.memory_search` · **Effect:** `read_memory` · **Permission:** Allow

### Step 10.1 — Tool row (class 0)

```
name:            "memory_search"
description:     "Search the agent's persistent memory store using a natural language query.
                  Returns the most relevant memory documents ranked by semantic similarity.
                  Limit defaults to 5; maximum is 20."
capability_id:   "builtin.memory_search"
effect_type:     "read_memory"
param_schema: {
  "type": "object",
  "properties": {
    "query": {"type": "string", "description": "Natural language search query"},
    "limit": {"type": "integer", "minimum": 1, "maximum": 20, "default": 5}
  },
  "required": ["query"],
  "additionalProperties": false
}
param_template:  {"query": "{{query}}"}
preconditions:   "query must not be empty"
error_handling:  "empty result is not an error; memory backend unavailable → tool error"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

### Step 10.2 — ToolSkill: `ts-memory-search` (class 13)

```
name:        "ts-memory-search"
tool_name:   "memory_search"
description: "Executor binding for memory_search. Required: query (natural language).
              Optional: limit (1–20, default 5). Returns ranked memory documents with
              content and relevance scores."
param_schema: {
  "type": "object",
  "properties": {
    "query": {"type": "string"},
    "limit": {"type": "integer", "minimum": 1, "maximum": 20}
  },
  "required": ["query"],
  "additionalProperties": false
}
param_template:  {"query": "{{query}}"}
preconditions:   "query must not be empty"
error_handling:  "no results → empty list (not an error)"
category:        "memory"
source:          "system"
validation_status: "validated"
```

### Step 10.3 — PythonCode: `pc-exec-memory-search` (class 22)

```
name:        "pc-exec-memory-search"
description: "Orchestrator executor: calls __execute_action__ to search persistent memory
              via builtin.memory_search. Input: query (string), limit (optional int 1–20)."
content: |
  # Orchestrator executor body.
  _query = "{{vars.slot0}}"
  _limit = {{vars.slot1}}
  _params = {"query": _query}
  if _limit and _limit > 0:
      _params["limit"] = _limit
  result = __execute_action__("memory_search", _params)
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 10.4 — Leaf Skill: `skill-memory-search` (class 1)

> One grain: semantic search across memory.

```
name:        "skill-memory-search"
class_code:  1
description: "Leaf skill: how to retrieve relevant information from the agent's persistent memory."
body: |
  Use `ts-memory-search` (via pc-exec-memory-search) when you need to recall past work,
  find saved notes, or check whether something was previously recorded. Provide a natural
  language query that describes what you are looking for. Set limit higher (up to 20)
  when broader recall coverage is needed. Review the returned documents and surface only
  those relevant to the current context.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 10.5 — Leaf Skill: `skill-memory-search-broad` (class 1)

> Separate grain: wide recall with higher limit.

```
name:        "skill-memory-search-broad"
class_code:  1
description: "Leaf skill: how to perform a broad memory recall across many documents."
body: |
  When a topic may span multiple memory documents, use `ts-memory-search` with
  `limit=20` to cast a wider net. Review all returned documents before deciding
  which are relevant. This is useful for session start — recovering full context about
  a project or topic before beginning work.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 10.6 — Recipe: `memory-search` (class 21)

> **Tier:** 0 — deterministic semantic search dispatch.

```
name:        "memory-search"
description: "Search the agent's persistent memory."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-memory-search>"],
    "label":   "Pre-load ts-memory-search ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-memory-search>"],
    "label":   "PythonCode calls __execute_action__(memory_search, {query, limit})"
  }
]
intent_examples: [
  {"input": "what do you remember about this project",        "class": 2},
  {"input": "search memory for authentication notes",         "class": 2},
  {"input": "find any saved notes about this topic",          "class": 2},
  {"input": "recall what we discussed last time",             "class": 2},
  {"input": "memory search",                                  "class": 1},
  {"input": "do you have notes on this",                      "class": 2},
  {"input": "search my memory for database setup",            "class": 2},
  {"input": "recall my earlier decisions about this module",  "class": 2},
  {"input": "find memory entries about this feature",         "class": 2},
  {"input": "memory recall",                                  "class": 1}
]
source: "system"
validation_status: "validated"
```

---

### Step 10.7 — Recipe: `memory-search-broad` (class 21)

> **Tier:** 0 — wide recall with limit=20. Separate recipe because the broad-recall use case
> (session start, "what do I know about this topic") is distinct from a focused search.

```
name:        "memory-search-broad"
description: "Search the agent's persistent memory with a wide recall (limit=20)."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-memory-search>"],
    "label":   "Pre-load ts-memory-search ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-memory-search>"],
    "label":   "PythonCode calls __execute_action__(memory_search, {query, limit:20})"
  }
]
intent_examples: [
  {"input": "recall everything you know about this project",   "class": 2},
  {"input": "broad memory recall for this topic",              "class": 2},
  {"input": "search all my memory about this feature",         "class": 2},
  {"input": "full memory recall at session start",             "class": 2},
  {"input": "memory broad search",                             "class": 1},
  {"input": "find all notes I have on this",                   "class": 2},
  {"input": "recall all prior decisions about this system",    "class": 2},
  {"input": "wide memory search for onboarding context",       "class": 2},
  {"input": "deep recall across all memory docs",              "class": 2},
  {"input": "start-of-session full memory restore",            "class": 2}
]
source: "system"
validation_status: "validated"
```

---

## Step 11 — `builtin.memory_write` (Persistent Memory Write)

> **Capability:** `builtin.memory_write` · **Effect:** `write_memory` · **Permission:** Allow

### Step 11.1 — Tool row (class 0)

```
name:            "memory_write"
description:     "Write or append content to the agent's persistent memory. Default target is
                  'daily_log' (today's dated log). Other targets: 'memory' (MEMORY.md),
                  'heartbeat' (HEARTBEAT.md), 'bootstrap' (clears BOOTSTRAP.md), or any
                  relative memory document path. Supports patch mode (old_string/new_string)."
capability_id:   "builtin.memory_write"
effect_type:     "write_memory"
param_schema: {
  "type": "object",
  "properties": {
    "content":     {"type": "string",  "description": "Content to write or append"},
    "target":      {"type": "string",  "description": "Destination: 'memory', 'daily_log' (default), 'heartbeat', 'bootstrap', or relative path"},
    "append":      {"type": "boolean", "description": "Append when true; replace when false", "default": true},
    "metadata":    {"type": "object",  "description": "Optional document metadata"},
    "old_string":  {"type": "string",  "description": "Exact text to replace (patch mode)"},
    "new_string":  {"type": "string",  "description": "Replacement text (patch mode)"},
    "replace_all": {"type": "boolean", "description": "Replace every old_string occurrence"},
    "timezone":    {"type": "string",  "description": "IANA timezone for daily_log date resolution"}
  },
  "additionalProperties": false
}
param_template:  {"content": "{{content}}"}
preconditions:   "content required unless using bootstrap target"
error_handling:  "old_string not found in patch mode → tool error; write failure → tool error"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

### Step 11.2 — ToolSkill: `ts-memory-write` (class 13)

```
name:        "ts-memory-write"
tool_name:   "memory_write"
description: "Executor binding for memory_write. Default writes to 'daily_log' (append mode).
              Use target='memory' for MEMORY.md. Patch mode: supply old_string + new_string.
              Setting append=false replaces the full document."
param_schema: {
  "type": "object",
  "properties": {
    "content":     {"type": "string"},
    "target":      {"type": "string"},
    "append":      {"type": "boolean"},
    "old_string":  {"type": "string"},
    "new_string":  {"type": "string"},
    "replace_all": {"type": "boolean"},
    "timezone":    {"type": "string"}
  },
  "additionalProperties": false
}
param_template:  {"content": "{{content}}"}
preconditions:   "patch mode requires both old_string and new_string"
error_handling:  "patch not found → tool error with safe summary"
category:        "memory"
source:          "system"
validation_status: "validated"
```

### Step 11.3 — PythonCode: `pc-exec-memory-write` (class 22)

```
name:        "pc-exec-memory-write"
description: "Orchestrator executor: calls __execute_action__ to write to persistent memory
              via builtin.memory_write. Input: content (string), target (optional string,
              default 'daily_log'), append (optional bool, default true)."
content: |
  # Orchestrator executor body.
  _content = "{{vars.slot0}}"
  _target = "{{vars.slot1}}"
  _append = {{vars.slot2}}
  _params = {"content": _content}
  if _target and _target != "":
      _params["target"] = _target
  if _append is not None:
      _params["append"] = _append
  result = __execute_action__("memory_write", _params)
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 11.4 — PythonCode: `pc-exec-memory-patch` (class 22)

> Separate executor for patch-mode writes (old_string → new_string).

```
name:        "pc-exec-memory-patch"
description: "Orchestrator executor: calls __execute_action__ for a targeted patch to a memory
              document via builtin.memory_write patch mode. Input: target (string), old_string
              (string), new_string (string), replace_all (optional bool)."
content: |
  # Orchestrator executor body.
  _target = "{{vars.slot0}}"
  _old = "{{vars.slot1}}"
  _new = "{{vars.slot2}}"
  _replace_all = {{vars.slot3}}
  _params = {"target": _target, "old_string": _old, "new_string": _new}
  if _replace_all:
      _params["replace_all"] = True
  result = __execute_action__("memory_write", _params)
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 11.5 — Leaf Skill: `skill-memory-write-log` (class 1)

> One grain: appending to the daily log.

```
name:        "skill-memory-write-log"
class_code:  1
description: "Leaf skill: how to log a note or progress update to today's daily log."
body: |
  Use `ts-memory-write` (via pc-exec-memory-write) with the default target='daily_log'
  and append=true to add timestamped progress notes, decisions, or session context to
  today's dated log. This is the lightest-weight memory write — use it frequently to
  maintain a running record of work within a session.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 11.6 — Leaf Skill: `skill-memory-write-main` (class 1)

> Separate grain: writing to the main MEMORY.md document.

```
name:        "skill-memory-write-main"
class_code:  1
description: "Leaf skill: how to update the main MEMORY.md document."
body: |
  Use `ts-memory-write` with target='memory' to update the primary MEMORY.md document.
  With append=true, content is added to the end. With append=false, the entire document
  is replaced — use this only when intentionally rebuilding the memory from scratch.
  For targeted updates (patch a section), use skill-memory-write-patch instead.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 11.7 — Leaf Skill: `skill-memory-write-patch` (class 1)

> Separate grain: targeted patch of an existing memory document.

```
name:        "skill-memory-write-patch"
class_code:  1
description: "Leaf skill: how to make a targeted edit to an existing memory document."
body: |
  Use `ts-memory-write` in patch mode (old_string + new_string) to replace a specific
  section of a memory document without rewriting the whole file. Read the document first
  with skill-memory-read to find the exact text to replace. Use replace_all=true when
  the same string appears multiple times and all occurrences should change.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 11.8 — Recipe: `memory-write` (class 21)

> **Tier:** 0 — deterministic write dispatch to daily_log.

```
name:        "memory-write"
description: "Write or append content to the agent's persistent memory."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-memory-write>"],
    "label":   "Pre-load ts-memory-write ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-memory-write>"],
    "label":   "PythonCode calls __execute_action__(memory_write, {content, target, append})"
  }
]
intent_examples: [
  {"input": "save this to memory",                  "class": 2},
  {"input": "remember this for later",              "class": 2},
  {"input": "log this progress note",               "class": 2},
  {"input": "update MEMORY.md with this decision",  "class": 2},
  {"input": "add this to my daily log",             "class": 1},
  {"input": "write a note to memory",               "class": 2},
  {"input": "store this for later",                 "class": 2},
  {"input": "persist this outcome to memory",       "class": 2},
  {"input": "memory write",                         "class": 1},
  {"input": "append this to the daily log",         "class": 1}
]
source: "system"
validation_status: "validated"
```

---

### Step 11.9 — Recipe: `memory-write-log` (class 21)

> **Tier:** 0 — deterministic append to daily_log. One recipe per target variant.
> The daily_log is the most common write target — its own recipe improves routing accuracy.

```
name:        "memory-write-log"
description: "Append a note or progress entry to today's daily log in persistent memory."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-memory-write>"],
    "label":   "Pre-load ts-memory-write ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-memory-write>"],
    "label":   "PythonCode calls __execute_action__(memory_write, {content, target:'daily_log', append:true})"
  }
]
intent_examples: [
  {"input": "log this progress note",                     "class": 1},
  {"input": "add to my daily log",                        "class": 1},
  {"input": "append a note to today's log",               "class": 1},
  {"input": "write a progress update to the daily log",   "class": 1},
  {"input": "daily log entry",                            "class": 1},
  {"input": "record this in the daily log",               "class": 1},
  {"input": "log what I did today",                       "class": 2},
  {"input": "log session progress",                       "class": 1},
  {"input": "note this down in my activity log",          "class": 2},
  {"input": "add this to today's memory log",             "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Step 11.10 — Recipe: `memory-write-main` (class 21)

> **Tier:** 0 — deterministic write to MEMORY.md. One recipe per target variant.
> Writing to the main MEMORY.md is a deliberate, structured action — its own recipe
> distinguishes it from daily logging.

```
name:        "memory-write-main"
description: "Append content to the main MEMORY.md document in persistent memory."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-memory-write>"],
    "label":   "Pre-load ts-memory-write ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-memory-write>"],
    "label":   "PythonCode calls __execute_action__(memory_write, {content, target:'memory', append:true})"
  }
]
intent_examples: [
  {"input": "update MEMORY.md with this",                 "class": 1},
  {"input": "add this decision to MEMORY.md",             "class": 1},
  {"input": "write this to the main memory document",     "class": 1},
  {"input": "append to MEMORY.md",                        "class": 1},
  {"input": "update my main memory",                      "class": 1},
  {"input": "save this finding to MEMORY.md",             "class": 1},
  {"input": "add a permanent note to memory",             "class": 2},
  {"input": "write to the memory document",               "class": 1}
]
source: "system"
validation_status: "validated"
```

---

### Step 11.11 — Recipe: `memory-write-patch` (class 21)

> **Tier:** 0 — deterministic patch of an existing memory document. One recipe per mode.
> Patch mode (old_string → new_string) is structurally different from append mode and
> warrants its own recipe for correct routing.

```
name:        "memory-write-patch"
description: "Patch a specific section of an existing memory document using search-replace."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-memory-write>"],
    "label":   "Pre-load ts-memory-write ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-memory-patch>"],
    "label":   "PythonCode calls __execute_action__(memory_write, {target, old_string, new_string})"
  }
]
intent_examples: [
  {"input": "patch a section in MEMORY.md",                 "class": 1},
  {"input": "replace this text in my memory document",      "class": 1},
  {"input": "update a specific section of a memory file",   "class": 1},
  {"input": "memory write patch mode",                      "class": 1},
  {"input": "fix a section in HEARTBEAT.md",                "class": 2},
  {"input": "search and replace in a memory document",      "class": 2},
  {"input": "targeted edit to a memory file",               "class": 1},
  {"input": "update one section without replacing the file","class": 2}
]
source: "system"
validation_status: "validated"
```

---

## Step 12 — `builtin.memory_read` (Persistent Memory Read by Path)

> **Capability:** `builtin.memory_read` · **Effect:** `read_memory` · **Permission:** Allow

### Step 12.1 — Tool row (class 0)

```
name:            "memory_read"
description:     "Read a specific memory document by its relative path. Returns the full
                  document content. Use memory_search for semantic discovery; use memory_read
                  when you know the exact path."
capability_id:   "builtin.memory_read"
effect_type:     "read_memory"
param_schema: {
  "type": "object",
  "properties": {
    "path": {"type": "string", "description": "Relative memory document path to read"}
  },
  "required": ["path"],
  "additionalProperties": false
}
param_template:  {"path": "{{path}}"}
preconditions:   "path must not be empty"
error_handling:  "document not found → tool error"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

### Step 12.2 — ToolSkill: `ts-memory-read` (class 13)

```
name:        "ts-memory-read"
tool_name:   "memory_read"
description: "Executor binding for memory_read. Required: path (relative memory document path).
              Returns the full document content. Use for known paths; use ts-memory-search
              for semantic discovery."
param_schema: {
  "type": "object",
  "properties": {
    "path": {"type": "string"}
  },
  "required": ["path"],
  "additionalProperties": false
}
param_template:  {"path": "{{path}}"}
preconditions:   "path must not be empty"
error_handling:  "not found → tool error"
category:        "memory"
source:          "system"
validation_status: "validated"
```

### Step 12.3 — PythonCode: `pc-exec-memory-read` (class 22)

```
name:        "pc-exec-memory-read"
description: "Orchestrator executor: calls __execute_action__ to read a memory document by
              path via builtin.memory_read. Input: path (string). Output: full document content."
content: |
  # Orchestrator executor body.
  _path = "{{vars.slot0}}"
  result = __execute_action__("memory_read", {"path": _path})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 12.4 — Leaf Skill: `skill-memory-read` (class 1)

```
name:        "skill-memory-read"
class_code:  1
description: "Leaf skill: how to read a specific memory document by its exact path."
body: |
  Use `ts-memory-read` (via pc-exec-memory-read) when you know the exact path of a memory
  document (e.g. MEMORY.md, HEARTBEAT.md, or a specific note file). Returns the full
  content of the document. If you do not know the exact path, use skill-memory-search to
  discover it first, or use skill-memory-tree to browse the directory structure.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 12.5 — Recipe: `memory-read` (class 21)

> **Tier:** 0 — deterministic read by known path.

```
name:        "memory-read"
description: "Read a specific memory document by path."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-memory-read>"],
    "label":   "Pre-load ts-memory-read ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-memory-read>"],
    "label":   "PythonCode calls __execute_action__(memory_read, {path})"
  }
]
intent_examples: [
  {"input": "read MEMORY.md",                         "class": 1},
  {"input": "show me the contents of HEARTBEAT.md",   "class": 1},
  {"input": "read my memory document",                "class": 2},
  {"input": "open this memory file",                  "class": 2},
  {"input": "show memory at this path",               "class": 1},
  {"input": "read the file at this memory path",      "class": 1},
  {"input": "memory read",                            "class": 1},
  {"input": "open the notes at this memory location", "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Step 12.6 — Recipe: `memory-read-main` (class 21)

> **Tier:** 0 — read MEMORY.md by well-known path. One recipe per common known path.
> The MEMORY.md document is the primary durable context file — routing to it directly
> avoids the overhead of path lookup or search.

```
name:        "memory-read-main"
description: "Read the main MEMORY.md document from persistent memory."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-memory-read>"],
    "label":   "Pre-load ts-memory-read ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-memory-read>"],
    "label":   "PythonCode calls __execute_action__(memory_read, {path:'MEMORY.md'})"
  }
]
intent_examples: [
  {"input": "read MEMORY.md",                            "class": 1},
  {"input": "show me MEMORY.md",                         "class": 1},
  {"input": "open the main memory document",             "class": 1},
  {"input": "read my persistent memory",                 "class": 2},
  {"input": "what is in MEMORY.md",                      "class": 1},
  {"input": "show me the contents of memory",            "class": 2},
  {"input": "display MEMORY.md",                         "class": 1},
  {"input": "read main memory file",                     "class": 1},
  {"input": "show me my durable context document",       "class": 2},
  {"input": "read the primary memory doc at session start", "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Step 12.7 — Recipe: `memory-read-heartbeat` (class 21)

> **Tier:** 0 — read HEARTBEAT.md by well-known path. One recipe per common known path.
> HEARTBEAT.md is a regularly updated status/context file — the orchestrator can
> route directly to it without path ambiguity.

```
name:        "memory-read-heartbeat"
description: "Read the HEARTBEAT.md status document from persistent memory."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-memory-read>"],
    "label":   "Pre-load ts-memory-read ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-memory-read>"],
    "label":   "PythonCode calls __execute_action__(memory_read, {path:'HEARTBEAT.md'})"
  }
]
intent_examples: [
  {"input": "read HEARTBEAT.md",                         "class": 1},
  {"input": "show me the heartbeat document",            "class": 1},
  {"input": "what is in HEARTBEAT.md",                   "class": 1},
  {"input": "read the agent heartbeat status",           "class": 2},
  {"input": "show me the current heartbeat",             "class": 2},
  {"input": "display HEARTBEAT.md",                      "class": 1},
  {"input": "read heartbeat",                            "class": 1},
  {"input": "open the heartbeat memory file",            "class": 1},
  {"input": "show the latest heartbeat checkpoint",      "class": 2},
  {"input": "what does my heartbeat status say",         "class": 2}
]
source: "system"
validation_status: "validated"
```



## Step 13 — `builtin.memory_tree` (Memory Directory Tree)

> **Capability:** `builtin.memory_tree` · **Effect:** `read_memory` · **Permission:** Allow

### Step 13.1 — Tool row (class 0)

```
name:            "memory_tree"
description:     "List the directory tree of the agent's persistent memory. Returns entry names
                  and types up to the specified depth. Used to discover memory structure before
                  targeted reads."
capability_id:   "builtin.memory_tree"
effect_type:     "read_memory"
param_schema: {
  "type": "object",
  "properties": {
    "path":  {"type": "string",  "description": "Relative memory directory path (omit for root)"},
    "depth": {"type": "integer", "minimum": 1, "maximum": 10, "default": 1}
  },
  "additionalProperties": false
}
param_template:  {}
preconditions:   []
error_handling:  "path not found in memory → tool error"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

### Step 13.2 — ToolSkill: `ts-memory-tree` (class 13)

```
name:        "ts-memory-tree"
tool_name:   "memory_tree"
description: "Executor binding for memory_tree. Optional: path (relative memory dir, defaults
              to root), depth (1–10, default 1). Returns the directory tree of persistent memory."
param_schema: {
  "type": "object",
  "properties": {
    "path":  {"type": "string"},
    "depth": {"type": "integer", "minimum": 1, "maximum": 10}
  },
  "additionalProperties": false
}
param_template:  {}
preconditions:   []
error_handling:  "path not found → tool error"
category:        "memory"
source:          "system"
validation_status: "validated"
```

### Step 13.3 — PythonCode: `pc-exec-memory-tree` (class 22)

```
name:        "pc-exec-memory-tree"
description: "Orchestrator executor: calls __execute_action__ to list the memory directory
              tree via builtin.memory_tree. Input: path (optional string), depth (optional int)."
content: |
  # Orchestrator executor body.
  _path = "{{vars.slot0}}"
  _depth = {{vars.slot1}}
  _params = {}
  if _path and _path != "":
      _params["path"] = _path
  if _depth and _depth > 0:
      _params["depth"] = _depth
  result = __execute_action__("memory_tree", _params)
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 13.4 — Leaf Skill: `skill-memory-tree` (class 1)

```
name:        "skill-memory-tree"
class_code:  1
description: "Leaf skill: how to browse the structure of the agent's persistent memory."
body: |
  Use `ts-memory-tree` (via pc-exec-memory-tree) to discover what memory documents exist.
  Call with no parameters to get the root structure at depth=1. Increase depth to see
  deeper levels. Use the returned structure to decide which documents to read with
  skill-memory-read or to inform a skill-memory-search query.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 13.5 — Recipe: `memory-tree` (class 21)

> **Tier:** 0 — deterministic tree listing.

```
name:        "memory-tree"
description: "List the directory structure of the agent's persistent memory."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-memory-tree>"],
    "label":   "Pre-load ts-memory-tree ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-memory-tree>"],
    "label":   "PythonCode calls __execute_action__(memory_tree, {path, depth})"
  }
]
intent_examples: [
  {"input": "what files are in my memory",             "class": 2},
  {"input": "show me the memory directory structure",  "class": 2},
  {"input": "list all memory documents",               "class": 1},
  {"input": "browse my memory files",                  "class": 2},
  {"input": "memory tree",                             "class": 1},
  {"input": "what memory documents exist",             "class": 2},
  {"input": "show me the memory hierarchy",            "class": 2},
  {"input": "memory directory listing",                "class": 1},
  {"input": "what notes do I have stored",             "class": 2},
  {"input": "explore my memory structure",             "class": 2}
]
source: "system"
validation_status: "validated"
```

---

## Step 13.x — Memory PythonCode Helpers + Domain Skill

### Step 13.x.1 — PythonCode `pc-memory-extract-section` (class 22)

> Pure logic: extracts a named Markdown section. No I/O, no imports.

```
name:        "pc-memory-extract-section"
description: "Pure-logic helper: extracts a named section from a Markdown document using
              heading matching. Input: content (string), heading (string — heading text
              without # prefix). Output: {section_content, heading, found}."
content: |
  # No I/O, no imports. IBS bakes in content and heading before execution.
  content = "{{vars.slot0}}"
  heading = "{{vars.slot1}}"
  lines = content.split("\n")
  in_section = False
  section_lines = []
  for line in lines:
      stripped = line.lstrip("#").strip()
      if stripped == heading and line.startswith("#"):
          in_section = True
          continue
      if in_section:
          if line.startswith("#"):
              break
          section_lines.append(line)
  section_content = "\n".join(section_lines).strip() if section_lines else None
  result = {"section_content": section_content, "heading": heading, "found": section_content is not None}
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 13.x.2 — PythonCode `pc-memory-format-entry` (class 22)

> Pure logic: formats a memory entry for appending. Timestamp supplied as string param.

```
name:        "pc-memory-format-entry"
description: "Pure-logic helper: formats a memory entry string ready for appending to a
              memory document. Input: text (string), timestamp_str (string — caller supplies
              pre-fetched timestamp). Output: {formatted_entry}."
content: |
  # No I/O, no imports, no datetime. Caller must supply timestamp_str.
  text = "{{vars.slot0}}"
  timestamp_str = "{{vars.slot1}}"
  formatted_entry = f"### {timestamp_str}\n\n{text}\n"
  result = {"formatted_entry": formatted_entry}
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 13.x.3 — Domain Skill `skill-memory` (class 2)

```
name:        "skill-memory"
class_code:  2
description: "The memory domain provides four tools for the agent's persistent memory store:

              READING / DISCOVERING:
              — skill-memory-search: Semantic search by topic — use when path is unknown.
              — skill-memory-search-broad: Wide recall with limit=20 for session start.
              — skill-memory-search-and-read: Search + immediately read the top result.
              — skill-memory-read: Read a specific document by exact path.
              — skill-memory-tree: Browse the directory structure.

              WRITING:
              — skill-memory-write-log: Append a note to today's daily_log (default).
              — skill-memory-write-main: Update the main MEMORY.md document.
              — skill-memory-write-patch: Targeted patch of an existing memory document.

              Decision guide:
              • Recalling by topic (summary list) → skill-memory-search
              • Recalling + reading the top result in one step → skill-memory-search-and-read
              • Session start full recall → skill-memory-search-broad
              • Reading a known file → skill-memory-read
              • Logging progress → skill-memory-write-log
              • Updating permanent context → skill-memory-write-main
              • Patching a section → skill-memory-write-patch
              • Discovering what files exist → skill-memory-tree

              Orchestrator note: NEVER use datetime.now() in PythonCode. Always call
              skill-time-now first to get a timestamp, then pass it to pc-memory-format-entry."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```


## Step 13.x.4 — Memory Search-and-Read Combined Recipe

> **`memory-search-and-read`** is a Tier-1 combined recipe that executes a semantic search
> and then reads the top result automatically. This two-step pattern (search → read
> highest-scoring document) is extremely common but was missing as a named recipe.
>
> The pattern is Tier 1 because the LLM must decide which result to read when there are
> multiple candidates. For the single-result-auto-read pattern, the PythonCode does the
> dispatch; the LLM step interprets the search query intent.

### Step 13.x.4.1 — Leaf Skill: `skill-memory-search-and-read` (class 1)

```
name:        "skill-memory-search-and-read"
class_code:  1
description: "Leaf skill: how to search memory and immediately read the top result."
body: |
  Use when the user wants to recall information and immediately see the full content —
  not just the search summary. The pattern:
  1. Call ts-memory-search with the topic query (via pc-exec-memory-search).
  2. Take the highest-scoring result's path from the search output.
  3. Call ts-memory-read with that path (via pc-exec-memory-read).
  4. Return the full document content.

  If no results are found, report that no memory matches the topic. Do not fabricate
  a document path. Always check the search result before reading.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 13.x.4.2 — Recipe: `memory-search-and-read` (class 21)

> **Tier:** 1 — LLM needed to formulate the query and select the correct result.
> The PythonCode dispatches both the search and the read; the LLM decides query + result.

```
name:        "memory-search-and-read"
description: "Search persistent memory by topic and read the top matching document in one flow."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-memory-search-and-read>"],
    "label":   "Load search-and-read combined leaf skill body"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-memory-search>", "<uuid:ts-memory-read>"],
    "label":   "Pre-load both ToolSkill bindings"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-memory-search>", "<uuid:pc-exec-memory-read>"],
    "label":   "PythonCode: search memory, take top result path, read document"
  },
  {
    "step_id": "step-4",
    "type":    "llm",
    "label":   "LLM interprets query intent, selects best result path, presents content"
  }
]
intent_examples: [
  {"input": "recall what I know about this topic",             "class": 2},
  {"input": "search memory and show me the full document",     "class": 1},
  {"input": "find and read the memory about X",                "class": 1},
  {"input": "look up this topic in my memory and show it",     "class": 2},
  {"input": "recall and display this memory doc",              "class": 2},
  {"input": "search then read the result",                     "class": 1},
  {"input": "find this note and open it",                      "class": 2},
  {"input": "memory search and read",                          "class": 1},
  {"input": "recall the document about X",                     "class": 2},
  {"input": "find this memory entry and show its contents",    "class": 1}
]
source: "system"
validation_status: "validated"
```

---



## Step 14 — `builtin.time` (Time Operations)

> **Capability:** `builtin.time` · **Effect:** `read_only` · **Permission:** Allow
> Operations: now, parse, convert — routed through one Tool, three ToolSkills.

### Step 14.1 — Tool row (class 0)

```
name:            "time"
description:     "Perform time and timezone operations: get the current time (now), parse a
                  timestamp string (parse), convert between timezones (convert)."
capability_id:   "builtin.time"
effect_type:     "read_only"
param_schema: {
  "type": "object",
  "properties": {
    "operation":    {"type": "string", "enum": ["now","parse","convert"],
                     "description": "Time operation. Defaults to now."},
    "input":        {"type": "string", "description": "Timestamp for parse/convert"},
    "timezone":     {"type": "string", "description": "IANA timezone name"},
    "from_timezone":{"type": "string", "description": "IANA timezone for input interpretation"},
    "to_timezone":  {"type": "string", "description": "IANA timezone for conversion output"}
  },
  "additionalProperties": false
}
param_template:  {"operation": "now"}
preconditions:   []
error_handling:  "invalid timezone → tool error; invalid timestamp format → tool error"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

### Step 14.2 — ToolSkill: `ts-time-now` (class 13)

```
name:        "ts-time-now"
tool_name:   "time"
description: "Executor binding: get the current UTC timestamp (operation='now'). Optional:
              timezone (IANA name) to return current time in a specific timezone."
param_schema: {
  "type": "object",
  "properties": {
    "operation": {"type": "string", "enum": ["now"], "default": "now"},
    "timezone":  {"type": "string"}
  },
  "additionalProperties": false
}
param_template:  {"operation": "now"}
preconditions:   []
error_handling:  "invalid timezone → tool error"
category:        "time"
source:          "system"
validation_status: "validated"
```

### Step 14.3 — PythonCode: `pc-exec-time-now` (class 22)

```
name:        "pc-exec-time-now"
description: "Orchestrator executor: calls __execute_action__ to get the current timestamp
              via builtin.time operation='now'. Input: timezone (optional IANA string)."
content: |
  # Orchestrator executor body.
  _timezone = "{{vars.slot0}}"
  _params = {"operation": "now"}
  if _timezone and _timezone != "":
      _params["timezone"] = _timezone
  result = __execute_action__("time", _params)
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 14.4 — Leaf Skill: `skill-time-now` (class 1)

```
name:        "skill-time-now"
class_code:  1
description: "Leaf skill: how to get the current date and time."
body: |
  Use `ts-time-now` (via pc-exec-time-now) to get the current UTC timestamp. Provide a
  timezone parameter if the user specified a locale (e.g. 'America/New_York'). The returned
  timestamp can be used as input to other time operations or to stamp memory entries.
  PythonCode that needs the current time must always call this first — never use datetime.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 14.5 — Recipe: `time-now` (class 21)

> **Tier:** 0 — deterministic timestamp fetch.

```
name:        "time-now"
description: "Get the current date and time."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-time-now>"],
    "label":   "Pre-load ts-time-now ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-time-now>"],
    "label":   "PythonCode calls __execute_action__(time, {operation:now})"
  }
]
intent_examples: [
  {"input": "what time is it",                "class": 1},
  {"input": "what is today's date",           "class": 1},
  {"input": "current time in Tokyo",          "class": 2},
  {"input": "get the current UTC timestamp",  "class": 1},
  {"input": "what day is it",                 "class": 1},
  {"input": "what time is it now",            "class": 1},
  {"input": "what is the current time",       "class": 1},
  {"input": "time now",                       "class": 1},
  {"input": "give me a timestamp",            "class": 1},
  {"input": "what time is it in Berlin",      "class": 2}
]
source: "system"
validation_status: "validated"
```

### Step 14.5b — Recipe: `time-now-tz` (class 21)

> **Tier:** 0 — deterministic timestamp fetch in a specified timezone. One recipe per variant.
> When the user specifies a timezone, this variant routes more accurately than `time-now`.

```
name:        "time-now-tz"
description: "Get the current date and time in a specific IANA timezone."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-time-now>"],
    "label":   "Pre-load ts-time-now ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-time-now>"],
    "label":   "PythonCode calls __execute_action__(time, {operation:now, timezone:<tz>})"
  }
]
intent_examples: [
  {"input": "what time is it in Tokyo",                   "class": 1},
  {"input": "current time in America/New_York",           "class": 1},
  {"input": "what time is it in Europe/Berlin",           "class": 1},
  {"input": "time now in Australia/Sydney",               "class": 1},
  {"input": "current time in Pacific timezone",           "class": 2},
  {"input": "what is the time in EST",                    "class": 2},
  {"input": "get me the current time in London",          "class": 1},
  {"input": "what time is it in India right now",         "class": 2},
  {"input": "tell me the time in Singapore",              "class": 2},
  {"input": "current UTC offset for Europe/Paris",        "class": 2},
  {"input": "time in China right now",                    "class": 2},
  {"input": "what is the local time in New York",         "class": 2}
]
source: "system"
validation_status: "validated"
```



### Step 14.6 — ToolSkill: `ts-time-parse` (class 13)

```
name:        "ts-time-parse"
tool_name:   "time"
description: "Executor binding: parse a timestamp string (operation='parse'). Required: input
              (timestamp string). Optional: timezone (IANA, for interpreting the input)."
param_schema: {
  "type": "object",
  "properties": {
    "operation": {"type": "string", "enum": ["parse"]},
    "input":     {"type": "string"},
    "timezone":  {"type": "string"}
  },
  "required": ["operation", "input"],
  "additionalProperties": false
}
param_template:  {"operation": "parse", "input": "{{input}}"}
preconditions:   "input must be a recognisable timestamp string"
error_handling:  "unrecognised format → tool error"
category:        "time"
source:          "system"
validation_status: "validated"
```

### Step 14.7 — PythonCode: `pc-exec-time-parse` (class 22)

```
name:        "pc-exec-time-parse"
description: "Orchestrator executor: calls __execute_action__ to parse a timestamp string
              via builtin.time operation='parse'. Input: input (string), timezone (optional)."
content: |
  # Orchestrator executor body.
  _input = "{{vars.slot0}}"
  _timezone = "{{vars.slot1}}"
  _params = {"operation": "parse", "input": _input}
  if _timezone and _timezone != "":
      _params["timezone"] = _timezone
  result = __execute_action__("time", _params)
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 14.8 — Leaf Skill: `skill-time-parse` (class 1)

```
name:        "skill-time-parse"
class_code:  1
description: "Leaf skill: how to parse a timestamp string into a structured time value."
body: |
  Use `ts-time-parse` (via pc-exec-time-parse) to interpret a date or time in text form.
  Supports ISO 8601, RFC 2822, and common human-readable formats. Provide timezone when
  the input is ambiguous about its timezone context.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 14.9 — Recipe: `time-parse` (class 21)

> **Tier:** 0 — deterministic parse.

```
name:        "time-parse"
description: "Parse a timestamp string into a structured time value."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-time-parse>"],
    "label":   "Pre-load ts-time-parse ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-time-parse>"],
    "label":   "PythonCode calls __execute_action__(time, {operation:parse, input})"
  }
]
intent_examples: [
  {"input": "parse this date string",              "class": 1},
  {"input": "what timestamp is 2024-01-15T10:30",  "class": 1},
  {"input": "interpret this date format",          "class": 2},
  {"input": "parse the timestamp from this log",   "class": 2},
  {"input": "what does this date mean",            "class": 2},
  {"input": "parse this ISO timestamp",            "class": 1},
  {"input": "read this date string",               "class": 2},
  {"input": "time parse",                          "class": 1}
]
source: "system"
validation_status: "validated"
```

### Step 14.10 — ToolSkill: `ts-time-convert` (class 13)

```
name:        "ts-time-convert"
tool_name:   "time"
description: "Executor binding: convert a timestamp between timezones (operation='convert').
              Required: input (timestamp string). Optional: from_timezone (IANA, default UTC),
              to_timezone (IANA, default UTC)."
param_schema: {
  "type": "object",
  "properties": {
    "operation":    {"type": "string", "enum": ["convert"]},
    "input":        {"type": "string"},
    "from_timezone":{"type": "string"},
    "to_timezone":  {"type": "string"}
  },
  "required": ["operation", "input"],
  "additionalProperties": false
}
param_template:  {"operation": "convert", "input": "{{input}}"}
preconditions:   "input must be a recognisable timestamp"
error_handling:  "invalid timezone → tool error"
category:        "time"
source:          "system"
validation_status: "validated"
```

### Step 14.11 — PythonCode: `pc-exec-time-convert` (class 22)

```
name:        "pc-exec-time-convert"
description: "Orchestrator executor: calls __execute_action__ to convert a timestamp between
              timezones via builtin.time operation='convert'. Input: input (string),
              from_timezone (optional), to_timezone (optional IANA string)."
content: |
  # Orchestrator executor body.
  _input = "{{vars.slot0}}"
  _from_tz = "{{vars.slot1}}"
  _to_tz = "{{vars.slot2}}"
  _params = {"operation": "convert", "input": _input}
  if _from_tz and _from_tz != "":
      _params["from_timezone"] = _from_tz
  if _to_tz and _to_tz != "":
      _params["to_timezone"] = _to_tz
  result = __execute_action__("time", _params)
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 14.12 — Leaf Skill: `skill-time-convert` (class 1)

```
name:        "skill-time-convert"
class_code:  1
description: "Leaf skill: how to convert a timestamp to a different timezone."
body: |
  Use `ts-time-convert` (via pc-exec-time-convert) to express a timestamp in a different
  timezone. Provide the input timestamp and the target `to_timezone` (IANA name, e.g.
  'America/New_York', 'Europe/Berlin', 'Asia/Tokyo'). Optionally specify `from_timezone`
  if the input's timezone is ambiguous.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 14.13 — Recipe: `time-convert` (class 21)

> **Tier:** 0 — deterministic timezone conversion.

```
name:        "time-convert"
description: "Convert a timestamp to a different timezone."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-time-convert>"],
    "label":   "Pre-load ts-time-convert ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-time-convert>"],
    "label":   "PythonCode calls __execute_action__(time, {operation:convert, input, to_timezone})"
  }
]
intent_examples: [
  {"input": "convert this time to New York timezone",   "class": 2},
  {"input": "what is 3pm UTC in Tokyo",                 "class": 2},
  {"input": "timezone conversion for this timestamp",   "class": 2},
  {"input": "what time is this in EST",                 "class": 2},
  {"input": "convert this UTC time to local time",      "class": 2},
  {"input": "what is this time in Europe/Berlin",       "class": 2},
  {"input": "time convert to Asia/Tokyo",               "class": 1},
  {"input": "express this timestamp in Pacific time",   "class": 2}
]
source: "system"
validation_status: "validated"
```

---

## Step 14.x — `builtin.time` Additional Operations (diff + format)

> **`time.diff` and `time.format` are both implemented in Rust** (`first_party_tools/time.rs`)
> but were missing from the plan. These are pure Tier-0 operations — deterministic, no LLM.
> `diff` returns signed seconds/minutes/hours/days between two timestamps.
> `format` renders a timestamp as a human-readable string using a chrono format string.

### Step 14.14 — ToolSkill: `ts-time-diff` (class 13)

```
name:        "ts-time-diff"
tool_name:   "time"
description: "Executor binding: compute the signed difference between two timestamps.
             Operation: 'diff'. Required: input (first timestamp string), timestamp2
             (second timestamp string). Optional: from_timezone / timezone (IANA, if
             inputs lack timezone info). Returns {seconds, minutes, hours, days} — all
             signed: positive when timestamp2 is after input."
param_schema: {
  "type": "object",
  "properties": {
    "operation":    {"type": "string", "enum": ["diff"]},
    "input":        {"type": "string", "description": "First timestamp string"},
    "timestamp":    {"type": "string", "description": "Alias for input"},
    "timestamp2":   {"type": "string", "description": "Second timestamp string"},
    "timezone":     {"type": "string", "description": "IANA timezone for both inputs"},
    "from_timezone":{"type": "string", "description": "Alias for timezone in diff context"}
  },
  "required": ["operation", "input", "timestamp2"],
  "additionalProperties": false
}
param_template:  '{"operation":"diff","input":"{{input}}","timestamp2":"{{timestamp2}}"}'
preconditions:   ["both input and timestamp2 must be recognisable timestamp strings"]
error_handling:  "unrecognised format → tool error; ambiguous local time → tool error"
category:        "time"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 14.15 — ToolSkill: `ts-time-format` (class 13)

```
name:        "ts-time-format"
tool_name:   "time"
description: "Executor binding: format a timestamp as a human-readable string.
             Operation: 'format'. Required: input (timestamp string). Optional:
             format / format_string (chrono format string, default '%Y-%m-%d %H:%M:%S %Z'),
             timezone (IANA timezone to express the output in),
             from_timezone (IANA timezone for interpreting a naive input).
             Returns {formatted, utc_iso, timezone?}."
param_schema: {
  "type": "object",
  "properties": {
    "operation":    {"type": "string", "enum": ["format"]},
    "input":        {"type": "string", "description": "Timestamp string to format"},
    "timestamp":    {"type": "string", "description": "Alias for input"},
    "format":       {"type": "string", "description": "chrono format string, e.g. '%d %b %Y'"},
    "format_string":{"type": "string", "description": "Alias for format"},
    "timezone":     {"type": "string", "description": "IANA timezone for the output"},
    "from_timezone":{"type": "string", "description": "IANA timezone for interpreting a naive input"}
  },
  "required": ["operation", "input"],
  "additionalProperties": false
}
param_template:  '{"operation":"format","input":"{{input}}"}'
preconditions:   ["input must be a recognisable timestamp string"]
error_handling:  "unrecognised format → tool error; invalid timezone → tool error"
category:        "time"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 14.16 — PythonCode: `pc-exec-time-diff` (class 22)

```
name:        "pc-exec-time-diff"
description: "Orchestrator executor: calls __execute_action__ to compute the signed
              difference between two timestamps via builtin.time operation='diff'.
              Input: input (string), timestamp2 (string). Output: {seconds, minutes,
              hours, days} — all signed."
content: |
  # Orchestrator executor body. No I/O, no imports, no network.
  # IBS bakes in slot values before execution.
  _input = "{{vars.slot0}}"
  _ts2   = "{{vars.slot1}}"
  result = __execute_action__("time", {
      "operation":  "diff",
      "input":      _input,
      "timestamp2": _ts2
  })
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 14.17 — PythonCode: `pc-exec-time-format` (class 22)

```
name:        "pc-exec-time-format"
description: "Orchestrator executor: calls __execute_action__ to format a timestamp
              as a human-readable string via builtin.time operation='format'.
              Input: input (string), optional format_string (string), optional timezone
              (string). Output: {formatted, utc_iso, timezone?}."
content: |
  # Orchestrator executor body. No I/O, no imports, no network.
  # IBS bakes in slot values before execution.
  _input  = "{{vars.slot0}}"
  _fmt    = "{{vars.slot1}}"
  _tz     = "{{vars.slot2}}"
  _params = {"operation": "format", "input": _input}
  if _fmt:
      _params["format_string"] = _fmt
  if _tz:
      _params["timezone"] = _tz
  result = __execute_action__("time", _params)
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 14.18 — Leaf Skill: `skill-time-diff` (class 1)

```
name:        "skill-time-diff"
class_code:  1
description: "Leaf skill: how to compute the difference between two timestamps."
body: |
  Use `ts-time-diff` (via pc-exec-time-diff) to compute the signed duration between
  two timestamps. Provide both `input` (first timestamp) and `timestamp2` (second
  timestamp) as ISO 8601 strings or any recognised format. The result contains
  `seconds`, `minutes`, `hours`, and `days` — all signed (positive when timestamp2
  is after input). If the inputs are in local time without timezone info, supply
  `from_timezone` (IANA name). Use this when the user asks 'how long ago', 'how
  many days between', or 'what is the duration between these dates'.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

---

### Step 14.19 — Leaf Skill: `skill-time-format` (class 1)

```
name:        "skill-time-format"
class_code:  1
description: "Leaf skill: how to format a timestamp as a human-readable string."
body: |
  Use `ts-time-format` (via pc-exec-time-format) to render a timestamp in a custom
  or human-readable format. Provide `input` as the source timestamp and optionally
  `format_string` using chrono format codes (e.g. `'%d %b %Y'`, `'%I:%M %p'`,
  `'%A, %B %-d, %Y'`). Optionally supply `timezone` (IANA) to localise the output.
  Default format is `'%Y-%m-%d %H:%M:%S %Z'`. Use when the user asks to display a
  date in a particular style, or when building a human-readable timestamp label for
  memory entries, reports, or logs.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

---

### Step 14.20 — Recipe: `time-diff` (class 21)

> **Tier:** 0 — fully deterministic. The Rust layer computes the duration; no LLM needed.

```
name:        "time-diff"
description: "Compute the signed difference between two timestamps (seconds, minutes, hours, days)."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-time-diff>"],
    "label":   "Pre-load ts-time-diff ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-time-diff>"],
    "label":   "PythonCode calls __execute_action__(time, {operation:diff, input, timestamp2})"
  }
]
intent_examples: [
  {"input": "how many days between these two dates",        "class": 2},
  {"input": "how long ago was this timestamp",              "class": 2},
  {"input": "compute the duration between two timestamps",  "class": 1},
  {"input": "what is the difference in hours between X and Y", "class": 2},
  {"input": "time diff",                                    "class": 1},
  {"input": "how many seconds between these two times",     "class": 2},
  {"input": "elapsed time between these events",            "class": 2},
  {"input": "time difference in days",                      "class": 1}
]
source: "system"
validation_status: "validated"
```

---

### Step 14.21 — Recipe: `time-format` (class 21)

> **Tier:** 0 — deterministic format rendering. No LLM needed.

```
name:        "time-format"
description: "Format a timestamp as a human-readable string using a chrono format string."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-time-format>"],
    "label":   "Pre-load ts-time-format ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-time-format>"],
    "label":   "PythonCode calls __execute_action__(time, {operation:format, input, format_string?})"
  }
]
intent_examples: [
  {"input": "format this date as day month year",          "class": 2},
  {"input": "display this timestamp in a readable format", "class": 1},
  {"input": "format this timestamp",                       "class": 1},
  {"input": "render this date as DD MMM YYYY",             "class": 2},
  {"input": "time format",                                 "class": 1},
  {"input": "show this date in 12-hour time",              "class": 2},
  {"input": "format the current date for a log entry",     "class": 2},
  {"input": "pretty-print this timestamp",                 "class": 1}
]
source: "system"
validation_status: "validated"
```

---

## Step 15 — `builtin.json` (JSON Operations)

> **Capability:** `builtin.json` · **Effect:** `read_only` · **Permission:** Allow
> Operations: parse, stringify, query, validate.

### Step 15.1 — Tool row (class 0)

```
name:            "json"
description:     "Perform JSON operations: parse a JSON string (parse), serialize a value to
                  a JSON string (stringify), extract a value by dot/bracket path (query), or
                  validate whether a string is valid JSON (validate)."
capability_id:   "builtin.json"
effect_type:     "read_only"
param_schema: {
  "type": "object",
  "properties": {
    "operation": {"type": "string", "enum": ["parse","stringify","query","validate"]},
    "data":      {"description": "JSON string or JSON value to process"},
    "path":      {"type": "string", "description": "Dot/bracket path for query operation"}
  },
  "required": ["operation", "data"],
  "additionalProperties": false
}
param_template:  {"operation": "{{operation}}", "data": "{{data}}"}
preconditions:   "operation required; data required"
error_handling:  "invalid JSON for parse/query → tool error; path not found in query → null"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

### Step 15.2 — ToolSkill: `ts-json-query` (class 13)

```
name:        "ts-json-query"
tool_name:   "json"
description: "Executor binding for json query operation. Required: operation='query', data
              (JSON string or value), path (dot/bracket path). Returns value at path or null."
param_schema: {
  "type": "object",
  "properties": {
    "operation": {"type": "string", "enum": ["query"]},
    "data":      {},
    "path":      {"type": "string"}
  },
  "required": ["operation", "data", "path"],
  "additionalProperties": false
}
param_template:  {"operation": "query", "data": "{{data}}", "path": "{{path}}"}
preconditions:   "data must be valid JSON; path must not be empty"
error_handling:  "invalid JSON → tool error; path not found → null (not a tool error)"
category:        "data"
source:          "system"
validation_status: "validated"
```

### Step 15.3 — PythonCode: `pc-exec-json-query` (class 22)

```
name:        "pc-exec-json-query"
description: "Orchestrator executor: calls __execute_action__ for json query operation.
              Input: data (JSON value or string), path (dot-separated path string)."
content: |
  # Orchestrator executor body.
  _data = {{vars.slot0}}
  _path = "{{vars.slot1}}"
  result = __execute_action__("json", {"operation": "query", "data": _data, "path": _path})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 15.4 — Leaf Skill: `skill-json-query` (class 1)

```
name:        "skill-json-query"
class_code:  1
description: "Leaf skill: how to extract a value from a JSON structure by path."
body: |
  Use `ts-json-query` (via pc-exec-json-query) to extract a specific field from a JSON
  structure. Provide the data and a dot-separated path (e.g. 'user.address.city' or
  'items.0.name'). Returns null if the path does not exist. For multi-field extraction,
  use pc-json-extract-field PythonCode instead.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 15.5 — Recipe: `json-query` (class 21)

> **Tier:** 0 — deterministic path extraction.

```
name:        "json-query"
description: "Extract a value from a JSON structure by path."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-json-query>"],
    "label":   "Pre-load ts-json-query ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-json-query>"],
    "label":   "PythonCode calls __execute_action__(json, {operation:query, data, path})"
  }
]
intent_examples: [
  {"input": "extract the user name from this JSON",         "class": 2},
  {"input": "get the value at this JSON path",              "class": 1},
  {"input": "query this JSON for the id field",             "class": 2},
  {"input": "json query items.0.name",                      "class": 1},
  {"input": "extract nested field from API response",       "class": 2},
  {"input": "get value at json path user.email",            "class": 1},
  {"input": "extract the status field from this response",  "class": 2},
  {"input": "json path extraction",                         "class": 1}
]
source: "system"
validation_status: "validated"
```

### Step 15.6 — ToolSkill: `ts-json-stringify` (class 13)

```
name:        "ts-json-stringify"
tool_name:   "json"
description: "Executor binding for json stringify and parse operations. Required: operation
              ('stringify' or 'parse'), data. Stringify → formatted JSON string; parse →
              structured value from JSON string."
param_schema: {
  "type": "object",
  "properties": {
    "operation": {"type": "string", "enum": ["stringify","parse"]},
    "data":      {}
  },
  "required": ["operation", "data"],
  "additionalProperties": false
}
param_template:  {"operation": "{{operation}}", "data": "{{data}}"}
preconditions:   "data must be valid for the selected operation"
error_handling:  "invalid JSON string for parse → tool error"
category:        "data"
source:          "system"
validation_status: "validated"
```

### Step 15.7 — ToolSkill: `ts-json-validate` (class 13)

> Separate ToolSkill for validation only.

```
name:        "ts-json-validate"
tool_name:   "json"
description: "Executor binding for json validate operation. Required: operation='validate',
              data (string to check). Returns {valid: bool, error: string|null}."
param_schema: {
  "type": "object",
  "properties": {
    "operation": {"type": "string", "enum": ["validate"]},
    "data":      {"type": "string"}
  },
  "required": ["operation", "data"],
  "additionalProperties": false
}
param_template:  {"operation": "validate", "data": "{{data}}"}
preconditions:   []
error_handling:  "returns {valid: false, error: ...} for invalid JSON — never a tool error"
category:        "data"
source:          "system"
validation_status: "validated"
```

### Step 15.8 — PythonCode: `pc-exec-json-stringify` (class 22)

```
name:        "pc-exec-json-stringify"
description: "Orchestrator executor: calls __execute_action__ for json stringify or parse.
              Input: operation ('stringify' or 'parse'), data."
content: |
  # Orchestrator executor body.
  _operation = "{{vars.slot0}}"
  _data = {{vars.slot1}}
  result = __execute_action__("json", {"operation": _operation, "data": _data})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 15.9 — PythonCode: `pc-exec-json-validate` (class 22)

```
name:        "pc-exec-json-validate"
description: "Orchestrator executor: calls __execute_action__ to validate a JSON string.
              Input: data (string). Output: {valid, error}."
content: |
  # Orchestrator executor body.
  _data = "{{vars.slot0}}"
  result = __execute_action__("json", {"operation": "validate", "data": _data})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 15.10 — Leaf Skill: `skill-json-stringify` (class 1)

> One grain: serialize a value to a JSON string.

```
name:        "skill-json-stringify"
class_code:  1
description: "Leaf skill: how to convert a value to a formatted JSON string."
body: |
  Use `ts-json-stringify` with operation='stringify' (via pc-exec-json-stringify) to
  format a structured value as a human-readable JSON string for display or for writing
  to a file. The result is pretty-printed.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 15.11 — Leaf Skill: `skill-json-parse` (class 1)

> Separate grain: parse a JSON string to a structured value.

```
name:        "skill-json-parse"
class_code:  1
description: "Leaf skill: how to parse a JSON string into a structured value."
body: |
  Use `ts-json-stringify` with operation='parse' (via pc-exec-json-stringify) when you
  have a raw JSON string (e.g. from a tool response body) and need to work with it as
  a structured value. The result can then be queried with ts-json-query or
  pc-json-extract-field.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 15.12 — Leaf Skill: `skill-json-validate` (class 1)

> Separate grain: check if a string is valid JSON.

```
name:        "skill-json-validate"
class_code:  1
description: "Leaf skill: how to check whether a string is valid JSON."
body: |
  Use `ts-json-validate` (via pc-exec-json-validate) to check whether a string is
  syntactically valid JSON before attempting to parse or process it. Returns {valid: bool,
  error: string|null}. Useful as a guard before running json-query or json-parse.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 15.13 — Recipe: `json-stringify` (class 21)

> **Tier:** 0 — deterministic stringify/parse.

```
name:        "json-stringify"
description: "Stringify or parse a JSON value."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-json-stringify>"],
    "label":   "Pre-load ts-json-stringify ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-json-stringify>"],
    "label":   "PythonCode calls __execute_action__(json, {operation, data})"
  }
]
intent_examples: [
  {"input": "format this as JSON",              "class": 1},
  {"input": "stringify this object",            "class": 1},
  {"input": "parse this JSON string",           "class": 1},
  {"input": "pretty print this JSON",           "class": 1},
  {"input": "convert this to a JSON string",    "class": 2},
  {"input": "json stringify",                   "class": 1},
  {"input": "serialize this to JSON",           "class": 1},
  {"input": "format this JSON structure",       "class": 1}
]
source: "system"
validation_status: "validated"
```


### Step 15.14 — Recipe: `json-parse` (class 21)

> **Tier:** 0 — deterministic JSON string parse. One recipe per operation variant.
> Parsing a JSON string is the inverse of stringify and a very common distinct use case.

```
name:        "json-parse"
description: "Parse a JSON string into a structured value."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-json-stringify>"],
    "label":   "Pre-load ts-json-stringify ToolSkill binding (handles parse operation)"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-json-stringify>"],
    "label":   "PythonCode calls __execute_action__(json, {operation:'parse', data})"
  }
]
intent_examples: [
  {"input": "parse this JSON",                     "class": 1},
  {"input": "decode this JSON",                    "class": 1},
  {"input": "convert this JSON string to a value", "class": 1},
  {"input": "json parse",                          "class": 1},
  {"input": "deserialize this JSON response",      "class": 2},
  {"input": "interpret this JSON payload",         "class": 2},
  {"input": "turn this JSON text into an object",  "class": 2},
  {"input": "parse the API response body as JSON", "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Step 15.15 — Recipe: `json-validate` (class 21)

> **Tier:** 0 — deterministic JSON validation. One recipe per operation variant.

```
name:        "json-validate"
description: "Validate whether a string is valid JSON."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-json-validate>"],
    "label":   "Pre-load ts-json-validate ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-json-validate>"],
    "label":   "PythonCode calls __execute_action__(json, {operation:'validate', data})"
  }
]
intent_examples: [
  {"input": "is this valid JSON",                "class": 1},
  {"input": "validate this JSON string",         "class": 1},
  {"input": "check if this is valid JSON",       "class": 1},
  {"input": "json validate",                     "class": 1},
  {"input": "is this JSON correct",              "class": 1},
  {"input": "verify this JSON syntax",           "class": 1},
  {"input": "check this JSON before using it",   "class": 2},
  {"input": "is this a valid JSON payload",      "class": 1}
]
source: "system"
validation_status: "validated"
```




## Step 15.x.1 — JSON Parse-and-Query Combined Recipe

> **`json-parse-and-query`** is a Tier-0 combined recipe for the very common pattern of
> taking a JSON string and immediately extracting a specific field from it. This two-step
> pattern (parse → query) is fully deterministic — the field path is pre-baked in vars.

### Leaf Skill: `skill-json-parse-and-query` (class 1)

```
name:        "skill-json-parse-and-query"
class_code:  1
description: "Leaf skill: how to parse a JSON string and immediately extract a field value."
body: |
  Use the two-step parse + query pattern when you receive a raw JSON string (e.g. from
  an HTTP response body) and immediately need a specific field. The pattern:
  1. Call ts-json (operation='parse') to get the structured object (via pc-exec-json-stringify).
  2. Call ts-json (operation='query') with a dot-path to extract the field.

  Alternatively, use pc-json-extract-field (pure Python) if the json tool is not bound.
  Always validate with json-validate before parse if the source is external or user-supplied.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Recipe: `json-parse-and-query` (class 21)

> **Tier:** 0 — field path pre-baked, fully deterministic. No LLM needed.
> Both the JSON string and the field path come from vars. The orchestrator parses then queries.

```
name:        "json-parse-and-query"
description: "Parse a JSON string and immediately extract a field by dot-path — both pre-baked in vars."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-json-validate>"],
    "label":   "Pre-load ts-json-validate ToolSkill binding (validate first)"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-json-validate>"],
    "label":   "PythonCode validates the JSON string before proceeding"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-json-query>"],
    "label":   "Pre-load ts-json-query ToolSkill binding"
  },
  {
    "step_id": "step-4",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-json-query>"],
    "label":   "PythonCode calls __execute_action__(json, {operation:'query', path:slot1}) on parsed data"
  }
]
intent_examples: [
  {"input": "get the field X from this JSON response",          "class": 1},
  {"input": "extract the value from this JSON payload",         "class": 2},
  {"input": "parse this API response and get the status field", "class": 2},
  {"input": "json parse and extract",                           "class": 1},
  {"input": "parse JSON and read field X",                      "class": 1},
  {"input": "extract data.items from this JSON",                "class": 2},
  {"input": "get the nested field from this JSON string",       "class": 2},
  {"input": "json parse then query path",                       "class": 1},
  {"input": "decode and extract this value",                    "class": 2},
  {"input": "parse this payload and read the error field",      "class": 2}
]
source: "system"
validation_status: "validated"
```

---


---

## Step 16 — `builtin.skill_list` / `builtin.skill_install` / `builtin.skill_remove` (Skill Management)

> Three related tools that let the agent inspect and manage the installed skill library.
> `skill_list` is Tier 0 (deterministic listing). `skill_install` and `skill_remove` require
> LLM confirmation (Tier 1) — irreversible side effects.

---

### Step 16.1 — Tool: `builtin.skill_list` (class 0)

```
capability_id: "builtin.skill_list"
name:          "skill_list"
description:   "List all skills currently installed in the active scope."
effect_type:   "Read"
param_schema:
  type: object
  properties:
    scope:
      type: string
      description: "Scope filter: 'all' | 'user' | 'system'. Defaults to 'all'."
  required: []
source: "system"
validation_status: "validated"
```

### Step 16.2 — Tool: `builtin.skill_install` (class 0)

```
capability_id: "builtin.skill_install"
name:          "skill_install"
description:   "Install a new skill from a URL or local path, entering the Q1/Q2 pipeline."
effect_type:   "Write"
param_schema:
  type: object
  properties:
    source_url:
      type: string
      description: "URL or local file path pointing to the skill manifest."
    scope:
      type: string
      description: "Target scope: 'user' (default) or 'system'."
  required: ["source_url"]
source: "system"
validation_status: "validated"
```

### Step 16.3 — Tool: `builtin.skill_remove` (class 0)

```
capability_id: "builtin.skill_remove"
name:          "skill_remove"
description:   "Remove an installed skill by name. Irreversible."
effect_type:   "Write"
param_schema:
  type: object
  properties:
    skill_name:
      type: string
      description: "Name of the skill to remove."
    scope:
      type: string
      description: "Scope the skill belongs to: 'user' | 'system'. Defaults to 'user'."
  required: ["skill_name"]
source: "system"
validation_status: "validated"
```

---

### Step 16.4 — ToolSkill: `ts-skill-list` (class 13)

```
name:        "ts-skill-list"
tool_name:   "skill_list"
description: "ToolSkill binding for builtin.skill_list — deterministic scope-filtered listing."
content: |
  Tool: builtin.skill_list
  Effect: Read — returns a JSON array of installed skills.

  Parameters:
  - scope (string, optional): 'all' (default) | 'user' | 'system'. Use 'user' when the user
    wants to see what they have installed. Use 'system' to inspect system-provided builtins.

  Output format:
    [{name, class_code, description, source, validation_status, installed_at}]

  Scope isolation: a 'user' scope call never returns system-only components. The agent
  cannot modify system-scope skills without elevated authority.

  When to use:
  - Before installing a skill, list first to check whether it already exists.
  - When the user asks "what skills do I have?"
  - As the first step in any skill management recipe.
source: "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator"]
```

### Step 16.5 — ToolSkill: `ts-skill-install` (class 13)

```
name:        "ts-skill-install"
tool_name:   "skill_install"
description: "ToolSkill binding for builtin.skill_install — installs a skill from URL/path."
content: |
  Tool: builtin.skill_install
  Effect: Write — installs a skill, creating a pending component that enters Q1 → Q2.

  Parameters:
  - source_url (string, required): URL (https://) or absolute local path to a skill manifest
    YAML/JSON. Remote URLs are fetched; the response must be a valid component manifest.
  - scope (string, optional): 'user' (default) | 'system'.

  Post-install state: the skill enters validation_status='pending' and goes through Q1.
  If Q1 fails, the install is rejected and logged. Q2 graduation is required before the
  skill is usable by the agent.

  Safety note: always confirm with the user before installing from an unknown source URL.
  Skills can contain PythonCode bodies that will execute in the orchestrator sandbox.
source: "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator"]
```

### Step 16.6 — ToolSkill: `ts-skill-remove` (class 13)

```
name:        "ts-skill-remove"
tool_name:   "skill_remove"
description: "ToolSkill binding for builtin.skill_remove — removes a skill by name."
content: |
  Tool: builtin.skill_remove
  Effect: Write — permanently removes a skill from the scope. Irreversible.

  Parameters:
  - skill_name (string, required): exact name of the skill to remove.
  - scope (string, optional): 'user' (default) | 'system'.

  Safety invariants:
  - System-scope skills cannot be removed by user-scope calls.
  - Removal of a skill that is referenced by an active recipe will fail with an error
    listing the dependent recipes. Resolve dependencies first.
  - Always confirm with the user before removal — this cannot be undone without
    reinstalling.
source: "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator"]
```

---

### Step 16.7 — PythonCode: `pc-exec-skill-list` (class 22)

> Orchestrator executor for `builtin.skill_list`. Dispatches the tool deterministically —
> the only actor that calls `__execute_action__` for a Tier-0 skill-list recipe.

```
name:        "pc-exec-skill-list"
class_code:  22
description: "Orchestrator executor: calls __execute_action__ to list installed skills.
              Input: scope (string). Output: [{name, class_code, …}]."
content: |
  # Orchestrator executor body.
  _scope = "{{vars.slot0}}" if "{{vars.slot0}}" else "all"
  result = __execute_action__("skill_list", {"scope": _scope})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 16.8 — Leaf Skill: `skill-skill-list` (class 1)

> Grain: enumerate installed skills, optionally filtered by scope.

```
name:        "skill-skill-list"
class_code:  1
description: "Leaf skill: how to list installed skills in the active scope."
body: |
  Use `ts-skill-list` (via pc-exec-skill-list) to retrieve a JSON array of all installed
  skills. Pass scope='user' to see only user-installed skills. Pass scope='system' to
  inspect system builtins. Omit scope (or pass 'all') to see everything.
  Check the returned array before deciding to install a skill — avoid duplicates.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator"]
```

### Step 16.9 — Leaf Skill: `skill-skill-install` (class 1)

> Grain: install a new skill from a URL, respecting the Q1/Q2 pipeline.

```
name:        "skill-skill-install"
class_code:  1
description: "Leaf skill: how to install a new skill from a URL or local path."
body: |
  Use `ts-skill-install` to fetch and register a skill manifest. Always:
  1. Run `ts-skill-list` first to confirm the skill does not already exist.
  2. Confirm the source URL with the user before proceeding.
  3. After install, inform the user the skill enters validation_status='pending' and
     cannot be used until Q1 and Q2 pass. Do not promise immediate availability.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator"]
```

### Step 16.10 — Leaf Skill: `skill-skill-remove` (class 1)

> Grain: remove an installed skill, confirming irreversibility.

```
name:        "skill-skill-remove"
class_code:  1
description: "Leaf skill: how to safely remove an installed skill."
body: |
  Use `ts-skill-remove` to permanently remove a skill by name. Always:
  1. Run `ts-skill-list` first to confirm the skill exists and note its scope.
  2. Confirm with the user that removal is intended and irreversible.
  3. If the tool returns a dependency error (recipes reference this skill), resolve those
     first or inform the user of the blocker.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator"]
```

---

### Step 16.11 — Domain Skill: `skill-skills` (class 2)

```
name:        "skill-skills"
class_code:  2
description: "Domain skill: skill management — list, install, remove."
body: |
  Skill management gives the agent and user visibility and control over the installed
  skill library. Use the right grain for each task:

  Listing skills:
  - skill-skill-list: enumerate the installed skill library (always start here)

  Installing a skill:
  - skill-skill-install: fetch a manifest from URL/path, confirm with user, enter Q1/Q2

  Removing a skill:
  - skill-skill-remove: confirm with user, check for dependent recipes, then remove

  Safety rules:
  - Never install from an untrusted URL without explicit user confirmation.
  - Never remove without explicit user confirmation — removal is irreversible.
  - System-scope skills cannot be modified from user-scope authority.
  - After install, the skill is 'pending' — not usable until Q2 graduates it.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator"]
```

---

### Step 16.12 — Recipe: `skill-list` (class 21)

> **Tier:** 0 — deterministic listing, no LLM needed.

```
name:        "skill-list"
description: "List all installed skills, optionally filtered by scope."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-skill-list>"],
    "label":   "Pre-load ts-skill-list ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-skill-list>"],
    "label":   "PythonCode calls __execute_action__(skill_list, {scope})"
  }
]
intent_examples: [
  {"input": "list my skills",                "class": 1},
  {"input": "what skills are installed",     "class": 1},
  {"input": "show me available skills",      "class": 1},
  {"input": "which skills do I have",        "class": 1},
  {"input": "list system skills",            "class": 2},
  {"input": "skill list",                    "class": 1},
  {"input": "show all installed skills",     "class": 1},
  {"input": "what capabilities are loaded",  "class": 2}
]
source: "system"
validation_status: "validated"
```

### Step 16.13 — Recipe: `skill-install` (class 21)

> **Tier:** 1 — LLM confirms source URL and communicates Q1/Q2 pipeline to user.

```
name:        "skill-install"
description: "Install a new skill from a URL, with user confirmation."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-skill-install>"],
    "label":   "Load skill-skill-install leaf skill body (install procedure)"
  },
  {
    "step_id": "step-2",
    "type":    "llm",
    "label":   "LLM confirms URL with user, explains pending state, calls ts-skill-install"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-skill-list>", "<uuid:ts-skill-install>"],
    "label":   "Pre-load ToolSkill bindings for list (pre-check) and install"
  }
]
intent_examples: [
  {"input": "install a skill from this URL",       "class": 1},
  {"input": "add a new skill",                     "class": 1},
  {"input": "install skill from https://...",      "class": 1},
  {"input": "load skill from local path",          "class": 2},
  {"input": "install this skill",                  "class": 1},
  {"input": "add skill from this path",            "class": 2},
  {"input": "skill install",                       "class": 1},
  {"input": "set up this new skill",               "class": 2}
]
source: "system"
validation_status: "validated"
```

### Step 16.14 — Recipe: `skill-remove` (class 21)

> **Tier:** 1 — LLM confirms name and scope, explains irreversibility.

```
name:        "skill-remove"
description: "Remove an installed skill by name, with user confirmation."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-skill-remove>"],
    "label":   "Load skill-skill-remove leaf skill body (removal procedure)"
  },
  {
    "step_id": "step-2",
    "type":    "llm",
    "label":   "LLM confirms skill name, warns about irreversibility, calls ts-skill-remove"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-skill-list>", "<uuid:ts-skill-remove>"],
    "label":   "Pre-load ToolSkill bindings for list (pre-check) and remove"
  }
]
intent_examples: [
  {"input": "remove skill X",                    "class": 1},
  {"input": "uninstall skill",                   "class": 1},
  {"input": "delete this skill",                 "class": 1},
  {"input": "remove my custom skill",            "class": 2},
  {"input": "skill remove",                      "class": 1},
  {"input": "uninstall this skill from my agent","class": 2},
  {"input": "delete skill by name",              "class": 1},
  {"input": "remove the skill named X",          "class": 1}
]
source: "system"
validation_status: "validated"
```

---

## Step 17 — `builtin.trigger_create` / `builtin.trigger_list` / `builtin.trigger_remove` (Trigger Management)

> Triggers are scheduled or event-driven run requests. `trigger_list` is Tier 0
> (deterministic). `trigger_create` and `trigger_remove` are Tier 1 — both have
> ExternalWrite effects and require user confirmation.

---

### Step 17.1 — Tool: `builtin.trigger_create` (class 0)

```
capability_id: "builtin.trigger_create"
name:          "trigger_create"
description:   "Create a new scheduled or event-driven trigger for a recipe or task."
effect_type:   "ExternalWrite"
param_schema:
  type: object
  properties:
    name:
      type: string
      description: "Human-readable trigger name."
    schedule:
      type: string
      description: "Cron expression ('0 9 * * 1') or interval ('every 1h', 'every 30m')."
    recipe_name:
      type: string
      description: "Name of the recipe to invoke on trigger."
    payload:
      type: object
      description: "Optional input vars to pass to the recipe at trigger time."
  required: ["name", "schedule", "recipe_name"]
source: "system"
validation_status: "validated"
```

### Step 17.2 — Tool: `builtin.trigger_list` (class 0)

```
capability_id: "builtin.trigger_list"
name:          "trigger_list"
description:   "List all configured triggers in the active scope."
effect_type:   "Read"
param_schema:
  type: object
  properties:
    scope:
      type: string
      description: "Scope filter: 'all' (default) | 'user' | 'system'."
  required: []
source: "system"
validation_status: "validated"
```

### Step 17.3 — Tool: `builtin.trigger_remove` (class 0)

```
capability_id: "builtin.trigger_remove"
name:          "trigger_remove"
description:   "Remove a trigger by name. Irreversible — the scheduled task stops immediately."
effect_type:   "ExternalWrite"
param_schema:
  type: object
  properties:
    trigger_name:
      type: string
      description: "Name of the trigger to remove."
  required: ["trigger_name"]
source: "system"
validation_status: "validated"
```

---

### Step 17.4 — ToolSkill: `ts-trigger-create` (class 13)

```
name:        "ts-trigger-create"
tool_name:   "trigger_create"
description: "ToolSkill binding for builtin.trigger_create — schedule a recipe invocation."
content: |
  Tool: builtin.trigger_create
  Effect: ExternalWrite — registers a persistent scheduled run request.

  Parameters:
  - name (string, required): human-readable trigger name. Must be unique in scope.
  - schedule (string, required): either a 5-field cron expression ('0 9 * * 1' = every
    Monday 9am) or a plain-English interval ('every 1h', 'every 30m', 'every day at 9am').
    The runtime normalizes interval syntax to cron internally.
  - recipe_name (string, required): the Recipe to invoke. Must be installed and
    validation_status='validated'.
  - payload (object, optional): key-value vars passed as input slots to the recipe.

  Cron field order: minute hour day-of-month month day-of-week.
  Examples:
    '0 9 * * 1'    → every Monday at 09:00
    '*/15 * * * *' → every 15 minutes
    'every 1h'     → every hour on the hour

  Safety: triggers run with the authority of the creating session's scope. They cannot
  escalate privilege beyond the scope in which they were created.
source: "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator"]
```

### Step 17.5 — ToolSkill: `ts-trigger-list` (class 13)

```
name:        "ts-trigger-list"
tool_name:   "trigger_list"
description: "ToolSkill binding for builtin.trigger_list — list all configured triggers."
content: |
  Tool: builtin.trigger_list
  Effect: Read — returns all triggers in scope as a JSON array.

  Parameters:
  - scope (string, optional): 'all' | 'user' | 'system'. Defaults to 'all'.

  Output format:
    [{name, schedule, recipe_name, payload, created_at, last_fired_at, next_fire_at}]

  Scope isolation: user-scope triggers are isolated from system-scope ones.
  Always list before creating to avoid duplicate trigger names.
source: "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator"]
```

### Step 17.6 — ToolSkill: `ts-trigger-remove` (class 13)

```
name:        "ts-trigger-remove"
tool_name:   "trigger_remove"
description: "ToolSkill binding for builtin.trigger_remove — remove a scheduled trigger."
content: |
  Tool: builtin.trigger_remove
  Effect: ExternalWrite — permanently removes the trigger. Stops immediately; any
  pending next-fire for this trigger is discarded.

  Parameters:
  - trigger_name (string, required): exact name of the trigger to remove.

  Safety:
  - Always confirm with the user before removing a trigger — the scheduled task will
    stop and cannot be recovered (only re-created from scratch).
  - Removing a trigger does not remove the recipe it pointed to.
source: "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator"]
```

---

### Step 17.7 — PythonCode: `pc-exec-trigger-list` (class 22)

> Orchestrator executor for `builtin.trigger_list`. Tier-0 dispatch.

```
name:        "pc-exec-trigger-list"
class_code:  22
description: "Orchestrator executor: calls __execute_action__ to list configured triggers.
              Input: scope (string). Output: [{name, schedule, recipe_name, …}]."
content: |
  # Orchestrator executor body.
  _scope = "{{vars.slot0}}" if "{{vars.slot0}}" else "all"
  result = __execute_action__("trigger_list", {"scope": _scope})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 17.8 — Leaf Skill: `skill-trigger-list` (class 1)

> Grain: enumerate configured triggers.

```
name:        "skill-trigger-list"
class_code:  1
description: "Leaf skill: how to list all configured triggers in the active scope."
body: |
  Use `ts-trigger-list` (via pc-exec-trigger-list) to retrieve a JSON array of all
  configured triggers. Inspect schedule, recipe_name, last_fired_at, and next_fire_at
  to give the user a clear picture of what is scheduled. Always list before creating
  to avoid name collisions.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator"]
```

### Step 17.9 — Leaf Skill: `skill-trigger-create` (class 1)

> Grain: schedule a new trigger for a recipe, confirming cron syntax.

```
name:        "skill-trigger-create"
class_code:  1
description: "Leaf skill: how to create a scheduled trigger for a recipe."
body: |
  Use `ts-trigger-create` to register a recurring or one-off trigger. Always:
  1. List existing triggers first (ts-trigger-list) to check for name conflicts.
  2. Confirm the schedule with the user — translate their natural-language request
     ('every Monday morning') into a cron expression ('0 9 * * 1') and confirm it.
  3. Confirm the recipe_name exists and is validation_status='validated'.
  4. Optionally confirm any payload vars with the user before creating.
  Triggers have ExternalWrite effect — the user should explicitly approve creation.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator"]
```

### Step 17.10 — Leaf Skill: `skill-trigger-remove` (class 1)

> Grain: remove a trigger safely, confirming irreversibility.

```
name:        "skill-trigger-remove"
class_code:  1
description: "Leaf skill: how to remove a configured trigger."
body: |
  Use `ts-trigger-remove` to permanently remove a scheduled trigger by name. Always:
  1. List triggers first to confirm the trigger exists and show the user its schedule.
  2. Confirm with the user that removal is intended — the trigger stops immediately
     and cannot be recovered without re-creating it from scratch.
  Triggers have ExternalWrite effect — explicit user approval is required.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator"]
```

---

### Step 17.11 — Domain Skill: `skill-triggers` (class 2)

```
name:        "skill-triggers"
class_code:  2
description: "Domain skill: trigger management — list, create, remove scheduled runs."
body: |
  Triggers are persistent scheduled invocations of recipes. Use the right grain:

  LISTING TRIGGERS:
  - skill-trigger-list: list ALL triggers (all scopes) — always start here.
  - skill-trigger-list-active: list ONLY currently active/enabled triggers (Tier-0).
  - skill-trigger-list-scheduled: list ONLY scheduled (cron/time-based) triggers (Tier-0).

  Decision: if user says "what triggers do I have" → trigger-list.
  If user says "what is currently active/running" → trigger-list-active.
  If user says "what is scheduled/recurring" → trigger-list-scheduled.

  CREATING A TRIGGER:
  - skill-trigger-create: confirm schedule (cron) + recipe_name + payload with user.
    Translate natural language schedule to cron and verify before committing.

  REMOVING A TRIGGER:
  - skill-trigger-remove: confirm name, warn about immediate stoppage, then remove.
  - skill-trigger-list + skill-trigger-remove-by-name: when name is known exactly
    (use pc-exec-trigger-resolve-and-remove — find by exact name then remove).

  Safety rules:
  - trigger_create and trigger_remove both have ExternalWrite effect — require explicit
    user confirmation for each.
  - Triggers run with the creating session's authority; they cannot escalate privilege.
  - A trigger referencing a recipe that is later removed will fail at fire time —
    inform the user of this risk when removing recipes.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator"]
```

---

### Step 17.12 — Recipe: `trigger-list` (class 21)

> **Tier:** 0 — deterministic listing, no LLM needed.

```
name:        "trigger-list"
description: "List all configured triggers in the active scope."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-trigger-list>"],
    "label":   "Pre-load ts-trigger-list ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-trigger-list>"],
    "label":   "PythonCode calls __execute_action__(trigger_list, {scope})"
  }
]
intent_examples: [
  {"input": "list my triggers",                  "class": 1},
  {"input": "what triggers are configured",      "class": 1},
  {"input": "show scheduled tasks",              "class": 1},
  {"input": "what is scheduled",                 "class": 1},
  {"input": "list system triggers",              "class": 2},
  {"input": "trigger list",                      "class": 1},
  {"input": "show me all my scheduled runs",     "class": 2},
  {"input": "what recipes are scheduled to run", "class": 2}
]
source: "system"
validation_status: "validated"
```

### Step 17.13 — Recipe: `trigger-create` (class 21)

> **Tier:** 1 — LLM translates schedule, confirms with user, ExternalWrite effect.

```
name:        "trigger-create"
description: "Create a scheduled trigger for a recipe, with user confirmation."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-trigger-create>"],
    "label":   "Load skill-trigger-create leaf skill body (creation procedure)"
  },
  {
    "step_id": "step-2",
    "type":    "llm",
    "label":   "LLM translates schedule to cron, confirms with user, calls ts-trigger-create"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-trigger-list>", "<uuid:ts-trigger-create>"],
    "label":   "Pre-load ToolSkill bindings for list (pre-check) and create"
  }
]
intent_examples: [
  {"input": "create a trigger to run X every morning",        "class": 1},
  {"input": "schedule recipe X every Monday",                 "class": 1},
  {"input": "set up a daily trigger",                         "class": 1},
  {"input": "run this recipe every hour",                     "class": 2},
  {"input": "trigger create",                                 "class": 1},
  {"input": "schedule this recipe every 15 minutes",          "class": 2},
  {"input": "create a cron trigger for this recipe",          "class": 1},
  {"input": "set up an hourly trigger for X",                 "class": 1},
  {"input": "automate this task to run weekly",               "class": 2},
  {"input": "schedule a recurring execution for this recipe", "class": 2}
]
source: "system"
validation_status: "validated"
```

### Step 17.14 — Recipe: `trigger-remove` (class 21)

> **Tier:** 1 — LLM confirms trigger name and warns user.

```
name:        "trigger-remove"
description: "Remove a configured trigger by name, with user confirmation."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-trigger-remove>"],
    "label":   "Load skill-trigger-remove leaf skill body (removal procedure)"
  },
  {
    "step_id": "step-2",
    "type":    "llm",
    "label":   "LLM confirms trigger name with user, warns about stoppage, calls ts-trigger-remove"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-trigger-list>", "<uuid:ts-trigger-remove>"],
    "label":   "Pre-load ToolSkill bindings for list (pre-check) and remove"
  }
]
intent_examples: [
  {"input": "remove trigger X",                       "class": 1},
  {"input": "delete this scheduled task",             "class": 1},
  {"input": "stop running recipe X",                  "class": 1},
  {"input": "cancel the daily trigger",               "class": 2},
  {"input": "trigger remove",                         "class": 1},
  {"input": "disable this scheduled trigger",         "class": 2},
  {"input": "stop the hourly trigger",                "class": 1},
  {"input": "delete the trigger named X",             "class": 1},
  {"input": "unschedule this recurring recipe",       "class": 2},
  {"input": "deactivate and remove this trigger",     "class": 2}
]
source: "system"
validation_status: "validated"
```

---

## Step 17.x — Trigger Remove-by-Name Pattern

> **Gap 10:** The existing `trigger-remove` is Tier 1 — LLM identifies the trigger by name.
> But when the name is already known exactly (e.g. from a previous `trigger-list` call whose
> result the orchestrator already holds), the LLM step is unnecessary.
>
> This section adds a `pc-exec-trigger-resolve-and-remove` PythonCode helper that:
> 1. Lists all triggers via `__execute_action__(trigger_list, {})`.
> 2. Finds the trigger whose name exactly matches the input.
> 3. Removes it via `__execute_action__(trigger_remove, {trigger_name: name})`.
>
> This is still Tier 1 because `trigger_remove` has ExternalWrite effect and benefits from
> user confirmation. However the LLM step is now ONLY for user confirmation — not for
> name disambiguation. The PythonCode step does the list+resolve+remove deterministically.

### Step 17.x.1 — PythonCode: `pc-exec-trigger-resolve-and-remove` (class 22)

> Orchestrator helper: list triggers, find by exact name, then remove.
> ExternalWrite — used inside a Tier-1 recipe where the LLM has already confirmed with
> the user. The PythonCode does the mechanical list-then-remove; the LLM step only
> confirms intent.

```
name:        "pc-exec-trigger-resolve-and-remove"
class_code:  22
description: "Orchestrator executor: lists all triggers, finds the one matching the given
              name exactly, and removes it. Input: trigger_name (string). Output:
              {removed: bool, trigger_name: string, error?: string}."
content: |
  # Orchestrator executor body. No I/O, no imports, no network.
  # IBS bakes in slot values before execution.
  _trigger_name = "{{vars.slot0}}"
  _list_result  = __execute_action__("trigger_list", {})
  _triggers     = _list_result.get("triggers", []) if isinstance(_list_result, dict) else []
  _found        = next((t for t in _triggers if t.get("name") == _trigger_name), None)
  if _found is None:
      result = {"removed": False, "trigger_name": _trigger_name,
                "error": f"No trigger named '{_trigger_name}' found"}
  else:
      _remove_result = __execute_action__("trigger_remove", {"trigger_name": _trigger_name})
      result = {"removed": True, "trigger_name": _trigger_name, "remove_result": _remove_result}
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 17.x.2 — Recipe: `trigger-remove-by-name` (class 21)

> **Tier:** 1 — ExternalWrite effect; user confirmation by LLM before the PythonCode acts.
> The split: LLM confirms intent with user (step-2), then PythonCode executes the
> list-find-remove deterministically (step-3). No ambiguous name resolution by LLM.

```
name:        "trigger-remove-by-name"
description: "Remove a trigger by exact name — LLM confirms intent, then PythonCode resolves and removes."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-trigger-remove>"],
    "label":   "Load skill-trigger-remove leaf skill body (safety procedure)"
  },
  {
    "step_id": "step-2",
    "type":    "llm",
    "label":   "LLM confirms trigger name with user and warns about irreversibility"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-trigger-list>", "<uuid:ts-trigger-remove>"],
    "label":   "Pre-load list + remove ToolSkill bindings"
  },
  {
    "step_id": "step-4",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-trigger-resolve-and-remove>"],
    "label":   "PythonCode: list triggers, find by exact name, remove — no LLM disambiguation"
  }
]
intent_examples: [
  {"input": "remove the trigger named X",                    "class": 1},
  {"input": "delete trigger X",                              "class": 1},
  {"input": "stop the trigger called X",                     "class": 1},
  {"input": "cancel trigger by name",                        "class": 1},
  {"input": "remove the scheduled trigger X",                "class": 2},
  {"input": "disable and remove trigger named X",            "class": 2},
  {"input": "delete this specific trigger by name",          "class": 1},
  {"input": "trigger remove by name",                        "class": 1}
]
source: "system"
validation_status: "validated"
```

---

## Step 17.x.3 — Trigger List Variant Recipes (Tier-0)

> Common use-case: the user asks to see only active triggers, or only scheduled triggers.
> `builtin.trigger_list` supports a `scope` parameter. These Tier-0 recipes pre-bake the
> scope value — no LLM needed to choose it.

### Step 17.x.3.1 — PythonCode: `pc-exec-trigger-list-active` (class 22)

```
name:        "pc-exec-trigger-list-active"
class_code:  22
description: "Orchestrator executor: calls __execute_action__(trigger_list, {scope:'active'})
              to list only currently active (running/enabled) triggers. No LLM needed."
content: |
  # Orchestrator executor body.
  result = __execute_action__("trigger_list", {"scope": "active"})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 17.x.3.2 — PythonCode: `pc-exec-trigger-list-scheduled` (class 22)

```
name:        "pc-exec-trigger-list-scheduled"
class_code:  22
description: "Orchestrator executor: calls __execute_action__(trigger_list, {scope:'scheduled'})
              to list only scheduled (cron/time-based) triggers. No LLM needed."
content: |
  # Orchestrator executor body.
  result = __execute_action__("trigger_list", {"scope": "scheduled"})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 17.x.3.3 — Leaf Skill: `skill-trigger-list-active` (class 1)

```
name:        "skill-trigger-list-active"
class_code:  1
description: "Leaf skill: how to list only currently active triggers."
body: |
  Use pc-exec-trigger-list-active to call trigger_list with scope='active'. Returns only
  triggers that are currently running or enabled. Use this when the user wants to know
  what is actively firing right now — not scheduled-but-paused entries.
  Compare with skill-trigger-list (all triggers) and skill-trigger-list-scheduled (cron).
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 17.x.3.4 — Leaf Skill: `skill-trigger-list-scheduled` (class 1)

```
name:        "skill-trigger-list-scheduled"
class_code:  1
description: "Leaf skill: how to list only scheduled (cron/time-based) triggers."
body: |
  Use pc-exec-trigger-list-scheduled to call trigger_list with scope='scheduled'. Returns
  only triggers that run on a cron or time-interval basis. Use when the user is asking
  about what is set up to run on a schedule, not manual/event triggers.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 17.x.3.5 — Recipe: `trigger-list-active` (class 21)

> **Tier:** 0 — scope pre-baked as 'active'. Deterministic — no LLM disambiguation needed.

```
name:        "trigger-list-active"
description: "List only currently active triggers (scope='active')."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-trigger-list>"],
    "label":   "Pre-load ts-trigger-list ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-trigger-list-active>"],
    "label":   "PythonCode calls __execute_action__(trigger_list, {scope:'active'})"
  }
]
intent_examples: [
  {"input": "show active triggers",                             "class": 1},
  {"input": "what triggers are currently running",              "class": 1},
  {"input": "list enabled triggers",                            "class": 1},
  {"input": "what is currently firing",                         "class": 2},
  {"input": "show me the active automations",                   "class": 2},
  {"input": "active trigger list",                              "class": 1},
  {"input": "what triggers are live right now",                 "class": 1},
  {"input": "show running triggers",                            "class": 1},
  {"input": "which triggers are enabled",                       "class": 1},
  {"input": "list triggers that are currently on",              "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Step 17.x.3.6 — Recipe: `trigger-list-scheduled` (class 21)

> **Tier:** 0 — scope pre-baked as 'scheduled'. Deterministic.

```
name:        "trigger-list-scheduled"
description: "List only scheduled (cron/time-based) triggers (scope='scheduled')."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-trigger-list>"],
    "label":   "Pre-load ts-trigger-list ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-trigger-list-scheduled>"],
    "label":   "PythonCode calls __execute_action__(trigger_list, {scope:'scheduled'})"
  }
]
intent_examples: [
  {"input": "show scheduled triggers",                          "class": 1},
  {"input": "what triggers run on a schedule",                  "class": 1},
  {"input": "list cron triggers",                               "class": 1},
  {"input": "what is scheduled to run",                         "class": 2},
  {"input": "show me my scheduled automations",                 "class": 2},
  {"input": "scheduled trigger list",                           "class": 1},
  {"input": "what runs on a timer",                             "class": 2},
  {"input": "list time-based triggers",                         "class": 1},
  {"input": "show recurring triggers",                          "class": 2},
  {"input": "what will run next based on schedule",             "class": 2}
]
source: "system"
validation_status: "validated"
```

---



## Step 18 — `builtin.spawn_subagent` (Child Agent Delegation)

> Spawns a child agent run delegated to a sub-goal. Always Tier 1 — §spawn_subagent-guard
> is a hard Q1 constraint: `llm_call_required` MUST be `true` for any recipe that
> references `builtin.spawn_subagent`.

---

### Step 18.1 — Tool: `builtin.spawn_subagent` (class 0)

```
capability_id: "builtin.spawn_subagent"
name:          "spawn_subagent"
description:   "Spawn a child agent run to handle a sub-goal or delegated procedure."
effect_type:   "ExternalWrite"
param_schema:
  type: object
  properties:
    goal:
      type: string
      description: "The task description or goal for the child agent."
    context:
      type: string
      description: "Optional additional context to pass to the child. Plain text."
    recipe_name:
      type: string
      description: "Optional: name of a recipe to seed the child's execution with."
    budget_tokens:
      type: integer
      description: "Optional token budget cap for the child run. Inherits parent default if absent."
  required: ["goal"]
source: "system"
validation_status: "validated"
```

---

### Step 18.2 — ToolSkill: `ts-spawn-subagent` (class 13)

```
name:        "ts-spawn-subagent"
tool_name:   "spawn_subagent"
description: "ToolSkill binding for builtin.spawn_subagent — delegate a task to a child agent."
content: |
  Tool: builtin.spawn_subagent
  Effect: ExternalWrite — creates a child agent run.

  Parameters:
  - goal (string, required): the sub-goal for the child. Be precise — the child has no
    access to parent conversation history unless you include it in 'context'.
  - context (string, optional): additional background text passed to the child. Include
    any file paths, decisions, or constraints the child needs.
  - recipe_name (string, optional): if you want the child to start from a known recipe
    path, pass its name here. The recipe must be validation_status='validated'.
  - budget_tokens (integer, optional): cap the child's token budget. Cannot exceed the
    parent's remaining budget.

  Scope isolation invariants:
  - The child runs in the same scope as the parent but cannot access parent-private
    session state or conversation history unless explicitly passed.
  - The child cannot escalate authority beyond the parent's capability grants.
  - Budget inheritance: if budget_tokens is omitted, the child inherits the parent
    session's default budget, not the parent's remaining balance.
  - The child's tool approvals are independent — the user may need to re-approve the
    same tool in the child's context.

  When to delegate:
  - The sub-task is self-contained and would not benefit from the parent's ongoing context.
  - The sub-task is long-running and you want to continue parent work in parallel.
  - You are implementing a named procedure that has a stable recipe shape.

  When NOT to delegate:
  - When the task requires back-and-forth with the parent's current state.
  - For trivial operations that take one or two tool calls.
source: "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator"]
```

---

### Step 18.3 — Leaf Skill: `skill-spawn-subagent` (class 1)

> Grain: delegate a self-contained sub-goal to a child agent run.

```
name:        "skill-spawn-subagent"
class_code:  1
description: "Leaf skill: how to spawn a child agent run for a delegated sub-task."
body: |
  Use `ts-spawn-subagent` to create a child agent run for a self-contained sub-goal.

  Before spawning:
  1. Ensure the goal is truly self-contained — include all necessary context in the
     'context' field since the child cannot see the parent conversation.
  2. Confirm with the user if the sub-task has any destructive or external effects.
  3. Set budget_tokens if the sub-task should be bounded.

  After spawning:
  - The child result is returned as a structured object. Check result.status for
    'completed' | 'failed' | 'budget_exceeded'.
  - If the child fails, report the failure reason to the user and decide whether to
    retry, rephrase the goal, or handle the sub-task in the parent context instead.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator"]
```

### Step 18.4 — Leaf Skill: `skill-spawn-named-procedure` (class 1)

> Grain: use spawn_subagent to execute a named procedure/recipe in a child.

```
name:        "skill-spawn-named-procedure"
class_code:  1
description: "Leaf skill: how to run a named recipe as a child agent procedure."
body: |
  Use `ts-spawn-subagent` with recipe_name set to invoke a known, stable procedure.

  Use this when:
  - You have a validated Recipe that encodes a complete procedure (e.g. 'file-patch',
    'memory-write', a user-installed skill recipe).
  - You want the child to follow that procedure's recipe structure exactly rather than
    improvise from a goal description.

  Pass relevant slot variables in 'context' as a structured key-value description:
    "vars: {slot0: '/path/to/file', slot1: 'search term'}"
  The child's recipe loader will extract these into its vars map.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator"]
```

---

### Step 18.5 — Domain Skill: `skill-subagent` (class 2)

```
name:        "skill-subagent"
class_code:  2
description: "Domain skill: child agent delegation via spawn_subagent."
body: |
  Delegation gives the parent agent a way to hand off a well-scoped sub-task to a
  child run with full tool access and its own budget.

  Choosing a delegation grain:
  - skill-spawn-subagent: general goal delegation — write a clear, self-contained goal
  - skill-spawn-named-procedure: procedure delegation — use an existing validated Recipe
  - skill-spawn-research: research/info-gathering delegation — returns structured summary
  - skill-spawn-coding: coding task delegation — file reads, patches, reports changes
  - skill-spawn-exploration: read-only deep analysis — returns catalogue or report
  - skill-spawn-query: focused single-question lookup — returns direct answer

  Decision guide:
  • Generic open-ended sub-goal → skill-spawn-subagent
  • Run a known recipe in a child → skill-spawn-named-procedure
  • Research / web lookups / memory searches → skill-spawn-research
  • File editing, debugging, writing code → skill-spawn-coding
  • Mapping codebase structure, tracing deps → skill-spawn-exploration
  • Single factual question needing 1-2 tool calls → skill-spawn-query

  Critical safety invariants (§spawn_subagent-guard):
  - Any Recipe binding spawn_subagent MUST have llm_call_required=true (hard Q1 rule).
    There is NO Tier-0 spawn recipe — the LLM must always be in the loop to frame
    the goal and confirm delegation.
  - Child cannot exceed parent scope or authority.
  - Budget inheritance is from the session default, not parent remaining balance.
  - Include all needed context explicitly — child has no parent conversation access.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator"]
```

---

### Step 18.6 — Recipe: `subagent-spawn` (class 21)

> **Tier:** 1 — §spawn_subagent-guard enforced. LLM MUST be in the loop.

```
name:        "subagent-spawn"
description: "Spawn a child agent for a delegated sub-task or named procedure."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-spawn-subagent>", "<uuid:skill-spawn-named-procedure>"],
    "label":   "Load spawn leaf skills (goal delegation + named procedure patterns)"
  },
  {
    "step_id": "step-2",
    "type":    "llm",
    "label":   "LLM frames the goal, decides generic-vs-recipe delegation, confirms with user, calls ts-spawn-subagent"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-spawn-subagent>"],
    "label":   "Pre-load ts-spawn-subagent ToolSkill binding"
  }
]
intent_examples: [
  {"input": "spawn a child agent to do X",                "class": 1},
  {"input": "delegate this task to a subagent",           "class": 1},
  {"input": "run procedure Y in a child session",         "class": 1},
  {"input": "create a child agent for this sub-task",     "class": 1},
  {"input": "use a subagent for this long-running task",  "class": 2},
  {"input": "subagent spawn",                             "class": 1},
  {"input": "hand off this work to a child agent",        "class": 2},
  {"input": "run this recipe in a child session",         "class": 2},
  {"input": "spawn subagent with this goal",              "class": 1},
  {"input": "delegate this to a parallel agent",          "class": 2}
]
source: "system"
validation_status: "validated"
```

---

## Step 18.x — Subagent Flavor-Specific Recipes

> **§spawn_subagent-guard applies to all recipes below** — all are `llm_call_required: true`.
> These recipes are NOT Tier-0. They specialise the generic `subagent-spawn` recipe by
> pre-framing the goal type so the LLM knows which kind of delegation is requested.
> One recipe per delegation flavour. The intent system routes here directly so the LLM
> already knows "this is a research delegation" before it composes the goal string.

### Step 18.x.1 — Leaf Skill: `skill-spawn-research` (class 1)

> Grain: delegate a focused information-gathering task to a child.

```
name:        "skill-spawn-research"
class_code:  1
description: "Leaf skill: how to delegate a research or information-gathering sub-task."
body: |
  Use `ts-spawn-subagent` with a goal written as a focused research question. Research
  delegation works best when:
  - The question is self-contained and answerable from memory, files, or web search.
  - You want the answer returned as a structured summary (not inline back-and-forth).
  - The child will need to call multiple tools (memory_search, glob, grep, or http).

  Frame the goal as a question: "Research and summarise X, focusing on Y. Return a
  structured summary with: key findings, relevant files/sources, open questions."
  Include all constraints in the context field — the child has no access to parent state.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator"]
```

---

### Step 18.x.2 — Leaf Skill: `skill-spawn-coding` (class 1)

> Grain: delegate a focused code-reading, code-writing, or debugging task to a child.

```
name:        "skill-spawn-coding"
class_code:  1
description: "Leaf skill: how to delegate a focused coding sub-task to a child agent."
body: |
  Use `ts-spawn-subagent` with a goal written as a concrete code task. Coding delegation
  works best when:
  - The task is scoped to a specific file, function, or module.
  - The child needs to read files, apply patches, and report a result.
  - The task is too long to inline and benefits from isolated execution.

  Frame the goal concretely: "Read /path/to/file, fix the bug described by X, write the
  corrected version back. Return the diff of changes made."
  Include all file paths and error descriptions in the context field.
  Set budget_tokens appropriately — coding tasks can be token-heavy.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator"]
```

---

### Step 18.x.3 — Leaf Skill: `skill-spawn-exploration` (class 1)

> Grain: delegate a deep read-only exploration of the codebase or workspace to a child.

```
name:        "skill-spawn-exploration"
class_code:  1
description: "Leaf skill: how to delegate a deep read-only workspace exploration task."
body: |
  Use `ts-spawn-subagent` with a goal written as a deep-analysis question. Exploration
  delegation works best when:
  - The task is read-only (no file writes, no shell execution with side effects).
  - You want the child to map out a codebase area, trace a dependency, or catalogue
    patterns across many files.
  - The result is a structured report or inventory.

  Frame the goal as an analysis assignment: "Explore all Rust files under crates/X/,
  identify all public trait definitions, and return a structured inventory with trait
  names, file paths, and method signatures."
  Explicitly state "read-only — do not modify any files" in the goal if needed.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator"]
```

---

### Step 18.x.4 — Leaf Skill: `skill-spawn-query` (class 1)

> Grain: delegate a focused single-question query that needs tool lookups to answer.

```
name:        "skill-spawn-query"
class_code:  1
description: "Leaf skill: how to delegate a focused lookup query to a child agent."
body: |
  Use `ts-spawn-subagent` when the user asks a specific factual question that requires
  one or two tool lookups (memory_search, grep, glob, or a quick http fetch) to answer.
  Query delegation avoids cluttering the parent context with intermediate tool results.

  Frame the goal as a direct question with expected output shape: "Find the current
  version of X in Cargo.toml and return it. Return only the version string."
  Keep goals short and unambiguous — the child will return a text result, not continue
  a conversation.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator"]
```

---

### Step 18.x.5 — Recipe: `subagent-research` (class 21)

> **Tier:** 1 — §spawn_subagent-guard. LLM frames the research goal.

```
name:        "subagent-research"
description: "Delegate a focused research or information-gathering task to a child agent."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-spawn-research>"],
    "label":   "Load research delegation leaf skill body"
  },
  {
    "step_id": "step-2",
    "type":    "llm",
    "label":   "LLM frames focused research goal string, sets context, calls ts-spawn-subagent"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-spawn-subagent>"],
    "label":   "Pre-load ts-spawn-subagent ToolSkill binding"
  }
]
intent_examples: [
  {"input": "research this topic using a child agent",      "class": 1},
  {"input": "have a subagent look this up",                 "class": 1},
  {"input": "delegate this research to a child",            "class": 1},
  {"input": "spawn a researcher subagent",                  "class": 1},
  {"input": "research X in a child session",                "class": 2},
  {"input": "use a subagent to find information about X",   "class": 2},
  {"input": "gather information on X via child agent",      "class": 2},
  {"input": "let a subagent research this and report back", "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Step 18.x.6 — Recipe: `subagent-coding` (class 21)

> **Tier:** 1 — §spawn_subagent-guard. LLM frames the coding task.

```
name:        "subagent-coding"
description: "Delegate a focused code-reading, code-writing, or debugging task to a child agent."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-spawn-coding>"],
    "label":   "Load coding delegation leaf skill body"
  },
  {
    "step_id": "step-2",
    "type":    "llm",
    "label":   "LLM scopes the code task, includes file paths + constraints in context, calls ts-spawn-subagent"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-spawn-subagent>"],
    "label":   "Pre-load ts-spawn-subagent ToolSkill binding"
  }
]
intent_examples: [
  {"input": "have a child agent fix this bug",              "class": 1},
  {"input": "delegate this coding task to a subagent",      "class": 1},
  {"input": "spawn a coder subagent to handle this",        "class": 1},
  {"input": "let a child agent write this code",            "class": 1},
  {"input": "use a subagent to refactor this file",         "class": 2},
  {"input": "have a child agent apply this patch",          "class": 2},
  {"input": "delegate the code changes to a child session", "class": 2},
  {"input": "subagent coding task",                         "class": 1}
]
source: "system"
validation_status: "validated"
```

---

### Step 18.x.7 — Recipe: `subagent-exploration` (class 21)

> **Tier:** 1 — §spawn_subagent-guard. LLM frames the exploration scope.

```
name:        "subagent-exploration"
description: "Delegate a deep read-only workspace or codebase exploration to a child agent."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-spawn-exploration>"],
    "label":   "Load exploration delegation leaf skill body"
  },
  {
    "step_id": "step-2",
    "type":    "llm",
    "label":   "LLM defines exploration scope and output format, calls ts-spawn-subagent"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-spawn-subagent>"],
    "label":   "Pre-load ts-spawn-subagent ToolSkill binding"
  }
]
intent_examples: [
  {"input": "have a subagent explore this codebase area",   "class": 1},
  {"input": "spawn an explorer to map out the structure",   "class": 1},
  {"input": "delegate a deep exploration to a child agent", "class": 1},
  {"input": "have a child agent catalogue this directory",  "class": 2},
  {"input": "explore the codebase with a subagent",         "class": 2},
  {"input": "subagent explore",                             "class": 1},
  {"input": "use a child agent to analyse code patterns",   "class": 2},
  {"input": "have a child agent trace this dependency",     "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Step 18.x.8 — Recipe: `subagent-query` (class 21)

> **Tier:** 1 — §spawn_subagent-guard. LLM frames the query.

```
name:        "subagent-query"
description: "Delegate a focused single-question lookup to a child agent."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-spawn-query>"],
    "label":   "Load query delegation leaf skill body"
  },
  {
    "step_id": "step-2",
    "type":    "llm",
    "label":   "LLM formulates focused single-question goal, expected output shape, calls ts-spawn-subagent"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-spawn-subagent>"],
    "label":   "Pre-load ts-spawn-subagent ToolSkill binding"
  }
]
intent_examples: [
  {"input": "ask a child agent to look this up",            "class": 1},
  {"input": "have a subagent answer this question",         "class": 1},
  {"input": "delegate this lookup to a child session",      "class": 1},
  {"input": "spawn a query subagent",                       "class": 1},
  {"input": "use a child agent to find the answer to X",    "class": 2},
  {"input": "subagent query",                               "class": 1},
  {"input": "let a child agent fetch this information",     "class": 2},
  {"input": "have a child agent check this value",          "class": 2}
]
source: "system"
validation_status: "validated"
```

---

## Step 19 — `builtin.echo` (Diagnostic Passthrough)

> `builtin.echo` is a diagnostic passthrough — it returns its input unchanged. It is
> used in tests and as a no-op stub during development. It has no user-facing Recipes.

---

### Step 19.1 — Tool: `builtin.echo` (class 0)

```
capability_id: "builtin.echo"
name:          "echo"
description:   "Diagnostic passthrough: returns input unchanged. For testing and stubs."
effect_type:   "Read"
param_schema:
  type: object
  properties:
    message:
      type: string
      description: "Any string. Returned verbatim in the tool response."
  required: ["message"]
source: "system"
validation_status: "validated"
```

### Step 19.2 — ToolSkill: `ts-echo` (class 13)

```
name:        "ts-echo"
tool_name:   "echo"
description: "ToolSkill binding for builtin.echo — diagnostic passthrough, no user-facing recipe."
content: |
  Tool: builtin.echo
  Effect: Read — returns the input message unchanged.

  Parameters:
  - message (string, required): any string value.

  Use cases (diagnostic / development only):
  - Confirm that the orchestrator's tool dispatch pipeline is functional.
  - Stub out a tool call during recipe development before the real tool is wired.
  - Verify that slot variable interpolation is working in a PythonCode executor.

  No user-facing recipe is defined for echo. Do not use echo in production recipe flows.
  If you find yourself routing user requests through echo, use the correct tool instead.
source: "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator"]
```

> **Note:** A minimal diagnostic recipe `echo-ping` is defined below for development and
> integration testing only. It is NOT intended for production use — route real tasks to
> appropriate domain recipes.

### Step 19.3 — PythonCode: `pc-exec-echo` (class 22)

```
name:        "pc-exec-echo"
description: "Orchestrator executor: calls __execute_action__ for builtin.echo (diagnostic
              passthrough). Input: message (string). Output: {message} — returned verbatim."
content: |
  # Diagnostic executor body. __execute_action__ provided by runtime sandbox.
  _message = "{{vars.slot0}}"
  result = __execute_action__("echo", {"message": _message})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 19.4 — Recipe: `echo-ping` (class 21)

> **Tier:** 0 — diagnostic passthrough. Used to verify the orchestrator tool dispatch
> pipeline is functional without side effects.

```
name:        "echo-ping"
description: "Diagnostic: echo a message through the tool dispatch pipeline (builtin.echo)."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-echo>"],
    "label":   "Pre-load ts-echo ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-echo>"],
    "label":   "PythonCode calls __execute_action__(echo, {message}) — returned verbatim"
  }
]
intent_examples: [
  {"input": "echo test",                                       "class": 1},
  {"input": "echo ping",                                       "class": 1},
  {"input": "test the tool pipeline",                          "class": 2},
  {"input": "diagnostic echo",                                 "class": 1},
  {"input": "verify the orchestrator can call tools",          "class": 2},
  {"input": "echo this message back",                          "class": 1},
  {"input": "test tool dispatch is working",                   "class": 2},
  {"input": "echo-ping",                                       "class": 1},
  {"input": "check the orchestrator is alive",                 "class": 2},
  {"input": "pipeline health check",                           "class": 2}
]
source: "system"
validation_status: "validated"
```

---

## Step 20 — Web Search Composition

> Web search is not a single raw tool — it is a composed capability built from
> `builtin.http` + PythonCode result extraction. There is no `builtin.web_search`
> capability ID. Instead, the `ts-web-search` ToolSkill describes the composition
> pattern, and `skill-web-search` guides the LLM on how to use it.

---

### Step 20.1 — ToolSkill: `ts-web-search` (class 13)

> Describes the composition of http + structured extraction for web search tasks.

```
name:        "ts-web-search"
tool_name:   "http"
description: "ToolSkill: web search via HTTP + structured extraction composition."
content: |
  Tool used: builtin.http (no dedicated builtin.web_search capability exists)
  Effect: Read — issues an HTTP GET to a search API endpoint, extracts results.

  Composition pattern:
  1. Use builtin.http to GET a search API endpoint (e.g. DuckDuckGo Instant Answer API,
     SerpAPI, a configured search provider endpoint).
  2. The response body is JSON. Use pc-json-extract-field (or a local PythonCode step)
     to extract the relevant results array from the response.
  3. Filter, rank, or summarize the results as needed.

  Parameter guidance:
  - url: the search API endpoint, with the query embedded as a URL param.
  - headers: include 'Accept: application/json' and any required API key header.
  - method: always GET for search.

  Constraints:
  - The agent has no built-in search engine — it must use a configured search API.
    If no search API is configured in the current scope, inform the user.
  - Respect the 15 MiB response cap from builtin.http.
  - Do not embed raw user PII in search queries without consent.
source: "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator"]
```

---

### Step 20.2 — PythonCode: `pc-web-search-extract` (class 22)

> Utility helper: extracts a usable results list from a generic search API JSON response.

```
name:        "pc-web-search-extract"
class_code:  22
description: "PythonCode helper: extract title+url+snippet list from a search API JSON response.
              Input: response body string. Output: [{title, url, snippet}] or error."
content: |
  # Pure orchestrator body — no imports. slot0 = pre-parsed response dict
  # (the http tool's JSON response is already a dict in the execution context).
  _data = "{{vars.slot0}}"
  if isinstance(_data, dict):
      _results = (
          _data.get("results") or
          _data.get("organic_results") or
          _data.get("items") or
          _data.get("RelatedTopics") or
          []
      )
      result = [
          {
              "title":   r.get("title") or r.get("Text", ""),
              "url":     r.get("url") or r.get("link") or r.get("FirstURL", ""),
              "snippet": r.get("snippet") or r.get("description") or ""
          }
          for r in _results if isinstance(r, dict)
      ]
  else:
      result = {"error": "expected parsed dict from http response", "raw": str(_data)[:500]}
consumer_tags: ["02:orchestrator"]
source:        "system"
validation_status: "validated"
```

### Step 20.3 — PythonCode: `pc-web-search-query-build` (class 22)

> Utility helper: builds a URL-encoded search query string from a natural-language query.
> Uses the same pure built-in percent-encoding logic as pc-url-encode — no imports.

```
name:        "pc-web-search-query-build"
class_code:  22
description: "PythonCode helper: URL-encode a search query for embedding in an API URL.
              No imports — uses pure built-in percent-encoding (same as pc-url-encode).
              Input: raw query string. Output: {encoded, raw}."
content: |
  # No imports — pure built-in percent-encoding (mirrors pc-url-encode logic).
  _raw = "{{vars.slot0}}".strip()
  _safe = set("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~")
  _encoded = "".join(c if c in _safe else "%" + format(ord(c), "02X") for c in _raw)
  result = {"encoded": _encoded, "raw": _raw}
consumer_tags: ["02:orchestrator"]
source:        "system"
validation_status: "validated"
```

---

### Step 20.4 — Leaf Skill: `skill-web-search` (class 1)

```
name:        "skill-web-search"
class_code:  1
description: "Leaf skill: how to perform a web search using builtin.http + JSON extraction."
body: |
  Web search is a composition, not a single tool. The pattern:
  1. Build the search URL: encode the user's query via pc-web-search-query-build and
     append it to the configured search API base URL.
  2. Issue an HTTP GET via ts-http-get (or directly via builtin.http) with
     Accept: application/json header and any required API key header.
  3. Parse and extract results from the response JSON using pc-web-search-extract.
  4. Present the top N results (title, URL, snippet) to the user. Ask if they want
     to fetch any result's full page via builtin.http for deeper reading.

  If no search API is configured, inform the user and ask them to configure one
  (endpoint URL + API key) before proceeding.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator"]
```

---

### Step 20.5 — Recipe: `web-search` (class 21)

> **Tier:** 1 — LLM formulates query, interprets results, decides follow-up fetches.

```
name:        "web-search"
description: "Search the web via a configured HTTP search API."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-web-search>"],
    "label":   "Load skill-web-search leaf skill body (composition pattern)"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-web-search-query-build>", "<uuid:pc-web-search-extract>"],
    "label":   "Load PythonCode helpers for query encoding and result extraction"
  },
  {
    "step_id": "step-3",
    "type":    "llm",
    "label":   "LLM formulates query, calls ts-http-get, extracts results, presents to user"
  },
  {
    "step_id": "step-4",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-http-fetch>"],
    "label":   "Pre-load ts-http-fetch ToolSkill binding (used for both search and follow-up fetches)"
  }
]
intent_examples: [
  {"input": "search the web for X",               "class": 1},
  {"input": "look up X online",                   "class": 1},
  {"input": "find information about X",           "class": 1},
  {"input": "what is the latest news on X",       "class": 1},
  {"input": "google X for me",                    "class": 2},
  {"input": "web search",                         "class": 1},
  {"input": "search online for this topic",       "class": 1},
  {"input": "find recent articles about X",       "class": 2},
  {"input": "internet search for X",              "class": 1},
  {"input": "find the official docs for X",       "class": 2}
]
source: "system"
validation_status: "validated"
```

---

## Step 20.x — Pure-Logic PythonCode Helpers (Gap 6: string, list, dict, CSV)

> **Gap 6:** The plan was missing general-purpose pure-logic PythonCode helpers for
> string processing, CSV parsing, and list/dict operations. These are class-22 components
> with NO I/O, NO imports, NO network. They use only built-in Python operations and
> IBS-injected slot variables. They are called from orchestrator-channel steps within
> Tier-0 and Tier-1 recipes to process tool output without an LLM.
>
> Design rule: each PythonCode helper does ONE thing. If a recipe needs two operations,
> chain two helpers — do not build a monolithic helper.

### Step 20.x.1 — PythonCode: `pc-string-split` (class 22)

```
name:        "pc-string-split"
class_code:  22
description: "Pure-logic helper: split a string by a delimiter. Input: text (string),
              delimiter (string, default newline). Output: {parts: [str], count: int}."
content: |
  # No I/O, no imports. IBS bakes in slot values before execution.
  _text      = "{{vars.slot0}}"
  _delimiter = "{{vars.slot1}}" if "{{vars.slot1}}" else "\n"
  _parts     = _text.split(_delimiter) if _text else []
  result = {"parts": _parts, "count": len(_parts)}
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 20.x.2 — PythonCode: `pc-string-join` (class 22)

```
name:        "pc-string-join"
class_code:  22
description: "Pure-logic helper: join a list of strings with a delimiter. Input: parts
              (list of strings), delimiter (string, default newline). Output: {text: str}."
content: |
  # No I/O, no imports. IBS bakes in slot values before execution.
  _parts     = {{vars.slot0}}
  _delimiter = "{{vars.slot1}}" if "{{vars.slot1}}" else "\n"
  _joined    = _delimiter.join(str(p) for p in _parts) if isinstance(_parts, list) else str(_parts)
  result = {"text": _joined}
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 20.x.3 — PythonCode: `pc-string-strip` (class 22)

```
name:        "pc-string-strip"
class_code:  22
description: "Pure-logic helper: strip whitespace (or a custom char set) from both ends
              of a string. Input: text (string), chars (optional string of chars to strip).
              Output: {text: str, changed: bool}."
content: |
  # No I/O, no imports. IBS bakes in slot values before execution.
  _text    = "{{vars.slot0}}"
  _chars   = "{{vars.slot1}}" if "{{vars.slot1}}" else None
  _stripped = _text.strip(_chars) if _chars else _text.strip()
  result = {"text": _stripped, "changed": _stripped != _text}
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 20.x.4 — PythonCode: `pc-string-replace` (class 22)

```
name:        "pc-string-replace"
class_code:  22
description: "Pure-logic helper: replace all occurrences of old_str with new_str in text.
              Input: text (string), old_str (string), new_str (string).
              Output: {text: str, count: int} — count is the number of replacements made."
content: |
  # No I/O, no imports. IBS bakes in slot values before execution.
  _text    = "{{vars.slot0}}"
  _old     = "{{vars.slot1}}"
  _new     = "{{vars.slot2}}"
  _count   = _text.count(_old)
  _result  = _text.replace(_old, _new)
  result = {"text": _result, "count": _count}
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 20.x.5 — PythonCode: `pc-string-contains` (class 22)

```
name:        "pc-string-contains"
class_code:  22
description: "Pure-logic helper: check whether text contains a substring (case-sensitive
              by default). Input: text (string), substring (string), case_insensitive
              (bool, optional). Output: {found: bool, text: str, substring: str}."
content: |
  # No I/O, no imports. IBS bakes in slot values before execution.
  _text            = "{{vars.slot0}}"
  _sub             = "{{vars.slot1}}"
  _case_insensitive = {{vars.slot2}} if "{{vars.slot2}}" not in ("", "False", "false") else False
  if _case_insensitive:
      _found = _sub.lower() in _text.lower()
  else:
      _found = _sub in _text
  result = {"found": _found, "text": _text, "substring": _sub}
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 20.x.6 — PythonCode: `pc-list-filter-nonempty` (class 22)

```
name:        "pc-list-filter-nonempty"
class_code:  22
description: "Pure-logic helper: remove empty strings and None values from a list.
              Input: items (list). Output: {items: list, removed: int, count: int}."
content: |
  # No I/O, no imports. IBS bakes in slot values before execution.
  _items  = {{vars.slot0}}
  _before = len(_items) if isinstance(_items, list) else 0
  _filtered = [x for x in _items if x is not None and x != ""] if isinstance(_items, list) else []
  result = {"items": _filtered, "removed": _before - len(_filtered), "count": len(_filtered)}
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 20.x.7 — PythonCode: `pc-list-slice` (class 22)

```
name:        "pc-list-slice"
class_code:  22
description: "Pure-logic helper: slice a list to at most max_items items starting from
              offset. Input: items (list), max_items (int), offset (int, default 0).
              Output: {items: list, total: int, offset: int, has_more: bool}."
content: |
  # No I/O, no imports. IBS bakes in slot values before execution.
  _items     = {{vars.slot0}}
  _max       = int({{vars.slot1}}) if {{vars.slot1}} else 10
  _offset    = int({{vars.slot2}}) if {{vars.slot2}} else 0
  _list      = _items if isinstance(_items, list) else []
  _sliced    = _list[_offset:_offset + _max]
  result = {"items": _sliced, "total": len(_list), "offset": _offset,
            "has_more": (_offset + _max) < len(_list)}
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 20.x.8 — PythonCode: `pc-list-unique` (class 22)

```
name:        "pc-list-unique"
class_code:  22
description: "Pure-logic helper: deduplicate a list preserving insertion order.
              Input: items (list). Output: {items: list, removed: int}."
content: |
  # No I/O, no imports. IBS bakes in slot values before execution.
  _items = {{vars.slot0}}
  _list  = _items if isinstance(_items, list) else []
  _seen  = set()
  _dedup = []
  for _item in _list:
      _key = str(_item)
      if _key not in _seen:
          _seen.add(_key)
          _dedup.append(_item)
  result = {"items": _dedup, "removed": len(_list) - len(_dedup)}
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 20.x.9 — PythonCode: `pc-dict-pick` (class 22)

```
name:        "pc-dict-pick"
class_code:  22
description: "Pure-logic helper: extract a subset of keys from a dict. Input: data (dict),
              keys (comma-separated string of key names). Output: {data: dict, missing: [str]}."
content: |
  # No I/O, no imports. IBS bakes in slot values before execution.
  _data     = {{vars.slot0}}
  _keys_str = "{{vars.slot1}}"
  _keys     = [k.strip() for k in _keys_str.split(",") if k.strip()]
  _picked   = {}
  _missing  = []
  if isinstance(_data, dict):
      for _k in _keys:
          if _k in _data:
              _picked[_k] = _data[_k]
          else:
              _missing.append(_k)
  result = {"data": _picked, "missing": _missing}
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 20.x.10 — PythonCode: `pc-dict-merge` (class 22)

```
name:        "pc-dict-merge"
class_code:  22
description: "Pure-logic helper: shallow-merge two dicts. Keys from dict_b overwrite
              dict_a on collision. Input: dict_a (dict), dict_b (dict).
              Output: {data: dict, overwritten_keys: [str]}."
content: |
  # No I/O, no imports. IBS bakes in slot values before execution.
  _a    = {{vars.slot0}}
  _b    = {{vars.slot1}}
  _a    = _a if isinstance(_a, dict) else {}
  _b    = _b if isinstance(_b, dict) else {}
  _over = [k for k in _b if k in _a]
  _merged = {**_a, **_b}
  result = {"data": _merged, "overwritten_keys": _over}
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 20.x.11 — PythonCode: `pc-csv-parse-lines` (class 22)

```
name:        "pc-csv-parse-lines"
class_code:  22
description: "Pure-logic helper: parse a CSV string into a list of row dicts. Uses the
              first row as header. Input: csv_text (string), delimiter (string, default comma).
              Output: {rows: [dict], headers: [str], count: int}."
content: |
  # No I/O, no imports needed — only builtin operations used.
  # IBS bakes in slot values before execution.
  _csv_text  = "{{vars.slot0}}"
  _delimiter = "{{vars.slot1}}" if "{{vars.slot1}}" else ","
  _lines     = [l for l in _csv_text.splitlines() if l.strip()]
  if not _lines:
      result = {"rows": [], "headers": [], "count": 0}
  else:
      _headers = [h.strip() for h in _lines[0].split(_delimiter)]
      _rows    = []
      for _line in _lines[1:]:
          _vals = [v.strip() for v in _line.split(_delimiter)]
          _row  = dict(zip(_headers, _vals))
          _rows.append(_row)
      result = {"rows": _rows, "headers": _headers, "count": len(_rows)}
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 20.x.12 — PythonCode: `pc-csv-rows-to-text` (class 22)

```
name:        "pc-csv-rows-to-text"
class_code:  22
description: "Pure-logic helper: render a list of dicts (from pc-csv-parse-lines) as a
              plain-text table. Input: rows (list of dicts), columns (optional comma-sep
              string of keys to include, default all). Output: {text: str, row_count: int}."
content: |
  # No I/O, no imports. IBS bakes in slot values before execution.
  _rows    = {{vars.slot0}}
  _cols_str = "{{vars.slot1}}"
  _rows    = _rows if isinstance(_rows, list) else []
  if not _rows:
      result = {"text": "", "row_count": 0}
  else:
      _all_keys = list(_rows[0].keys()) if _rows else []
      _cols     = [c.strip() for c in _cols_str.split(",") if c.strip()] if _cols_str else _all_keys
      _header   = " | ".join(_cols)
      _sep      = "-" * len(_header)
      _body     = "\n".join(" | ".join(str(row.get(c, "")) for c in _cols) for row in _rows)
      result    = {"text": f"{_header}\n{_sep}\n{_body}", "row_count": len(_rows)}
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

## Step 21 — Session Memory (Design Decision: No Builtin Recipe)

> **§0.23.12 Decision (preserved here):** An automatic `session-summarize` recipe that
> writes a durable session record to memory at session completion was **considered and
> rejected**. See saved_plan_to_v3.md §0.23.12 for full rationale.
>
> Summary of decision:
> - A lossy LLM-generated prose summary risks corrupting the agent's recall.
> - The trusted path is the agent deliberately writing durable notes via `memory_write`
>   as it works — the agent is best placed to judge what to persist.
> - The Kohai packet store remains forensic + self-improvement evidence, intentionally
>   separate from the memory system.
>
> **No builtin session-summarize recipe is defined.** This step exists only to record
> the decision so it is not re-proposed as a forgotten gap.
>
> If this decision is ever revisited, the agreed reversal shape is: a structured record
> (decisions, files touched, outcomes, open questions), owned by the Kohai, writing to
> memory_write, triggered on session completion, landing in Phase K. See §0.23.12 for
> the full reversal path. **This reversal is out of scope for the current plan.**

---

## Step 6.x — Additional Grep Leaf Skills, PythonCode, and Recipes

> These additions fill the grep variant gap. Each approach (case-insensitive, type-filtered)
> gets its own leaf skill and Tier-0 recipe — the orchestrator can route here directly
> without any LLM disambiguation.

### Step 6.x.1 — Leaf Skill: `skill-grep-case-insensitive` (class 1)

> Separate grain: case-insensitive search is a distinct approach and common enough to
> warrant its own leaf skill and recipe.

```
name:        "skill-grep-case-insensitive"
class_code:  1
description: "Leaf skill: how to perform a case-insensitive regex search across files."
body: |
  Use `ts-grep` with `case_insensitive=true` when the match should be case-independent
  (e.g. searching for 'error' should also match 'Error', 'ERROR'). Combine with any
  output_mode (files_with_matches, content, count). This is a distinct approach from
  the default case-sensitive search — prefer this skill when the user says 'any case',
  'case insensitive', or when the pattern contains mixed-case user input.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 6.x.2 — Leaf Skill: `skill-grep-type-filtered` (class 1)

> Separate grain: restrict grep to specific file types via glob filter.

```
name:        "skill-grep-type-filtered"
class_code:  1
description: "Leaf skill: how to restrict a grep search to specific file types using the glob filter."
body: |
  Use `ts-grep` with the `glob` parameter to restrict the search to a specific file type
  (e.g. glob='*.rs' to search only Rust files, glob='*.{ts,tsx}' for TypeScript). This
  is more precise than a workspace-root grep and avoids noise from unrelated file types.
  Combine with any output_mode. When the user specifies a file type in their search
  intent, always use the glob filter — it reduces result noise significantly.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 6.x.3 — PythonCode: `pc-exec-grep-case-insensitive` (class 22)

> Orchestrator executor for case-insensitive grep with content output mode.

```
name:        "pc-exec-grep-case-insensitive"
description: "Orchestrator executor: calls __execute_action__ for a case-insensitive grep
              via builtin.grep. Input: pattern (string), path (optional), output_mode
              (optional, default 'files_with_matches'). Sets case_insensitive=true."
content: |
  # Orchestrator executor body.
  _pattern = "{{vars.slot0}}"
  _path = "{{vars.slot1}}"
  _output_mode = "{{vars.slot2}}"
  _params = {"pattern": _pattern, "case_insensitive": True}
  if _path and _path != "":
      _params["path"] = _path
  if _output_mode and _output_mode != "":
      _params["output_mode"] = _output_mode
  else:
      _params["output_mode"] = "files_with_matches"
  result = __execute_action__("grep", _params)
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 6.x.4 — PythonCode: `pc-exec-grep-type-filtered` (class 22)

> Orchestrator executor for type-filtered grep with glob parameter.

```
name:        "pc-exec-grep-type-filtered"
description: "Orchestrator executor: calls __execute_action__ for a type-filtered grep
              via builtin.grep. Input: pattern (string), glob_filter (string e.g. '*.rs'),
              path (optional), output_mode (optional, default 'files_with_matches')."
content: |
  # Orchestrator executor body.
  _pattern = "{{vars.slot0}}"
  _glob_filter = "{{vars.slot1}}"
  _path = "{{vars.slot2}}"
  _output_mode = "{{vars.slot3}}"
  _params = {"pattern": _pattern, "glob": _glob_filter}
  if _path and _path != "":
      _params["path"] = _path
  if _output_mode and _output_mode != "":
      _params["output_mode"] = _output_mode
  else:
      _params["output_mode"] = "files_with_matches"
  result = __execute_action__("grep", _params)
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 6.x.5 — Recipe: `file-grep-case-insensitive` (class 21)

> **Tier:** 0 — case-insensitive grep variant. Routes here when the user specifies
> case-insensitive matching. One recipe per approach.

```
name:        "file-grep-case-insensitive"
description: "Search file contents case-insensitively using a regular expression."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-grep>"],
    "label":   "Pre-load ts-grep ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-grep-case-insensitive>"],
    "label":   "PythonCode calls __execute_action__(grep, {pattern, case_insensitive:true, ...})"
  }
]
intent_examples: [
  {"input": "find all uses of Error (any case)",               "class": 1},
  {"input": "case insensitive search for this pattern",        "class": 1},
  {"input": "find this word regardless of capitalisation",     "class": 1},
  {"input": "grep case insensitive",                           "class": 1},
  {"input": "search for TODO ignoring case",                   "class": 2},
  {"input": "case-insensitive regex search in the codebase",   "class": 1},
  {"input": "find 'config' in any capitalisation",             "class": 2},
  {"input": "grep -i for this pattern",                        "class": 1},
  {"input": "search files case insensitively",                 "class": 1}
]
source: "system"
validation_status: "validated"
```

### Step 6.x.6 — Recipe: `file-grep-type-filtered` (class 21)

> **Tier:** 0 — type-filtered grep variant. Routes here when the user specifies a file
> type alongside a search pattern. One recipe per approach.

```
name:        "file-grep-type-filtered"
description: "Search only specific file types for a pattern using a glob file-type filter."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-grep>"],
    "label":   "Pre-load ts-grep ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-grep-type-filtered>"],
    "label":   "PythonCode calls __execute_action__(grep, {pattern, glob:'*.ext', ...})"
  }
]
intent_examples: [
  {"input": "find this pattern only in .rs files",             "class": 1},
  {"input": "grep for this in TypeScript files only",          "class": 1},
  {"input": "search only Python files for this string",        "class": 1},
  {"input": "find this in .json config files",                 "class": 2},
  {"input": "grep only Rust source files for this pattern",    "class": 1},
  {"input": "search in .ts and .tsx files",                    "class": 2},
  {"input": "find this function only in test files",           "class": 2},
  {"input": "grep specific file extension for pattern",        "class": 1},
  {"input": "search only markdown files for this text",        "class": 2}
]
source: "system"
validation_status: "validated"
```

---

## Step 6.x.7 — Grep Invert Pattern (exclude matching lines)

> **`file-grep-invert`** is a distinct approach: find lines or files that do NOT match
> a pattern. Common use-cases: find files without a copyright header, find lines not
> matching a comment, find source files missing an import. One approach per skill/recipe.

### Step 6.x.7.1 — PythonCode: `pc-exec-grep-invert` (class 22)

```
name:        "pc-exec-grep-invert"
class_code:  22
description: "Orchestrator executor: calls __execute_action__ for an inverted grep via
              builtin.grep. Input: pattern (string), path (optional), output_mode (optional,
              default 'files_with_matches'). Sets invert_match=true."
content: |
  # Orchestrator executor body.
  _pattern = "{{vars.slot0}}"
  _path = "{{vars.slot1}}"
  _output_mode = "{{vars.slot2}}"
  _params = {"pattern": _pattern, "invert_match": True}
  if _path and _path != "":
      _params["path"] = _path
  if _output_mode and _output_mode != "":
      _params["output_mode"] = _output_mode
  else:
      _params["output_mode"] = "files_with_matches"
  result = __execute_action__("grep", _params)
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 6.x.7.2 — Leaf Skill: `skill-grep-invert` (class 1)

```
name:        "skill-grep-invert"
class_code:  1
description: "Leaf skill: how to find files or lines that do NOT match a pattern."
body: |
  Use `ts-grep` with `invert_match=true` when you need to find content that EXCLUDES a
  pattern (e.g. source files without a copyright header, lines that are not comments,
  configs missing a required key). The output returns non-matching entries. Combine with
  output_mode='files_with_matches' to get the list of files without that pattern, or
  'content' to get non-matching lines. Use pc-exec-grep-invert for execution.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 6.x.7.3 — Recipe: `file-grep-invert` (class 21)

> **Tier:** 0 — inverted grep. Routes here when the user says "files without X", "not matching",
> "missing this pattern". One recipe per approach.

```
name:        "file-grep-invert"
description: "Find files or lines that do NOT contain a given pattern (inverted grep)."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-grep>"],
    "label":   "Pre-load ts-grep ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-grep-invert>"],
    "label":   "PythonCode calls __execute_action__(grep, {pattern, invert_match:true, ...})"
  }
]
intent_examples: [
  {"input": "find files without this pattern",                   "class": 1},
  {"input": "files missing this import",                         "class": 2},
  {"input": "which files don't have a copyright header",         "class": 2},
  {"input": "invert grep — exclude matching lines",              "class": 1},
  {"input": "grep -v for this pattern",                          "class": 1},
  {"input": "show lines that do not match",                      "class": 1},
  {"input": "find files not containing this string",             "class": 1},
  {"input": "which source files lack this function",             "class": 2},
  {"input": "filter out lines matching this pattern",            "class": 2},
  {"input": "exclude files that have this keyword",              "class": 2}
]
source: "system"
validation_status: "validated"
```

---



## Step 4.x — Additional List-Dir Leaf Skills, PythonCode, and Recipes

> These additions fill the list_dir variant gap. Filtering by type (files only, dirs only)
> is a common orchestrator need — each approach gets its own leaf skill and recipe.

### Step 4.x.1 — Leaf Skill: `skill-list-dir-files-only` (class 1)

> Separate grain: list only files (no subdirectories) in a directory.

```
name:        "skill-list-dir-files-only"
class_code:  1
description: "Leaf skill: how to list only regular files (no subdirectories) in a directory."
body: |
  Use `ts-list-dir` and then filter the result with `pc-exec-list-filter-by-type` to
  return only entries of type 'file'. This is useful when you want to process every file
  in a directory without recursing, and you want to skip subdirectory entries. The
  filter is applied in the PythonCode step after the list call returns.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 4.x.2 — Leaf Skill: `skill-list-dir-dirs-only` (class 1)

> Separate grain: list only subdirectories in a directory.

```
name:        "skill-list-dir-dirs-only"
class_code:  1
description: "Leaf skill: how to list only subdirectories in a directory."
body: |
  Use `ts-list-dir` and then filter the result with `pc-exec-list-filter-by-type` to
  return only entries of type 'directory'. This is useful when exploring the top-level
  structure of a project (e.g. list only the immediate subdirectories of the repo root).
  The filter is applied in the PythonCode step after the list call returns.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 4.x.3 — PythonCode: `pc-exec-list-filter-by-type` (class 22)

> Pure logic: filters a list_dir result to a specific entry type. No I/O, no imports.

```
name:        "pc-exec-list-filter-by-type"
description: "Pure-logic helper: filters a list_dir result to only entries of a given type.
              Input: entries (list from list_dir result), entry_type ('file' | 'directory').
              Output: {entries, entry_type, count} — only matching entries."
content: |
  # No I/O, no imports. IBS bakes in entries and entry_type before execution.
  # __execute_action__ is NOT called here — this is a post-processing step.
  _entries = {{vars.slot0}}
  _entry_type = "{{vars.slot1}}"
  if not isinstance(_entries, list):
      _entries = []
  filtered = [e for e in _entries if isinstance(e, dict) and e.get("type") == _entry_type]
  result = {"entries": filtered, "entry_type": _entry_type, "count": len(filtered)}
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 4.x.4 — PythonCode: `pc-url-encode` (class 22)

> Pure logic: URL-encodes a string. No `import` statements — uses only built-in Python
> character-by-character percent-encoding. Placed near list_dir helpers for convenience;
> used by HTTP and web-search recipes.

```
name:        "pc-url-encode"
description: "Pure-logic helper: URL-encodes a string (percent-encoding, spaces as %20).
              No imports — uses pure built-in character-by-character encoding.
              Input: raw string. Output: {encoded, raw}."
content: |
  # No imports — pure built-in percent-encoding. Covers all non-unreserved chars.
  _raw = "{{vars.slot0}}".strip()
  _safe = set("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~")
  _encoded = "".join(c if c in _safe else "%" + format(ord(c), "02X") for c in _raw)
  result = {"encoded": _encoded, "raw": _raw}
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 4.x.5 — Recipe: `file-list-files-only` (class 21)

> **Tier:** 0 — list only regular files in a directory. One recipe per variant.
> Files-only listing is a distinct use case from a full directory listing.

```
name:        "file-list-files-only"
description: "List only regular files (no subdirectories) in a directory."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-list-dir>"],
    "label":   "Pre-load ts-list-dir ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-list-dir>", "<uuid:pc-exec-list-filter-by-type>"],
    "label":   "PythonCode: list_dir then filter entries to type='file'"
  }
]
intent_examples: [
  {"input": "list only the files in this directory",           "class": 1},
  {"input": "show me files without subdirectories",            "class": 1},
  {"input": "files only, no folders",                          "class": 1},
  {"input": "list all files in this directory (no dirs)",      "class": 1},
  {"input": "what files are directly in the src folder",       "class": 2},
  {"input": "show only file entries not directories",          "class": 1},
  {"input": "list files in the project root",                  "class": 2},
  {"input": "just the files please no subfolders",             "class": 1},
  {"input": "enumerate files in this folder",                  "class": 1}
]
source: "system"
validation_status: "validated"
```

---

### Step 4.x.6 — Recipe: `file-list-dirs-only` (class 21)

> **Tier:** 0 — list only subdirectories in a directory. One recipe per variant.
> Directories-only listing is a distinct use case for project exploration and navigation
> — routes here when the user says "show subdirectories" or "list folders only".

```
name:        "file-list-dirs-only"
description: "List only subdirectories (no regular files) in a directory."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-list-dir>"],
    "label":   "Pre-load ts-list-dir ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-list-dir>", "<uuid:pc-exec-list-filter-by-type>"],
    "label":   "PythonCode: list_dir then filter entries to type='directory'"
  }
]
intent_examples: [
  {"input": "list only subdirectories",                        "class": 1},
  {"input": "show me only the folders",                        "class": 1},
  {"input": "directories only, no files",                      "class": 1},
  {"input": "what subdirectories are in this folder",          "class": 1},
  {"input": "list only the immediate subdirs",                 "class": 1},
  {"input": "show folder structure without files",             "class": 2},
  {"input": "list the top-level project directories",          "class": 2},
  {"input": "just folders no files",                           "class": 1},
  {"input": "what are the child directories here",             "class": 2}
]
source: "system"
validation_status: "validated"
```

---

## Step 8.x — Additional HTTP Leaf Skills, PythonCode, and Recipes

> These additions fill the HTTP variant gap. HEAD, authenticated GET, PUT, DELETE, and
> webhook-POST are all distinct dispatch patterns deserving their own leaf skills and
> Tier-0 recipes. One approach per leaf skill; one pattern per recipe.

### Step 8.x.1 — Leaf Skill: `skill-http-head` (class 1)

> Separate grain: HEAD requests (metadata only, no response body).

```
name:        "skill-http-head"
class_code:  1
description: "Leaf skill: how to make an HTTP HEAD request to check resource metadata."
body: |
  Use `ts-http-fetch` with method='head' (via pc-exec-http-head) when you only need
  the response headers and status code — not the response body. HEAD is cheaper than
  GET for large resources and is the correct method for existence checks, content-type
  inspection, and reachability tests. The response body will be empty; inspect the
  status code (200 = exists, 404 = not found, etc.) and headers.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 8.x.2 — Leaf Skill: `skill-http-put` (class 1)

> Separate grain: PUT requests (replace a resource at a URL).

```
name:        "skill-http-put"
class_code:  1
description: "Leaf skill: how to make an HTTP PUT request to replace a resource."
body: |
  Use `ts-http-fetch` with method='put' and a `body` (via pc-exec-http-put) when the
  target API uses PUT semantics (idempotent full replacement of a resource). Include a
  Content-Type header (typically 'application/json') and an Authorization header when
  required. Non-2xx responses are not tool errors — always check the status field.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 8.x.3 — Leaf Skill: `skill-http-delete` (class 1)

> Separate grain: DELETE requests (remove a resource at a URL).

```
name:        "skill-http-delete"
class_code:  1
description: "Leaf skill: how to make an HTTP DELETE request to remove a resource."
body: |
  Use `ts-http-fetch` with method='delete' (via pc-exec-http-delete) to remove a
  resource via REST API. DELETE has ExternalWrite semantics — always confirm with the
  user before dispatching. Include Authorization headers when required. Check the
  response status (204 No Content = success for many REST APIs; 404 = already gone).
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Step 8.x.4 — PythonCode: `pc-exec-http-head` (class 22)

```
name:        "pc-exec-http-head"
description: "Orchestrator executor: calls __execute_action__ for an HTTP HEAD request via
              builtin.http. Input: url (string). Output: status code and headers only."
content: |
  # Orchestrator executor body.
  _url = "{{vars.slot0}}"
  result = __execute_action__("http", {"url": _url, "method": "head"})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 8.x.5 — PythonCode: `pc-exec-http-get-authenticated` (class 22)

```
name:        "pc-exec-http-get-authenticated"
description: "Orchestrator executor: calls __execute_action__ for an authenticated HTTP GET
              via builtin.http. Input: url (string), auth_header_value (string — full value
              for the Authorization header, e.g. 'Bearer <token>'). Output: status + body."
content: |
  # Orchestrator executor body.
  _url = "{{vars.slot0}}"
  _auth = "{{vars.slot1}}"
  _params = {"url": _url, "method": "get", "headers": {"Authorization": _auth}}
  result = __execute_action__("http", _params)
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 8.x.6 — PythonCode: `pc-exec-http-put` (class 22)

```
name:        "pc-exec-http-put"
description: "Orchestrator executor: calls __execute_action__ for an HTTP PUT request via
              builtin.http. Input: url (string), body (JSON value), headers (optional dict).
              Output: status + body."
content: |
  # Orchestrator executor body.
  _url = "{{vars.slot0}}"
  _body = {{vars.slot1}}
  _headers = {{vars.slot2}}
  _params = {"url": _url, "method": "put", "body": _body}
  if _headers:
      _params["headers"] = _headers
  result = __execute_action__("http", _params)
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 8.x.7 — PythonCode: `pc-exec-http-delete` (class 22)

```
name:        "pc-exec-http-delete"
description: "Orchestrator executor: calls __execute_action__ for an HTTP DELETE request via
              builtin.http. Input: url (string), headers (optional dict with auth).
              Output: status + body."
content: |
  # Orchestrator executor body.
  _url = "{{vars.slot0}}"
  _headers = {{vars.slot1}}
  _params = {"url": _url, "method": "delete"}
  if _headers:
      _params["headers"] = _headers
  result = __execute_action__("http", _params)
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Step 8.x.8 — Recipe: `http-head` (class 21)

> **Tier:** 0 — deterministic HEAD dispatch. One recipe per method variant.
> HEAD requests are a distinct pattern: metadata-only, no body, used for existence
> checks and reachability tests.

```
name:        "http-head"
description: "Send an HTTP HEAD request to check resource metadata (status + headers only)."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-http-fetch>"],
    "label":   "Pre-load ts-http-fetch ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-http-head>"],
    "label":   "PythonCode calls __execute_action__(http, {url, method:'head'})"
  }
]
intent_examples: [
  {"input": "check if this URL exists",                        "class": 1},
  {"input": "HEAD request to this endpoint",                   "class": 1},
  {"input": "check if this resource is reachable",             "class": 1},
  {"input": "what content type does this URL return",          "class": 2},
  {"input": "check the headers of this URL without downloading","class": 2},
  {"input": "HTTP HEAD this URL",                              "class": 1},
  {"input": "test if this API endpoint is up",                 "class": 2},
  {"input": "check resource metadata without fetching body",   "class": 1},
  {"input": "is this URL reachable",                           "class": 1}
]
source: "system"
validation_status: "validated"
```

### Step 8.x.9 — Recipe: `http-authenticated-get` (class 21)

> **Tier:** 0 — deterministic authenticated GET. One recipe per auth pattern.
> The auth token is a slot variable baked in by IBS — the orchestrator calls this
> recipe deterministically without LLM involvement.

```
name:        "http-authenticated-get"
description: "Fetch a URL via HTTP GET with a Bearer token Authorization header."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-http-fetch>"],
    "label":   "Pre-load ts-http-fetch ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-http-get-authenticated>"],
    "label":   "PythonCode calls __execute_action__(http, {url, method:get, headers:{Authorization:...}})"
  }
]
intent_examples: [
  {"input": "call this API with my bearer token",              "class": 1},
  {"input": "authenticated GET request to this endpoint",      "class": 1},
  {"input": "fetch this URL with Authorization header",        "class": 1},
  {"input": "GET this private API endpoint",                   "class": 2},
  {"input": "call this REST API using bearer auth",            "class": 2},
  {"input": "fetch the protected resource with my token",      "class": 2},
  {"input": "HTTP GET with bearer token",                      "class": 1},
  {"input": "authenticated http get",                          "class": 1},
  {"input": "call this endpoint with my API key as bearer",    "class": 2}
]
source: "system"
validation_status: "validated"
```

### Step 8.x.10 — Recipe: `http-put` (class 21)

> **Tier:** 1 — LLM must compose the PUT URL, headers, and replacement body.

```
name:        "http-put"
description: "Send an HTTP PUT request to replace a resource at a URL."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-http-put>", "<uuid:skill-http-authenticated>"],
    "label":   "Load http-put + auth leaf skill context"
  },
  {
    "step_id": "step-1",
    "type":    "llm",
    "label":   "LLM constructs the PUT URL, headers, and replacement body from user instructions"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-http-fetch>"],
    "label":   "Pre-load ts-http-fetch binding"
  }
]
intent_examples: [
  {"input": "update this resource via PUT",                    "class": 1},
  {"input": "PUT request to replace this record",             "class": 1},
  {"input": "replace this API resource via PUT",              "class": 1},
  {"input": "HTTP PUT to this endpoint",                      "class": 1},
  {"input": "send a PUT request with this body",              "class": 2},
  {"input": "update this REST resource",                      "class": 2},
  {"input": "PUT to update this configuration",               "class": 2},
  {"input": "replace the document via REST PUT",              "class": 2}
]
source: "system"
validation_status: "validated"
```

### Step 8.x.11 — Recipe: `http-delete` (class 21)

> **Tier:** 1 — LLM confirms the DELETE target and ExternalWrite effect with user.

```
name:        "http-delete"
description: "Send an HTTP DELETE request to remove a resource, with user confirmation."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-http-delete>", "<uuid:skill-http-authenticated>"],
    "label":   "Load http-delete + auth leaf skill context"
  },
  {
    "step_id": "step-1",
    "type":    "llm",
    "label":   "LLM confirms target URL with user (ExternalWrite — irreversible), then calls ts-http-fetch"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-http-fetch>"],
    "label":   "Pre-load ts-http-fetch binding"
  }
]
intent_examples: [
  {"input": "delete this resource via REST API",               "class": 1},
  {"input": "send a DELETE request to this endpoint",         "class": 1},
  {"input": "HTTP DELETE this record",                        "class": 1},
  {"input": "remove this resource via REST",                  "class": 2},
  {"input": "delete this API entry",                          "class": 2},
  {"input": "call DELETE on this endpoint",                   "class": 1},
  {"input": "destroy this resource via HTTP",                 "class": 2},
  {"input": "DELETE request to remove this item",             "class": 1}
]
source: "system"
validation_status: "validated"
```

### Step 8.x.12 — Recipe: `http-post-json-webhook` (class 21)

> **Tier:** 0 — deterministic POST with a pre-structured JSON body for webhook calls.
> A webhook POST has a known URL and a fixed-shape JSON body (event type + payload).
> The orchestrator can dispatch this deterministically when all slots are baked in.

```
name:        "http-post-json-webhook"
description: "Send a JSON webhook POST notification to a pre-configured URL."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-http-fetch>"],
    "label":   "Pre-load ts-http-fetch ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-http-post>"],
    "label":   "PythonCode calls __execute_action__(http, {url, method:post, body:{event, payload}, headers:{Content-Type:application/json}})"
  }
]
intent_examples: [
  {"input": "send a webhook notification",                     "class": 1},
  {"input": "post a JSON event to this webhook URL",           "class": 1},
  {"input": "fire a webhook with this payload",                "class": 1},
  {"input": "send a webhook alert",                            "class": 2},
  {"input": "notify the webhook endpoint",                     "class": 1},
  {"input": "trigger the webhook with a JSON body",            "class": 1},
  {"input": "post event to webhook",                           "class": 1},
  {"input": "send a hook notification to this URL",            "class": 2},
  {"input": "call the webhook endpoint with JSON",             "class": 1}
]
source: "system"
validation_status: "validated"
```

---

## Step 13.x.4 — Recipe: `memory-tree-deep` (class 21)

> **Tier:** 0 — memory tree listing with depth=5. One recipe per depth variant.
> Deep tree traversal is a distinct use case from the default depth=1 surface scan.
> Routes here when the user wants to see the full nested memory structure.

```
name:        "memory-tree-deep"
description: "List the deep directory structure of the agent's persistent memory (depth=5)."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-memory-tree>"],
    "label":   "Pre-load ts-memory-tree ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-memory-tree>"],
    "label":   "PythonCode calls __execute_action__(memory_tree, {depth:5})"
  }
]
intent_examples: [
  {"input": "show me the full memory structure",               "class": 1},
  {"input": "deep memory tree listing",                        "class": 1},
  {"input": "show all levels of memory directory",             "class": 1},
  {"input": "full nested memory tree",                         "class": 1},
  {"input": "memory tree deep",                                "class": 1},
  {"input": "show everything in my memory store",              "class": 2},
  {"input": "list the complete memory hierarchy",              "class": 2},
  {"input": "what is the full structure of my memory",         "class": 2},
  {"input": "explore all levels of memory",                    "class": 2}
]
source: "system"
validation_status: "validated"
```

---

## Step 16.x — Additional Skill-List Recipes

> Two new Tier-0 variants for scope-specific skill listing. The orchestrator can route
> directly to the right scope without asking the LLM to interpret 'user' vs 'system'.

### Step 16.x.1 — Recipe: `skill-list-user-only` (class 21)

> **Tier:** 0 — list only user-scope installed skills.

```
name:        "skill-list-user-only"
description: "List only user-installed skills (scope='user')."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-skill-list>"],
    "label":   "Pre-load ts-skill-list ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-skill-list>"],
    "label":   "PythonCode calls __execute_action__(skill_list, {scope:'user'})"
  }
]
intent_examples: [
  {"input": "what skills have I installed",                    "class": 1},
  {"input": "list my user-installed skills",                   "class": 1},
  {"input": "show only the skills I added",                    "class": 1},
  {"input": "which user skills do I have",                     "class": 1},
  {"input": "my custom skills list",                           "class": 2},
  {"input": "show skills installed by user",                   "class": 1},
  {"input": "list user-scope skills",                          "class": 1},
  {"input": "what have I installed as skills",                 "class": 2},
  {"input": "my skill library",                                "class": 2}
]
source: "system"
validation_status: "validated"
```

### Step 16.x.2 — Recipe: `skill-list-system-only` (class 21)

> **Tier:** 0 — list only system-provided built-in skills.

```
name:        "skill-list-system-only"
description: "List only system-provided built-in skills (scope='system')."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-skill-list>"],
    "label":   "Pre-load ts-skill-list ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-skill-list>"],
    "label":   "PythonCode calls __execute_action__(skill_list, {scope:'system'})"
  }
]
intent_examples: [
  {"input": "what built-in skills are available",              "class": 1},
  {"input": "list system skills",                              "class": 1},
  {"input": "show me the built-in capabilities",               "class": 1},
  {"input": "what system-level skills exist",                  "class": 1},
  {"input": "list the system builtins",                        "class": 1},
  {"input": "show only system-provided skills",                "class": 1},
  {"input": "what skills come with the system",                "class": 2},
  {"input": "list builtin skills scope system",                "class": 1},
  {"input": "show factory-installed skills",                   "class": 2}
]
source: "system"
validation_status: "validated"
```

---

## Step 14.x — Domain Skill `skill-time` (class 2)

> References all time leaf skills by name. No duplicated content. Replaces the
> individual leaf skills being referenced loosely from the management catalogue.

```
name:        "skill-time"
class_code:  2
description: "The time domain provides one tool for all time operations:

              GETTING CURRENT TIME:
              — skill-time-now: Get the current UTC timestamp (and optionally in a timezone).

              PARSING:
              — skill-time-parse: Parse a timestamp string into a structured time value.

              CONVERTING:
              — skill-time-convert: Convert a timestamp to a different timezone.

              DIFFING:
              — skill-time-diff: Compute the signed duration between two timestamps.
                Returns {seconds, minutes, hours, days}. Positive = timestamp2 is after input.

              FORMATTING:
              — skill-time-format: Render a timestamp as a human-readable string.
                Uses chrono format codes. Default: '%Y-%m-%d %H:%M:%S %Z'.

              Decision guide:
              • What time is it now → skill-time-now
              • Time in a specific timezone → skill-time-now (with timezone parameter)
              • Parse a date/time string → skill-time-parse
              • Convert between timezones → skill-time-convert
              • How long between two timestamps → skill-time-diff
              • Display a date in a human-readable style → skill-time-format

              PythonCode in the orchestrator must NEVER use datetime.now() or any date
              library directly — always call skill-time-now first to get the current time
              from the runtime clock."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

## Step 15.x — Domain Skill `skill-json` (class 2)

> References all JSON leaf skills by name. No duplicated content.

```
name:        "skill-json"
class_code:  2
description: "The JSON domain provides one tool for four JSON operations:

              EXTRACTING:
              — skill-json-query: Extract a value by dot/bracket path from a JSON structure.
              — skill-json-parse-and-query: Parse a JSON string AND immediately extract a field
                (combined two-step pattern — Tier-0, both pre-baked in vars).

              SERIALIZING:
              — skill-json-stringify: Convert a structured value to a pretty-printed JSON string.

              PARSING:
              — skill-json-parse: Parse a JSON string into a structured value.

              VALIDATING:
              — skill-json-validate: Check whether a string is valid JSON (returns {valid, error}).

              Decision guide:
              • Have a structured value, need a field → skill-json-query
              • Have a JSON string, need a specific field → skill-json-parse-and-query (Tier-0)
              • Need to write JSON to a file or display it → skill-json-stringify
              • Have a raw JSON string, need a dict/list → skill-json-parse
              • Unsure if a string is valid JSON before parsing → skill-json-validate first

              Always validate before parsing when the source is external or user-provided.
              pc-json-extract-field is an alternative pure-Python extractor for multi-hop
              path resolution when the json tool is not available in the current context."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

## Step 22.x — Per-Tool ExtensionCatalogues (class 23)

> One ExtensionCatalogue per tool/section. These sit alongside the five global domain
> catalogues and provide more precise grouping: each catalogue owns exactly one tool's
> full component stack (Tool + ToolSkill + PythonCode + Leaf Skills + Recipes).
>
> **Design principle — extension per section:**
> The global domain catalogues (builtin-filesystem, builtin-network, etc.) group all
> components of a domain. The per-tool catalogues drill down further: `ext-read-file`
> owns only the read_file components. This enables more precise recipe construction,
> narrower context injection, and finer-grained capability grants. When the orchestrator
> needs only file-reading context, it loads `ext-read-file` — not the entire filesystem
> catalogue.

### Step 22.x.1 — ExtensionCatalogue: `ext-read-file` (class 23)

```
name:         "ext-read-file"
class_code:   23
overview_doc: |
  # File Read Capability
  Tool: builtin.read_file
  Effect: read (sandboxed to workspace mount)

  Reads a file's content (full or line-range). Use for inspecting source files,
  config files, logs, or any workspace file before editing or processing it.

  Approaches:
  - Full read: path only → file-read recipe
  - Ranged read: path + range → file-read-range recipe
  - Large file: paginate using range='N-M' in iterations

task_groups:
  - group_name:  "file-read-full"
    description: "Read a complete file"
  - group_name:  "file-read-range"
    description: "Read a specific line range"

child_component_ids: [
  "<uuid:read_file>",
  "<uuid:ts-read-file>",
  "<uuid:pc-exec-read-file>",
  "<uuid:skill-read-file>",
  "<uuid:skill-read-file-range>",
  "<uuid:file-read>",
  "<uuid:file-read-range>"
]
source: "system"
validation_status: "validated"
```

### Step 22.x.2 — ExtensionCatalogue: `ext-write-file` (class 23)

```
name:         "ext-write-file"
class_code:   23
overview_doc: |
  # File Write Capability
  Tool: builtin.write_file
  Effect: write (sandboxed to workspace mount)

  Writes or replaces a file's full content. Use when creating a new file or
  intentionally replacing an entire file. For partial edits, prefer ext-apply-patch.

  Approaches:
  - New file: path + new content → file-write recipe
  - Full replace: read first, then write with new content → file-write recipe

task_groups:
  - group_name:  "file-write-new"
    description: "Create a new file"
  - group_name:  "file-write-replace"
    description: "Fully replace existing file content"

child_component_ids: [
  "<uuid:write_file>",
  "<uuid:ts-write-file>",
  "<uuid:pc-exec-write-file>",
  "<uuid:skill-write-file-new>",
  "<uuid:skill-write-file-replace>",
  "<uuid:file-write>"
]
source: "system"
validation_status: "validated"
```

### Step 22.x.3 — ExtensionCatalogue: `ext-list-dir` (class 23)

```
name:         "ext-list-dir"
class_code:   23
overview_doc: |
  # Directory Listing Capability
  Tool: builtin.list_dir
  Effect: read_filesystem (sandboxed to workspace mount)

  Lists directory contents: single level, recursive tree, or type-filtered.

  Approaches:
  - Shallow listing: path only → file-list recipe
  - Recursive scan: path + recursive:true → file-list-recursive recipe
  - Files only: list then filter → file-list-files-only recipe
  - Directories only: list then filter → file-list-dirs-only recipe

task_groups:
  - group_name:  "dir-list-shallow"
    description: "Single-level directory listing"
  - group_name:  "dir-list-recursive"
    description: "Recursive directory tree scan"
  - group_name:  "dir-list-filtered"
    description: "Type-filtered listing (files-only, dirs-only)"

child_component_ids: [
  "<uuid:list_dir>",
  "<uuid:ts-list-dir>",
  "<uuid:pc-exec-list-dir>",
  "<uuid:pc-exec-list-filter-by-type>",
  "<uuid:skill-list-dir>",
  "<uuid:skill-list-dir-recursive>",
  "<uuid:skill-list-dir-files-only>",
  "<uuid:skill-list-dir-dirs-only>",
  "<uuid:file-list>",
  "<uuid:file-list-recursive>",
  "<uuid:file-list-files-only>",
  "<uuid:file-list-dirs-only>"
]
source: "system"
validation_status: "validated"
```

### Step 22.x.4 — ExtensionCatalogue: `ext-glob` (class 23)

```
name:         "ext-glob"
class_code:   23
overview_doc: |
  # Glob File Search Capability
  Tool: builtin.glob
  Effect: read_filesystem (sandboxed to workspace mount)

  Finds files by name or extension pattern. Sorted by modification time.

  Approaches:
  - By extension: **/*.ext → file-glob-by-extension recipe
  - By name pattern: **/name* → file-glob-by-name recipe
  - In a subdirectory: path + pattern → file-glob-in-subdir recipe
  - Generic pattern: any pattern → file-glob recipe

task_groups:
  - group_name:  "glob-by-extension"
    description: "Find all files of a specific extension"
  - group_name:  "glob-by-name"
    description: "Find files matching a name pattern"
  - group_name:  "glob-in-subdir"
    description: "Restrict glob to a subdirectory"

child_component_ids: [
  "<uuid:glob>",
  "<uuid:ts-glob>",
  "<uuid:pc-exec-glob>",
  "<uuid:skill-glob-by-extension>",
  "<uuid:skill-glob-by-name>",
  "<uuid:skill-glob-in-subdir>",
  "<uuid:file-glob>",
  "<uuid:file-glob-by-extension>",
  "<uuid:file-glob-by-name>",
  "<uuid:file-glob-in-subdir>"
]
source: "system"
validation_status: "validated"
```

### Step 22.x.5 — ExtensionCatalogue: `ext-grep` (class 23)

```
name:         "ext-grep"
class_code:   23
overview_doc: |
  # Grep Content Search Capability
  Tool: builtin.grep
  Effect: read_filesystem (sandboxed to workspace mount)

  Searches file contents by regex or literal pattern. Three output modes:
  files_with_matches (fast, compact), content (matching lines + context), count (frequency).

  Approaches:
  - Which files: output_mode=files_with_matches → file-grep-files recipe
  - Matching lines: output_mode=content → file-grep-content recipe
  - Count occurrences: output_mode=count → file-grep-count recipe
  - Case-insensitive: case_insensitive=true → file-grep-case-insensitive recipe
  - Type-filtered: glob='*.ext' → file-grep-type-filtered recipe

task_groups:
  - group_name:  "grep-files"
    description: "Find which files contain a pattern"
  - group_name:  "grep-content"
    description: "Retrieve matching lines with context"
  - group_name:  "grep-count"
    description: "Count occurrences without returning content"
  - group_name:  "grep-insensitive"
    description: "Case-insensitive search"
  - group_name:  "grep-typed"
    description: "Type-filtered search"

child_component_ids: [
  "<uuid:grep>",
  "<uuid:ts-grep>",
  "<uuid:pc-exec-grep>",
  "<uuid:pc-exec-grep-case-insensitive>",
  "<uuid:pc-exec-grep-type-filtered>",
  "<uuid:skill-grep-files>",
  "<uuid:skill-grep-content>",
  "<uuid:skill-grep-count>",
  "<uuid:skill-grep-case-insensitive>",
  "<uuid:skill-grep-type-filtered>",
  "<uuid:file-grep>",
  "<uuid:file-grep-files>",
  "<uuid:file-grep-content>",
  "<uuid:file-grep-count>",
  "<uuid:file-grep-case-insensitive>",
  "<uuid:file-grep-type-filtered>"
]
source: "system"
validation_status: "validated"
```

### Step 22.x.6 — ExtensionCatalogue: `ext-apply-patch` (class 23)

```
name:         "ext-apply-patch"
class_code:   23
overview_doc: |
  # Apply Patch Capability
  Tool: builtin.apply_patch
  Effect: mixed (reads + writes the file, sandboxed to workspace mount)
  Permission: Ask (requires user confirmation in most profiles)

  Applies a targeted search-replace edit to a file. Safer than full file replacement
  because it requires exact matching of the old content.

  Approaches:
  - Single unique replacement: old_string + new_string → file-patch recipe (Tier 1)
  - Replace all occurrences: replace_all=true → file-patch-replace-all recipe (Tier 0 if exact strings are slot-provided)

  Most patch operations are Tier 1 because the LLM must read the file first and compose
  exact old/new strings. file-patch-replace-all is Tier 0 when the caller supplies both
  the old and new strings directly as recipe slots.

task_groups:
  - group_name:  "patch-single"
    description: "Replace one unique occurrence (Tier 1)"
  - group_name:  "patch-all"
    description: "Replace all occurrences of a string (Tier 0 with explicit slots)"

child_component_ids: [
  "<uuid:apply_patch>",
  "<uuid:ts-apply-patch>",
  "<uuid:pc-exec-apply-patch>",
  "<uuid:skill-apply-patch-single>",
  "<uuid:skill-apply-patch-all>",
  "<uuid:file-patch>",
  "<uuid:file-patch-replace-all>"
]
source: "system"
validation_status: "validated"
```

### Step 22.x.7 — ExtensionCatalogue: `ext-http` (class 23)

```
name:         "ext-http"
class_code:   23
overview_doc: |
  # HTTP Inline-Response Capability
  Tool: builtin.http
  Effect: network_egress
  Permission: Ask

  Issues HTTP requests (GET, POST, PUT, PATCH, DELETE, HEAD) and returns the response
  inline (body capped at 256 KiB). For larger responses use ext-http-save.

  Approaches:
  - GET a URL: → http-get recipe (Tier 0)
  - GET JSON API: → http-get-json recipe (Tier 0, Accept:application/json header)
  - GET authenticated: → http-authenticated-get recipe (Tier 0, Bearer token)
  - HEAD (metadata only): → http-head recipe (Tier 0)
  - POST JSON body: → http-post recipe (Tier 1, LLM composes body)
  - POST webhook: → http-post-json-webhook recipe (Tier 0, pre-structured body)
  - PUT (replace resource): → http-put recipe (Tier 1, LLM composes body)
  - PATCH (partial update): → http-patch recipe (Tier 1, LLM composes partial body)
  - DELETE (remove resource): → http-delete recipe (Tier 1, user confirmation required)

task_groups:
  - group_name:  "http-get"
    description: "GET requests (various auth/format variants)"
  - group_name:  "http-mutate"
    description: "POST, PUT, DELETE requests"
  - group_name:  "http-head"
    description: "HEAD requests for metadata/existence checks"

child_component_ids: [
  "<uuid:http>",
  "<uuid:ts-http-fetch>",
  "<uuid:pc-exec-http-get>",
  "<uuid:pc-exec-http-get-authenticated>",
  "<uuid:pc-exec-http-post>",
  "<uuid:pc-exec-http-head>",
  "<uuid:pc-exec-http-put>",
  "<uuid:pc-exec-http-patch>",
  "<uuid:pc-exec-http-delete>",
  "<uuid:pc-http-status-check>",
  "<uuid:pc-json-extract-field>",
  "<uuid:skill-http-get>",
  "<uuid:skill-http-post>",
  "<uuid:skill-http-authenticated>",
  "<uuid:skill-http-head>",
  "<uuid:skill-http-put>",
  "<uuid:skill-http-patch>",
  "<uuid:skill-http-delete>",
  "<uuid:http-get>",
  "<uuid:http-get-json>",
  "<uuid:http-authenticated-get>",
  "<uuid:http-head>",
  "<uuid:http-post>",
  "<uuid:http-post-json-webhook>",
  "<uuid:http-put>",
  "<uuid:http-patch>",
  "<uuid:http-delete>",
  "<uuid:skill-http>"
]
source: "system"
validation_status: "validated"
```

### Step 22.x.8 — ExtensionCatalogue: `ext-http-save` (class 23)

```
name:         "ext-http-save"
class_code:   23
overview_doc: |
  # HTTP Save-to-File Capability
  Tool: builtin.http.save
  Effect: network_egress + write_filesystem
  Permission: Ask

  Issues an HTTP request and saves the response body to a scoped workspace file.
  Use when the response exceeds 256 KiB or must be persisted for later processing.

  Approaches:
  - Download and save: url + save_to → http-save recipe (Tier 0)
  - Save large API response for parsing: url + save_to → http-save recipe (Tier 0)
  - Save with explicit large cap (5 MiB): → http-save-large recipe (Tier 0)

task_groups:
  - group_name:  "http-save-download"
    description: "Download and save to workspace file"
  - group_name:  "http-save-api"
    description: "Save large API response for later processing"

child_component_ids: [
  "<uuid:http.save>",
  "<uuid:ts-http-save>",
  "<uuid:pc-exec-http-save>",
  "<uuid:skill-http-save-download>",
  "<uuid:skill-http-save-api>",
  "<uuid:http-save>",
  "<uuid:http-save-large>"
]
source: "system"
validation_status: "validated"
```

### Step 22.x.9 — ExtensionCatalogue: `ext-memory-search` (class 23)

```
name:         "ext-memory-search"
class_code:   23
overview_doc: |
  # Memory Semantic Search Capability
  Tool: builtin.memory_search
  Effect: read_memory

  Searches the agent's persistent memory by natural-language query. Returns ranked
  documents by semantic similarity.

  Approaches:
  - Focused search (default limit=5): → memory-search recipe (Tier 0)
  - Broad recall (limit=20): → memory-search-broad recipe (Tier 0)

task_groups:
  - group_name:  "memory-search-focused"
    description: "Targeted semantic search"
  - group_name:  "memory-search-broad"
    description: "Wide recall for session start or full-topic discovery"

child_component_ids: [
  "<uuid:memory_search>",
  "<uuid:ts-memory-search>",
  "<uuid:pc-exec-memory-search>",
  "<uuid:skill-memory-search>",
  "<uuid:skill-memory-search-broad>",
  "<uuid:memory-search>",
  "<uuid:memory-search-broad>"
]
source: "system"
validation_status: "validated"
```

### Step 22.x.10 — ExtensionCatalogue: `ext-memory-write` (class 23)

```
name:         "ext-memory-write"
class_code:   23
overview_doc: |
  # Memory Write Capability
  Tool: builtin.memory_write
  Effect: write_memory

  Writes, appends, or patches the agent's persistent memory. Three targets:
  daily_log (default), MEMORY.md (main), or any relative memory path.

  Approaches:
  - Append to daily log: → memory-write-log recipe (Tier 0)
  - Append to MEMORY.md: → memory-write-main recipe (Tier 0)
  - Generic write: → memory-write recipe (Tier 0)
  - Targeted patch: → memory-write-patch recipe (Tier 0)

task_groups:
  - group_name:  "memory-write-log"
    description: "Append to daily log"
  - group_name:  "memory-write-main"
    description: "Append to MEMORY.md"
  - group_name:  "memory-write-patch"
    description: "Targeted patch of existing document"

child_component_ids: [
  "<uuid:memory_write>",
  "<uuid:ts-memory-write>",
  "<uuid:pc-exec-memory-write>",
  "<uuid:pc-exec-memory-patch>",
  "<uuid:pc-memory-format-entry>",
  "<uuid:skill-memory-write-log>",
  "<uuid:skill-memory-write-main>",
  "<uuid:skill-memory-write-patch>",
  "<uuid:memory-write>",
  "<uuid:memory-write-log>",
  "<uuid:memory-write-main>",
  "<uuid:memory-write-patch>"
]
source: "system"
validation_status: "validated"
```

### Step 22.x.11 — ExtensionCatalogue: `ext-memory-read` (class 23)

```
name:         "ext-memory-read"
class_code:   23
overview_doc: |
  # Memory Read by Path Capability
  Tool: builtin.memory_read
  Effect: read_memory

  Reads a specific memory document by its known path. Use memory_search when the
  path is unknown.

  Approaches:
  - Generic read by path: → memory-read recipe (Tier 0)
  - Read MEMORY.md: → memory-read-main recipe (Tier 0)
  - Read HEARTBEAT.md: → memory-read-heartbeat recipe (Tier 0)

task_groups:
  - group_name:  "memory-read-by-path"
    description: "Read a memory document by known path"
  - group_name:  "memory-read-wellknown"
    description: "Read well-known documents (MEMORY.md, HEARTBEAT.md)"

child_component_ids: [
  "<uuid:memory_read>",
  "<uuid:ts-memory-read>",
  "<uuid:pc-exec-memory-read>",
  "<uuid:pc-memory-extract-section>",
  "<uuid:skill-memory-read>",
  "<uuid:memory-read>",
  "<uuid:memory-read-main>",
  "<uuid:memory-read-heartbeat>"
]
source: "system"
validation_status: "validated"
```

### Step 22.x.12 — ExtensionCatalogue: `ext-memory-tree` (class 23)

```
name:         "ext-memory-tree"
class_code:   23
overview_doc: |
  # Memory Tree Directory Listing Capability
  Tool: builtin.memory_tree
  Effect: read_memory

  Lists the hierarchical directory structure of the agent's persistent memory.
  Use to discover what documents exist before searching or reading.

  Approaches:
  - Shallow tree (depth=1): → memory-tree recipe (Tier 0)
  - Deep tree (depth=5): → memory-tree-deep recipe (Tier 0)

task_groups:
  - group_name:  "memory-tree-shallow"
    description: "Root-level tree listing"
  - group_name:  "memory-tree-deep"
    description: "Deep nested tree listing"

child_component_ids: [
  "<uuid:memory_tree>",
  "<uuid:ts-memory-tree>",
  "<uuid:pc-exec-memory-tree>",
  "<uuid:skill-memory-tree>",
  "<uuid:memory-tree>",
  "<uuid:memory-tree-deep>"
]
source: "system"
validation_status: "validated"
```

### Step 22.x.13 — ExtensionCatalogue: `ext-time` (class 23)

```
name:         "ext-time"
class_code:   23
overview_doc: |
  # Time Operations Capability
  Tool: builtin.time
  Effect: read_only

  Provides five time operations via a single tool: now, parse, convert, diff, format.

  Approaches:
  - Get current time (UTC): → time-now recipe (Tier 0)
  - Get current time in a timezone: → time-now-tz recipe (Tier 0)
  - Parse a timestamp string: → time-parse recipe (Tier 0)
  - Convert between timezones: → time-convert recipe (Tier 0)
  - Compute duration between two timestamps: → time-diff recipe (Tier 0)
  - Format a timestamp as a human-readable string: → time-format recipe (Tier 0)

  PythonCode MUST NOT use datetime.now() — always use the time tool.

task_groups:
  - group_name:  "time-now"
    description: "Get current time"
  - group_name:  "time-parse"
    description: "Parse timestamp strings"
  - group_name:  "time-convert"
    description: "Timezone conversion"
  - group_name:  "time-diff"
    description: "Duration between timestamps"
  - group_name:  "time-format"
    description: "Human-readable timestamp rendering"

child_component_ids: [
  "<uuid:time>",
  "<uuid:ts-time-now>",
  "<uuid:ts-time-parse>",
  "<uuid:ts-time-convert>",
  "<uuid:ts-time-diff>",
  "<uuid:ts-time-format>",
  "<uuid:pc-exec-time-now>",
  "<uuid:pc-exec-time-parse>",
  "<uuid:pc-exec-time-convert>",
  "<uuid:pc-exec-time-diff>",
  "<uuid:pc-exec-time-format>",
  "<uuid:skill-time-now>",
  "<uuid:skill-time-parse>",
  "<uuid:skill-time-convert>",
  "<uuid:skill-time-diff>",
  "<uuid:skill-time-format>",
  "<uuid:skill-time>",
  "<uuid:time-now>",
  "<uuid:time-now-tz>",
  "<uuid:time-parse>",
  "<uuid:time-convert>",
  "<uuid:time-diff>",
  "<uuid:time-format>"
]
source: "system"
validation_status: "validated"
```

### Step 22.x.14 — ExtensionCatalogue: `ext-json` (class 23)

```
name:         "ext-json"
class_code:   23
overview_doc: |
  # JSON Operations Capability
  Tool: builtin.json
  Effect: read_only

  Provides four JSON operations via a single tool: query, stringify, parse, validate.

  Approaches:
  - Extract a field by path: → json-query recipe (Tier 0)
  - Stringify / pretty-print: → json-stringify recipe (Tier 0)
  - Parse JSON string: → json-parse recipe (Tier 0)
  - Validate JSON syntax: → json-validate recipe (Tier 0)

  Always validate before parsing when the source is external or user-provided.

task_groups:
  - group_name:  "json-query"
    description: "Extract values by path"
  - group_name:  "json-stringify-parse"
    description: "Serialize and deserialize"
  - group_name:  "json-validate"
    description: "Syntax validation"

child_component_ids: [
  "<uuid:json>",
  "<uuid:ts-json-query>",
  "<uuid:ts-json-stringify>",
  "<uuid:ts-json-validate>",
  "<uuid:pc-exec-json-query>",
  "<uuid:pc-exec-json-stringify>",
  "<uuid:pc-exec-json-validate>",
  "<uuid:skill-json-query>",
  "<uuid:skill-json-stringify>",
  "<uuid:skill-json-parse>",
  "<uuid:skill-json-validate>",
  "<uuid:skill-json>",
  "<uuid:json-query>",
  "<uuid:json-stringify>",
  "<uuid:json-parse>",
  "<uuid:json-validate>"
]
source: "system"
validation_status: "validated"
```

### Step 22.x.15 — ExtensionCatalogue: `ext-shell` (class 23)

```
name:         "ext-shell"
class_code:   23
overview_doc: |
  # Shell Execution Capability
  Tool: builtin.shell
  Effect: mixed (sandboxed subprocess)
  Permission: Ask

  TWO TIERS of shell execution in this catalogue:

  §shell-safe-fixed (Tier 0): Fixed-literal pre-validated commands.
  No user input enters the command string — zero injection surface.
  - Git inspection: → shell-git-status, shell-git-log, shell-git-diff-stat,
    shell-git-branch, shell-git-stash-list, shell-git-remote, shell-git-show-stat,
    shell-git-tag-list recipes (all Tier 0)
  - System info: → shell-pwd, shell-df, shell-ps, shell-env, shell-uname,
    shell-which, shell-date, shell-hostname, shell-whoami, shell-uptime,
    shell-free, shell-wc-l recipes (all Tier 0)

  §shell-guard-custom (Tier 1): User-composed or user-supplied commands.
  LLM must validate and compose the exact command before dispatch.
  - Single custom command: → shell-run recipe (Tier 1)
  - Multi-line script: → shell-script recipe (Tier 1)

  Prefer structured filesystem, network, and memory tools over shell whenever possible.
  Shell is the last resort when no structured tool covers the need.

task_groups:
  - group_name:  "shell-safe-fixed-git"
    description: "Fixed-literal git commands (Tier 0, no LLM)"
  - group_name:  "shell-safe-fixed-sysinfo"
    description: "Fixed-literal system info commands (Tier 0, no LLM)"
  - group_name:  "shell-custom"
    description: "User-composed shell commands (Tier 1, LLM required)"

child_component_ids: [
  "<uuid:shell>",
  "<uuid:ts-shell-run>",

  "<uuid:pc-exec-shell-git-status>",
  "<uuid:pc-exec-shell-git-log>",
  "<uuid:pc-exec-shell-git-diff-stat>",
  "<uuid:pc-exec-shell-git-branch>",
  "<uuid:pc-exec-shell-git-stash-list>",
  "<uuid:pc-exec-shell-git-log-n>",
  "<uuid:pc-exec-shell-git-remote>",
  "<uuid:pc-exec-shell-git-show-stat>",
  "<uuid:pc-exec-shell-git-tag-list>",
  "<uuid:pc-exec-shell-git-diff-name-only>",
  "<uuid:pc-exec-shell-git-log-stat>",
  "<uuid:pc-exec-shell-git-stash-show>",
  "<uuid:pc-exec-shell-git-config-list>",
  "<uuid:pc-exec-shell-pwd>",
  "<uuid:pc-exec-shell-df>",
  "<uuid:pc-exec-shell-ps>",
  "<uuid:pc-exec-shell-env>",
  "<uuid:pc-exec-shell-uname>",
  "<uuid:pc-exec-shell-which>",
  "<uuid:pc-exec-shell-date>",
  "<uuid:pc-exec-shell-hostname>",
  "<uuid:pc-exec-shell-whoami>",
  "<uuid:pc-exec-shell-uptime>",
  "<uuid:pc-exec-shell-free>",
  "<uuid:pc-exec-shell-wc-l>",

  "<uuid:skill-shell-git-status>",
  "<uuid:skill-shell-git-log>",
  "<uuid:skill-shell-git-diff-stat>",
  "<uuid:skill-shell-git-branch>",
  "<uuid:skill-shell-git-stash-list>",
  "<uuid:skill-shell-git-remote>",
  "<uuid:skill-shell-git-show-stat>",
  "<uuid:skill-shell-git-tag-list>",
  "<uuid:skill-shell-git-diff-name-only>",
  "<uuid:skill-shell-git-log-stat>",
  "<uuid:skill-shell-git-stash-show>",
  "<uuid:skill-shell-git-config-list>",
  "<uuid:skill-shell-pwd>",
  "<uuid:skill-shell-df>",
  "<uuid:skill-shell-ps>",
  "<uuid:skill-shell-env>",
  "<uuid:skill-shell-uname>",
  "<uuid:skill-shell-which>",
  "<uuid:skill-shell-date>",
  "<uuid:skill-shell-hostname>",
  "<uuid:skill-shell-whoami>",
  "<uuid:skill-shell-uptime>",
  "<uuid:skill-shell-free>",
  "<uuid:skill-shell-wc-l>",

  "<uuid:shell-git-status>",
  "<uuid:shell-git-log>",
  "<uuid:shell-git-diff-stat>",
  "<uuid:shell-git-branch>",
  "<uuid:shell-git-stash-list>",
  "<uuid:shell-git-remote>",
  "<uuid:shell-git-show-stat>",
  "<uuid:shell-git-tag-list>",
  "<uuid:shell-git-diff-name-only>",
  "<uuid:shell-git-log-stat>",
  "<uuid:shell-git-stash-show>",
  "<uuid:shell-git-config-list>",
  "<uuid:shell-git-fetch>",
  "<uuid:shell-pwd>",
  "<uuid:shell-df>",
  "<uuid:shell-ps>",
  "<uuid:shell-env>",
  "<uuid:shell-uname>",
  "<uuid:shell-which>",
  "<uuid:shell-date>",
  "<uuid:shell-hostname>",
  "<uuid:shell-whoami>",
  "<uuid:shell-uptime>",
  "<uuid:shell-free>",
  "<uuid:shell-wc-l>",

  "<uuid:skill-shell-run>",
  "<uuid:skill-shell-safe-check>",
  "<uuid:skill-shell>",
  "<uuid:shell-run>",
  "<uuid:shell-script>",

  "<uuid:pc-exec-shell-git-add>",
  "<uuid:pc-exec-shell-git-commit>",
  "<uuid:pc-exec-shell-git-push>",
  "<uuid:pc-exec-shell-git-pull>",
  "<uuid:pc-exec-shell-git-fetch>",
  "<uuid:skill-shell-git-add>",
  "<uuid:skill-shell-git-commit>",
  "<uuid:skill-shell-git-push>",
  "<uuid:skill-shell-git-pull>",
  "<uuid:skill-shell-git-fetch>",
  "<uuid:shell-git-add>",
  "<uuid:shell-git-commit>",
  "<uuid:shell-git-push>",
  "<uuid:shell-git-pull>"
]
source: "system"
validation_status: "validated"
```

### Step 22.x.16 — ExtensionCatalogue: `ext-skill-management` (class 23)

```
name:         "ext-skill-management"
class_code:   23
overview_doc: |
  # Skill Management Capability
  Tools: builtin.skill_list, builtin.skill_install, builtin.skill_remove
  Effects: Read (list), Write (install/remove)

  Manages the installed skill library. List is Tier 0. Install and Remove are Tier 1
  (user confirmation required — both have side effects on the capability stack).

  Approaches:
  - List all skills: → skill-list recipe (Tier 0)
  - List user skills only: → skill-list-user-only recipe (Tier 0)
  - List system skills only: → skill-list-system-only recipe (Tier 0)
  - Install a skill: → skill-install recipe (Tier 1)
  - Remove a skill: → skill-remove recipe (Tier 1)

task_groups:
  - group_name:  "skill-list"
    description: "Enumerate installed skills (scope-filtered)"
  - group_name:  "skill-install"
    description: "Install a new skill from URL/path"
  - group_name:  "skill-remove"
    description: "Remove an installed skill"

child_component_ids: [
  "<uuid:skill_list>",
  "<uuid:ts-skill-list>",
  "<uuid:pc-exec-skill-list>",
  "<uuid:skill-skill-list>",
  "<uuid:skill-list>",
  "<uuid:skill-list-user-only>",
  "<uuid:skill-list-system-only>",

  "<uuid:skill_install>",
  "<uuid:ts-skill-install>",
  "<uuid:skill-skill-install>",
  "<uuid:skill-install>",

  "<uuid:skill_remove>",
  "<uuid:ts-skill-remove>",
  "<uuid:skill-skill-remove>",
  "<uuid:skill-remove>",

  "<uuid:skill-skills>"
]
source: "system"
validation_status: "validated"
```

### Step 22.x.17 — ExtensionCatalogue: `ext-trigger-management` (class 23)

```
name:         "ext-trigger-management"
class_code:   23
overview_doc: |
  # Trigger Management Capability
  Tools: builtin.trigger_list, builtin.trigger_create, builtin.trigger_remove
  Effects: Read (list), ExternalWrite (create/remove)

  Manages persistent scheduled triggers. List is Tier 0. Create and Remove are Tier 1
  (ExternalWrite effect, user confirmation required).

  Approaches:
  - List all triggers: → trigger-list recipe (Tier 0)
  - Create a trigger: → trigger-create recipe (Tier 1)
  - Remove a trigger (generic): → trigger-remove recipe (Tier 1 — LLM resolves name)
  - Remove a trigger by exact name: → trigger-remove-by-name recipe (Tier 1 — LLM confirms,
    PythonCode resolves and removes — no LLM disambiguation of the name)

task_groups:
  - group_name:  "trigger-list"
    description: "Enumerate configured triggers"
  - group_name:  "trigger-create"
    description: "Schedule a new trigger"
  - group_name:  "trigger-remove"
    description: "Remove a scheduled trigger"
  - group_name:  "trigger-remove-by-name"
    description: "Remove by exact name — PythonCode does list+resolve, LLM only confirms"

child_component_ids: [
  "<uuid:trigger_list>",
  "<uuid:ts-trigger-list>",
  "<uuid:pc-exec-trigger-list>",
  "<uuid:pc-exec-trigger-list-active>",
  "<uuid:pc-exec-trigger-list-scheduled>",
  "<uuid:skill-trigger-list>",
  "<uuid:skill-trigger-list-active>",
  "<uuid:skill-trigger-list-scheduled>",
  "<uuid:trigger-list>",
  "<uuid:trigger-list-active>",
  "<uuid:trigger-list-scheduled>",

  "<uuid:trigger_create>",
  "<uuid:ts-trigger-create>",
  "<uuid:skill-trigger-create>",
  "<uuid:trigger-create>",

  "<uuid:trigger_remove>",
  "<uuid:ts-trigger-remove>",
  "<uuid:skill-trigger-remove>",
  "<uuid:trigger-remove>",

  "<uuid:pc-exec-trigger-resolve-and-remove>",
  "<uuid:trigger-remove-by-name>",

  "<uuid:skill-triggers>"
]
source: "system"
validation_status: "validated"
```

### Step 22.x.18 — ExtensionCatalogue: `ext-spawn-subagent` (class 23)

```
name:         "ext-spawn-subagent"
class_code:   23
overview_doc: |
  # Child Agent Delegation Capability
  Tool: builtin.spawn_subagent
  Effect: ExternalWrite

  §spawn_subagent-guard: ALL recipes using this tool are Tier 1 (llm_call_required=true).
  The LLM MUST frame the goal and confirm delegation. No Tier-0 spawn dispatch.

  Approaches:
  - Generic goal delegation: write clear goal + context → subagent-spawn recipe (Tier 1)
  - Named procedure: specify recipe_name → subagent-spawn recipe (Tier 1)
  - Research delegation: focused info-gathering → subagent-research recipe (Tier 1)
  - Coding delegation: file read/write/patch task → subagent-coding recipe (Tier 1)
  - Exploration delegation: deep read-only analysis → subagent-exploration recipe (Tier 1)
  - Query delegation: focused single-question lookup → subagent-query recipe (Tier 1)

  Choose the most specific recipe — the intent system routes the user's phrasing here
  and the pre-loaded leaf skill body gives the LLM the right framing before it writes
  the goal string.

task_groups:
  - group_name:  "subagent-goal"
    description: "Delegate a self-contained sub-goal to a child agent"
  - group_name:  "subagent-procedure"
    description: "Run a named recipe as a child agent procedure"
  - group_name:  "subagent-typed"
    description: "Flavour-specific delegation: research, coding, exploration, query"

child_component_ids: [
  "<uuid:spawn_subagent>",
  "<uuid:ts-spawn-subagent>",
  "<uuid:skill-spawn-subagent>",
  "<uuid:skill-spawn-named-procedure>",
  "<uuid:skill-spawn-research>",
  "<uuid:skill-spawn-coding>",
  "<uuid:skill-spawn-exploration>",
  "<uuid:skill-spawn-query>",
  "<uuid:skill-subagent>",
  "<uuid:subagent-spawn>",
  "<uuid:subagent-research>",
  "<uuid:subagent-coding>",
  "<uuid:subagent-exploration>",
  "<uuid:subagent-query>"
]
source: "system"
validation_status: "validated"
```

### Step 22.x.19 — ExtensionCatalogue: `ext-web-search` (class 23)

```
name:         "ext-web-search"
class_code:   23
overview_doc: |
  # Web Search Composition Capability
  Tool: builtin.http (composed — no dedicated web_search capability)
  Effect: network_egress (read)

  Web search is a composed capability: builtin.http GET + JSON extraction.
  A search API endpoint must be configured in the session scope first.

  Approaches:
  - Search the web: → web-search recipe (Tier 1 — LLM formulates query, interprets results)

task_groups:
  - group_name:  "web-search"
    description: "Query a configured search API and extract results"

child_component_ids: [
  "<uuid:ts-web-search>",
  "<uuid:pc-web-search-extract>",
  "<uuid:pc-web-search-query-build>",
  "<uuid:pc-url-encode>",
  "<uuid:skill-web-search>",
  "<uuid:web-search>"
]
source: "system"
validation_status: "validated"
```

---



## Step 22 — ExtensionCatalogue: `builtin-filesystem` (class 23)

> Owns all filesystem capability components. Groups Tool, ToolSkill, PythonCode,
> Skill, and Recipe components for: read_file, write_file, list_dir, glob, grep,
> apply_patch. Also covers template writes (file-write-template), patch-replace-all
> (file-patch-replace-all), and recency-sorted glob (file-glob-recent).

```
name:         "builtin-filesystem"
class_code:   23
overview_doc: |
  # Filesystem Capabilities

  The filesystem domain gives the agent structured, sandboxed access to the host
  file system. All operations are scoped to the session's working directory or an
  explicitly granted path scope — the agent cannot read or write outside its allowed
  paths.

  ## Tools in this domain
  - builtin.read_file — read a file or range of lines from a file
  - builtin.write_file — create or overwrite a file
  - builtin.list_dir — list directory contents (shallow or recursive)
  - builtin.glob — find files by name/extension pattern
  - builtin.grep — search file contents by regex or literal
  - builtin.apply_patch — make targeted edits to a file (search-and-replace)

  ## When to use which tool
  - Locate files: glob (by name/extension) or grep (by content)
  - Read content: read_file (full file or line range)
  - Create/replace: write_file
  - Targeted edits: apply_patch (preferred over read+write for partial changes)
  - Explore structure: list_dir

  ## Scope and safety
  - All paths are resolved relative to the session root. Absolute paths outside
    the granted scope will be rejected.
  - write_file and apply_patch require approval in restricted profiles.
  - apply_patch uses exact string matching by default — provide exact whitespace.

task_groups:
  - group_name:  "file-read"
    description: "Reading files: range reads, full reads, grep-then-read workflows"
  - group_name:  "file-write"
    description: "Writing and patching files: create, overwrite, targeted edit"
  - group_name:  "file-search"
    description: "Finding files and content: glob by pattern, grep by content"
  - group_name:  "file-explore"
    description: "Directory listing and workspace navigation"

child_component_ids: [
  "<uuid:builtin.read_file>",
  "<uuid:ts-read-file>",
  "<uuid:pc-exec-read-file>",
  "<uuid:skill-read-file>",
  "<uuid:skill-read-file-range>",
  "<uuid:file-read>",
  "<uuid:file-read-range>",

  "<uuid:builtin.write_file>",
  "<uuid:ts-write-file>",
  "<uuid:pc-exec-write-file>",
  "<uuid:skill-write-file-new>",
  "<uuid:skill-write-file-replace>",
  "<uuid:skill-write-file-template>",
  "<uuid:file-write>",
  "<uuid:file-write-template>",

  "<uuid:builtin.list_dir>",
  "<uuid:ts-list-dir>",
  "<uuid:pc-exec-list-dir>",
  "<uuid:pc-exec-list-filter-by-type>",
  "<uuid:skill-list-dir>",
  "<uuid:skill-list-dir-recursive>",
  "<uuid:skill-list-dir-files-only>",
  "<uuid:skill-list-dir-dirs-only>",
  "<uuid:file-list>",
  "<uuid:file-list-recursive>",
  "<uuid:file-list-files-only>",
  "<uuid:file-list-dirs-only>",

  "<uuid:builtin.glob>",
  "<uuid:ts-glob>",
  "<uuid:pc-exec-glob>",
  "<uuid:skill-glob-by-extension>",
  "<uuid:skill-glob-by-name>",
  "<uuid:skill-glob-in-subdir>",
  "<uuid:file-glob>",
  "<uuid:file-glob-by-extension>",
  "<uuid:file-glob-by-name>",
  "<uuid:file-glob-in-subdir>",
  "<uuid:file-glob-recent>",

  "<uuid:builtin.grep>",
  "<uuid:ts-grep>",
  "<uuid:pc-exec-grep>",
  "<uuid:pc-exec-grep-case-insensitive>",
  "<uuid:pc-exec-grep-type-filtered>",
  "<uuid:skill-grep-files>",
  "<uuid:skill-grep-content>",
  "<uuid:skill-grep-count>",
  "<uuid:skill-grep-case-insensitive>",
  "<uuid:skill-grep-type-filtered>",
  "<uuid:file-grep>",
  "<uuid:file-grep-files>",
  "<uuid:file-grep-content>",
  "<uuid:file-grep-count>",
  "<uuid:file-grep-case-insensitive>",
  "<uuid:file-grep-type-filtered>",

  "<uuid:builtin.apply_patch>",
  "<uuid:ts-apply-patch>",
  "<uuid:pc-exec-apply-patch>",
  "<uuid:skill-apply-patch-single>",
  "<uuid:skill-apply-patch-all>",
  "<uuid:file-patch>",
  "<uuid:file-patch-replace-all>",

  "<uuid:skill-filesystem>"
]
source: "system"
validation_status: "validated"
```

---

## Step 23 — ExtensionCatalogue: `builtin-network` (class 23)

> Owns all HTTP/network capability components. Groups Tool, ToolSkill, PythonCode,
> Skill, and Recipe components for: http, http.save, and the web search composition.

```
name:         "builtin-network"
class_code:   23
overview_doc: |
  # Network Capabilities

  The network domain gives the agent structured HTTP access to external services.
  All HTTP calls are subject to the session's outbound allowlist. Raw socket access
  is not available — only HTTP(S) via the http and http.save tools.

  ## Tools in this domain
  - builtin.http — issue an HTTP request and receive the response body inline
  - builtin.http.save — issue an HTTP request and save the response body to a file

  ## Web search (composition)
  Web search is not a separate tool — it is a composition of builtin.http + structured
  JSON extraction (pc-web-search-extract). A search API endpoint must be configured
  in the session scope before web search can be used.

  ## Constraints
  - Response body cap: 15 MiB (builtin.http); same for http.save
  - Default timeout: 10 s (connect) / 30 s (read)
  - Redirect following: up to 5 hops
  - Headers: set Accept and Content-Type explicitly for JSON APIs

  ## Scope and safety
  - Outbound URLs are validated against the session's allowed-hosts list.
  - POST requests with user-controlled bodies must be confirmed before sending.
  - API keys in headers are resolved from the secrets layer — never hardcode them
    in recipe vars or PythonCode bodies.

task_groups:
  - group_name:  "http-fetch"
    description: "GET and POST requests with inline response body"
  - group_name:  "http-download"
    description: "Requests that save the response body to a file"
  - group_name:  "web-search"
    description: "Search API composition (http + JSON extraction)"

child_component_ids: [
  "<uuid:builtin.http>",
  "<uuid:ts-http-fetch>",
  "<uuid:pc-exec-http-get>",
  "<uuid:pc-exec-http-get-authenticated>",
  "<uuid:pc-exec-http-post>",
  "<uuid:pc-exec-http-head>",
  "<uuid:pc-exec-http-put>",
  "<uuid:pc-exec-http-patch>",
  "<uuid:pc-exec-http-delete>",
  "<uuid:pc-http-status-check>",
  "<uuid:pc-json-extract-field>",
  "<uuid:skill-http-get>",
  "<uuid:skill-http-post>",
  "<uuid:skill-http-authenticated>",
  "<uuid:skill-http-head>",
  "<uuid:skill-http-put>",
  "<uuid:skill-http-patch>",
  "<uuid:skill-http-delete>",
  "<uuid:http-get>",
  "<uuid:http-get-json>",
  "<uuid:http-authenticated-get>",
  "<uuid:http-head>",
  "<uuid:http-post>",
  "<uuid:http-post-json-webhook>",
  "<uuid:http-put>",
  "<uuid:http-patch>",
  "<uuid:http-delete>",

  "<uuid:builtin.http.save>",
  "<uuid:ts-http-save>",
  "<uuid:pc-exec-http-save>",
  "<uuid:skill-http-save-download>",
  "<uuid:skill-http-save-api>",
  "<uuid:http-save>",
  "<uuid:http-save-large>",

  "<uuid:skill-http>",

  "<uuid:ts-web-search>",
  "<uuid:pc-web-search-extract>",
  "<uuid:pc-web-search-query-build>",
  "<uuid:pc-url-encode>",
  "<uuid:skill-web-search>",
  "<uuid:web-search>"
]
source: "system"
validation_status: "validated"
```

---

## Step 24 — ExtensionCatalogue: `builtin-memory` (class 23)

> Owns all persistent memory capability components. Groups Tool, ToolSkill,
> PythonCode, Skill, and Recipe components for: memory_search, memory_write,
> memory_read, memory_tree.

```
name:         "builtin-memory"
class_code:   23
overview_doc: |
  # Memory Capabilities

  The memory domain gives the agent a persistent, structured workspace for storing
  and retrieving durable information across sessions. Memory is organized as a
  hierarchical key-space (path-based) with semantic search support.

  ## Tools in this domain
  - builtin.memory_search — semantic or keyword search across stored entries
  - builtin.memory_write  — create or update a memory entry at a path
  - builtin.memory_read   — retrieve a specific entry by exact path
  - builtin.memory_tree   — list the memory path hierarchy (directory-tree style)

  ## Navigation pattern
  Before searching or reading, run memory_tree to understand the structure. Then use
  memory_search for broad queries and memory_read for specific known paths.

  ## Write-back discipline
  The agent should write durable notes, decisions, and outcomes to memory as it works.
  This is the ONLY mechanism for cross-session persistence — there is no automatic
  session summarization. What the agent does not explicitly write is not remembered.

  ## Scope
  Each session scope has an isolated memory namespace. Cross-scope reads are not
  permitted unless explicitly granted.

task_groups:
  - group_name:  "memory-recall"
    description: "Searching and reading stored memory entries"
  - group_name:  "memory-persist"
    description: "Writing and updating memory entries"
  - group_name:  "memory-navigate"
    description: "Exploring the memory hierarchy via tree"

child_component_ids: [
  "<uuid:builtin.memory_search>",
  "<uuid:ts-memory-search>",
  "<uuid:pc-exec-memory-search>",
  "<uuid:skill-memory-search>",
  "<uuid:skill-memory-search-broad>",
  "<uuid:memory-search>",
  "<uuid:memory-search-broad>",

  "<uuid:builtin.memory_write>",
  "<uuid:ts-memory-write>",
  "<uuid:pc-exec-memory-write>",
  "<uuid:pc-exec-memory-patch>",
  "<uuid:pc-memory-format-entry>",
  "<uuid:skill-memory-write-log>",
  "<uuid:skill-memory-write-main>",
  "<uuid:skill-memory-write-patch>",
  "<uuid:memory-write>",
  "<uuid:memory-write-log>",
  "<uuid:memory-write-main>",
  "<uuid:memory-write-patch>",

  "<uuid:builtin.memory_read>",
  "<uuid:ts-memory-read>",
  "<uuid:pc-exec-memory-read>",
  "<uuid:pc-memory-extract-section>",
  "<uuid:skill-memory-read>",
  "<uuid:memory-read>",
  "<uuid:memory-read-main>",
  "<uuid:memory-read-heartbeat>",

  "<uuid:builtin.memory_tree>",
  "<uuid:ts-memory-tree>",
  "<uuid:pc-exec-memory-tree>",
  "<uuid:skill-memory-tree>",
  "<uuid:memory-tree>",
  "<uuid:memory-tree-deep>",

  "<uuid:skill-memory>"
]
source: "system"
validation_status: "validated"
```

---

## Step 25 — ExtensionCatalogue: `builtin-process` (class 23)

> Owns shell execution, spawn_subagent, and all trigger management components.
> Shell has TWO tiers: §shell-safe-fixed (Tier 0, fixed commands) and
> §shell-guard-custom (Tier 1, user-composed). `trigger-list` is Tier 0.
> `spawn_subagent` is always Tier 1 (§spawn_subagent-guard).

```
name:         "builtin-process"
class_code:   23
overview_doc: |
  # Process & Scheduling Capabilities

  The process domain covers: shell command execution (two tiers), child agent delegation,
  and persistent trigger scheduling.

  ## Tools in this domain
  - builtin.shell          — run a shell command in a sandboxed subprocess
  - builtin.spawn_subagent — delegate a sub-goal to a child agent run
  - builtin.trigger_create — create a scheduled or event-driven trigger
  - builtin.trigger_list   — list configured triggers (read-only)
  - builtin.trigger_remove — remove a trigger (irreversible)

  ## Shell safety — two tiers
  §shell-safe-fixed (Tier 0): Fixed-literal pre-validated commands.
  No user input enters the command string — the PythonCode hardcodes the command.
  Git inspection (git status, log, diff --stat, branch) and system info
  (pwd, df -h, ps aux, env, uname -a, which) are all Tier 0.

  §shell-guard-custom (Tier 1): User-composed or user-supplied commands.
  The LLM must compose and validate the exact command before dispatch.
  Never pass unvalidated user input into a custom shell command.

  ## Subagent invariants (§spawn_subagent-guard)
  - Any recipe using builtin.spawn_subagent MUST have llm_call_required=true. No Tier-0.
  - Child cannot exceed parent scope or authority.
  - Include all needed context explicitly — child has no parent conversation access.

  ## Trigger safety
  - trigger_create and trigger_remove have ExternalWrite effect — require user confirmation.
  - Triggers run with the creating session's authority and cannot escalate.

task_groups:
  - group_name:  "shell-safe-fixed"
    description: "Fixed-literal shell commands (Tier 0): git + system info"
  - group_name:  "shell-custom"
    description: "User-composed shell execution (Tier 1, LLM required)"
  - group_name:  "agent-delegation"
    description: "Child agent spawning and sub-task delegation"
  - group_name:  "trigger-management"
    description: "Scheduled trigger lifecycle: list, create, remove"

child_component_ids: [
  "<uuid:builtin.shell>",
  "<uuid:ts-shell-run>",
  "<uuid:pc-exec-shell-git-status>",
  "<uuid:pc-exec-shell-git-log>",
  "<uuid:pc-exec-shell-git-diff-stat>",
  "<uuid:pc-exec-shell-git-branch>",
  "<uuid:pc-exec-shell-git-stash-list>",
  "<uuid:pc-exec-shell-git-log-n>",
  "<uuid:pc-exec-shell-git-remote>",
  "<uuid:pc-exec-shell-git-show-stat>",
  "<uuid:pc-exec-shell-git-tag-list>",
  "<uuid:pc-exec-shell-pwd>",
  "<uuid:pc-exec-shell-df>",
  "<uuid:pc-exec-shell-ps>",
  "<uuid:pc-exec-shell-env>",
  "<uuid:pc-exec-shell-uname>",
  "<uuid:pc-exec-shell-which>",
  "<uuid:pc-exec-shell-date>",
  "<uuid:pc-exec-shell-hostname>",
  "<uuid:pc-exec-shell-whoami>",
  "<uuid:pc-exec-shell-uptime>",
  "<uuid:pc-exec-shell-free>",
  "<uuid:pc-exec-shell-wc-l>",
  "<uuid:skill-shell-git-status>",
  "<uuid:skill-shell-git-log>",
  "<uuid:skill-shell-git-diff-stat>",
  "<uuid:skill-shell-git-branch>",
  "<uuid:skill-shell-git-stash-list>",
  "<uuid:skill-shell-git-remote>",
  "<uuid:skill-shell-git-show-stat>",
  "<uuid:skill-shell-git-tag-list>",
  "<uuid:skill-shell-pwd>",
  "<uuid:skill-shell-df>",
  "<uuid:skill-shell-ps>",
  "<uuid:skill-shell-env>",
  "<uuid:skill-shell-uname>",
  "<uuid:skill-shell-which>",
  "<uuid:skill-shell-date>",
  "<uuid:skill-shell-hostname>",
  "<uuid:skill-shell-whoami>",
  "<uuid:skill-shell-uptime>",
  "<uuid:skill-shell-free>",
  "<uuid:skill-shell-wc-l>",
  "<uuid:shell-git-status>",
  "<uuid:shell-git-log>",
  "<uuid:shell-git-diff-stat>",
  "<uuid:shell-git-branch>",
  "<uuid:shell-git-stash-list>",
  "<uuid:shell-git-remote>",
  "<uuid:shell-git-show-stat>",
  "<uuid:shell-git-tag-list>",
  "<uuid:shell-pwd>",
  "<uuid:shell-df>",
  "<uuid:shell-ps>",
  "<uuid:shell-env>",
  "<uuid:shell-uname>",
  "<uuid:shell-which>",
  "<uuid:shell-date>",
  "<uuid:shell-hostname>",
  "<uuid:shell-whoami>",
  "<uuid:shell-uptime>",
  "<uuid:shell-free>",
  "<uuid:shell-wc-l>",
  "<uuid:skill-shell-run>",
  "<uuid:skill-shell-safe-check>",
  "<uuid:skill-shell>",
  "<uuid:shell-run>",
  "<uuid:shell-script>",

  "<uuid:pc-exec-shell-git-add>",
  "<uuid:pc-exec-shell-git-commit>",
  "<uuid:pc-exec-shell-git-push>",
  "<uuid:pc-exec-shell-git-pull>",
  "<uuid:pc-exec-shell-git-fetch>",
  "<uuid:skill-shell-git-add>",
  "<uuid:skill-shell-git-commit>",
  "<uuid:skill-shell-git-push>",
  "<uuid:skill-shell-git-pull>",
  "<uuid:skill-shell-git-fetch>",
  "<uuid:shell-git-add>",
  "<uuid:shell-git-commit>",
  "<uuid:shell-git-push>",
  "<uuid:shell-git-pull>",

  "<uuid:builtin.spawn_subagent>",
  "<uuid:ts-spawn-subagent>",
  "<uuid:skill-spawn-subagent>",
  "<uuid:skill-spawn-named-procedure>",
  "<uuid:skill-spawn-research>",
  "<uuid:skill-spawn-coding>",
  "<uuid:skill-spawn-exploration>",
  "<uuid:skill-spawn-query>",
  "<uuid:skill-subagent>",
  "<uuid:subagent-spawn>",
  "<uuid:subagent-research>",
  "<uuid:subagent-coding>",
  "<uuid:subagent-exploration>",
  "<uuid:subagent-query>",

  "<uuid:builtin.trigger_create>",
  "<uuid:ts-trigger-create>",
  "<uuid:skill-trigger-create>",
  "<uuid:trigger-create>",

  "<uuid:builtin.trigger_list>",
  "<uuid:ts-trigger-list>",
  "<uuid:pc-exec-trigger-list>",
  "<uuid:pc-exec-trigger-list-active>",
  "<uuid:pc-exec-trigger-list-scheduled>",
  "<uuid:skill-trigger-list>",
  "<uuid:skill-trigger-list-active>",
  "<uuid:skill-trigger-list-scheduled>",
  "<uuid:trigger-list>",
  "<uuid:trigger-list-active>",
  "<uuid:trigger-list-scheduled>",

  "<uuid:builtin.trigger_remove>",
  "<uuid:ts-trigger-remove>",
  "<uuid:skill-trigger-remove>",
  "<uuid:trigger-remove>",

  "<uuid:skill-triggers>"
]
source: "system"
validation_status: "validated"
```

---

## Step 26 — ExtensionCatalogue: `builtin-management` (class 23)

> Owns skill management, echo (diagnostic), time, and JSON components. This is the
> "utilities" domain — tools that manage the agent's own capability state (skills),
> plus the time and JSON utility toolchains.

```
name:         "builtin-management"
class_code:   23
overview_doc: |
  # Management & Utility Capabilities

  The management domain covers: skill lifecycle management, time operations, JSON
  manipulation, and the diagnostic echo passthrough.

  ## Tools in this domain
  - builtin.skill_list    — list installed skills
  - builtin.skill_install — install a skill from URL/path (enters Q1/Q2)
  - builtin.skill_remove  — remove an installed skill (irreversible)
  - builtin.time          — time queries: now, parse, convert
  - builtin.json          — JSON operations: query, stringify, parse, validate
  - builtin.echo          — diagnostic passthrough (no user-facing recipe)

  ## Skill management
  - Always list before installing (avoid duplicates).
  - Always confirm with user before installing from external URLs or removing.
  - After install, the skill is 'pending' — not usable until Q2 graduates it.
  - System-scope skills cannot be modified from user-scope authority.

  ## Time utilities
  - time/now: current UTC and local time in ISO 8601
  - time/parse: parse a datetime string into components
  - time/convert: convert between timezones or formats

  ## JSON utilities
  - json/query: extract a value from a JSON structure via jq-style path
  - json/stringify: serialize a value to a JSON string (pretty-printed)
  - json/parse: parse a JSON string to a structured value
  - json/validate: check whether a string is valid JSON

  ## Echo
  Echo is a diagnostic-only passthrough. It has no user-facing recipe. Use it only
  in tests and during recipe development.

task_groups:
  - group_name:  "skill-management"
    description: "Skill lifecycle: list, install, remove"
  - group_name:  "time-utilities"
    description: "Time queries, parsing, and conversion"
  - group_name:  "json-utilities"
    description: "JSON query, stringify, parse, validate"
  - group_name:  "diagnostics"
    description: "Echo passthrough (development/testing only)"

child_component_ids: [
  "<uuid:builtin.skill_list>",
  "<uuid:ts-skill-list>",
  "<uuid:pc-exec-skill-list>",
  "<uuid:skill-skill-list>",
  "<uuid:skill-list>",
  "<uuid:skill-list-user-only>",
  "<uuid:skill-list-system-only>",

  "<uuid:builtin.skill_install>",
  "<uuid:ts-skill-install>",
  "<uuid:skill-skill-install>",
  "<uuid:skill-install>",

  "<uuid:builtin.skill_remove>",
  "<uuid:ts-skill-remove>",
  "<uuid:skill-skill-remove>",
  "<uuid:skill-remove>",

  "<uuid:skill-skills>",

  "<uuid:builtin.time>",
  "<uuid:ts-time-now>",
  "<uuid:ts-time-parse>",
  "<uuid:ts-time-convert>",
  "<uuid:ts-time-diff>",
  "<uuid:ts-time-format>",
  "<uuid:pc-exec-time-now>",
  "<uuid:pc-exec-time-parse>",
  "<uuid:pc-exec-time-convert>",
  "<uuid:pc-exec-time-diff>",
  "<uuid:pc-exec-time-format>",
  "<uuid:skill-time-now>",
  "<uuid:skill-time-parse>",
  "<uuid:skill-time-convert>",
  "<uuid:skill-time-diff>",
  "<uuid:skill-time-format>",
  "<uuid:skill-time>",
  "<uuid:time-now>",
  "<uuid:time-now-tz>",
  "<uuid:time-parse>",
  "<uuid:time-convert>",
  "<uuid:time-diff>",
  "<uuid:time-format>",

  "<uuid:builtin.json>",
  "<uuid:ts-json-query>",
  "<uuid:ts-json-stringify>",
  "<uuid:ts-json-validate>",
  "<uuid:pc-exec-json-query>",
  "<uuid:pc-exec-json-stringify>",
  "<uuid:pc-exec-json-validate>",
  "<uuid:skill-json-query>",
  "<uuid:skill-json-stringify>",
  "<uuid:skill-json-parse>",
  "<uuid:skill-json-validate>",
  "<uuid:skill-json>",
  "<uuid:json-query>",
  "<uuid:json-stringify>",
  "<uuid:json-parse>",
  "<uuid:json-validate>",

  "<uuid:builtin.echo>",
  "<uuid:ts-echo>",
  "<uuid:pc-exec-echo>",
  "<uuid:echo-ping>"
]
source: "system"
validation_status: "validated"
```

---

## Step 1.x.18 — Git Write Operations (§shell-guard-custom, Tier 1)

> Git write operations (add, commit, push, pull, fetch) involve user-supplied file paths,
> commit messages, remote names, branch names, or credentials. They are **always Tier 1**
> (§shell-guard-custom unless noted). The LLM composes the exact command string, validates
> safety, and gets user approval. These operations are destructive or remote-modifying —
> the LLM MUST be in the loop.

### PythonCode: `pc-exec-shell-git-add` (class 22)

> Tier 1 helper — file path(s) supplied by LLM/user.
> §shell-guard-custom: command contains user-supplied paths → Tier 1 only.

```
name:        "pc-exec-shell-git-add"
description: "Orchestrator executor: calls __execute_action__ to run 'git add <path>'.
              Input: vars.slot0 = path(s) to stage. Use '.' to stage all changes.
              §shell-guard-custom — path is user/LLM-supplied. Tier 1 only."
content: |
  # §shell-guard-custom — path is user/LLM-supplied. Tier 1 only.
  _path = "{{vars.slot0}}" or "."
  result = __execute_action__("shell", {"command": "git add " + _path})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Leaf Skill: `skill-shell-git-add` (class 1)

```
name:        "skill-shell-git-add"
class_code:  1
description: "Leaf skill: how to stage files for commit with git add."
body: |
  Use pc-exec-shell-git-add (via shell tool) to stage files.
  §shell-guard-custom applies: always Tier 1.
  Always run 'git status' (skill-shell-git-status) first to know which files are modified.
  Pass '.' to stage all changes, or provide specific file paths. After staging, run
  'git status' again to confirm the staged content before committing.
  Never stage files the user has not confirmed — particularly .env, secrets, and
  binary files should be explicitly confirmed before staging.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Tier-1 Recipe: `shell-git-add` (class 21)

> **Tier:** 1 — §shell-guard-custom. File paths are user-supplied.
> Routes here for "git add", "stage these files", "add changes to git".

```
name:        "shell-git-add"
description: "Stage files for commit with git add (LLM confirms paths with user)."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-shell-git-add>", "<uuid:skill-shell-git-status>"],
    "label":   "Load git-add + git-status leaf skills"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run binding"
  },
  {
    "step_id": "step-3",
    "type":    "llm",
    "label":   "LLM checks git status, confirms which files to stage, dispatches git add"
  }
]
intent_examples: [
  {"input": "git add",                                "class": 1},
  {"input": "stage my changes",                       "class": 1},
  {"input": "add all files to git",                   "class": 1},
  {"input": "stage this file for commit",             "class": 2},
  {"input": "git add .",                              "class": 1},
  {"input": "add these changes to git staging",       "class": 2},
  {"input": "stage my modifications",                 "class": 2},
  {"input": "mark these files for the next commit",   "class": 2},
  {"input": "git add specific file",                  "class": 2},
  {"input": "track and stage these changes",          "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### PythonCode: `pc-exec-shell-git-commit` (class 22)

> Tier 1 helper — the LLM supplies the commit message via vars.slot0.
> §shell-guard-custom: command contains user-supplied content → Tier 1 only.

```
name:        "pc-exec-shell-git-commit"
description: "Orchestrator executor: calls __execute_action__ to run 'git commit -m <msg>'.
              Input: vars.slot0 = commit message (user-supplied, LLM-validated). Tier 1 only."
content: |
  # §shell-guard-custom — commit message is user/LLM-supplied. Tier 1 only.
  _msg = "{{vars.slot0}}"
  result = __execute_action__("shell", {"command": "git commit -m " + repr(_msg)})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### PythonCode: `pc-exec-shell-git-push` (class 22)

> Tier 1 helper — remote and branch are user-supplied.

```
name:        "pc-exec-shell-git-push"
description: "Orchestrator executor: calls __execute_action__ to run 'git push <remote> <branch>'.
              Input: vars.slot0 = remote (e.g. 'origin'), vars.slot1 = branch (e.g. 'main'). Tier 1 only."
content: |
  # §shell-guard-custom — remote/branch are user-supplied. Tier 1 only.
  _remote = "{{vars.slot0}}" or "origin"
  _branch = "{{vars.slot1}}" or "main"
  result = __execute_action__("shell", {"command": "git push " + _remote + " " + _branch})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### PythonCode: `pc-exec-shell-git-pull` (class 22)

```
name:        "pc-exec-shell-git-pull"
description: "Orchestrator executor: calls __execute_action__ to run 'git pull <remote> <branch>'.
              Input: vars.slot0 = remote, vars.slot1 = branch. Tier 1 only."
content: |
  # §shell-guard-custom — remote/branch are user-supplied. Tier 1 only.
  _remote = "{{vars.slot0}}" or "origin"
  _branch = "{{vars.slot1}}" or ""
  _cmd = "git pull " + _remote
  if _branch:
      _cmd = _cmd + " " + _branch
  result = __execute_action__("shell", {"command": _cmd})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### PythonCode: `pc-exec-shell-git-fetch` (class 22)

> `git fetch --all` with no branch slot is §shell-safe-fixed (Tier 0 eligible).
> `git fetch <remote>` with a user-supplied remote is Tier 1.

```
name:        "pc-exec-shell-git-fetch"
description: "Orchestrator executor: calls __execute_action__ to run 'git fetch --all'. Tier 0 safe."
content: |
  # §shell-safe-fixed — 'git fetch --all' is a fixed read-only remote query. No user input in command.
  result = __execute_action__("shell", {"command": "git fetch --all"})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Leaf Skills: Git Write Operations (class 1)

```
name:        "skill-shell-git-commit"
class_code:  1
description: "Leaf skill: how to commit staged changes with a message."
body: |
  Use pc-exec-shell-git-commit (via shell tool) to commit staged changes.
  §shell-guard-custom applies: always Tier 1. The LLM composes the commit message,
  confirms it with the user, then dispatches. Never commit without an explicit message.
  Always run 'git status' (skill-shell-git-status) first to verify what is staged.
  If nothing is staged, inform the user — do NOT run 'git add' without explicit instruction.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

```
name:        "skill-shell-git-push"
class_code:  1
description: "Leaf skill: how to push commits to a remote repository."
body: |
  Use pc-exec-shell-git-push (via shell tool) to push commits to a remote.
  §shell-guard-custom applies: always Tier 1. Default remote is 'origin', default branch is
  the current branch. Always run 'git log --oneline -5' first to confirm what is being pushed.
  Warn the user if pushing to main/master directly without a PR — this is a force-push risk.
  Never push to a remote the user has not confirmed.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

```
name:        "skill-shell-git-pull"
class_code:  1
description: "Leaf skill: how to pull changes from a remote repository."
body: |
  Use pc-exec-shell-git-pull (via shell tool) to pull remote changes.
  §shell-guard-custom applies: always Tier 1. Default remote is 'origin'.
  Warn the user if there are uncommitted local changes that could conflict (check via
  skill-shell-git-status first). If the pull results in conflicts, surface the conflict
  output to the user and do not attempt auto-resolution without explicit instruction.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

```
name:        "skill-shell-git-fetch"
class_code:  1
description: "Leaf skill: how to fetch all remote branches without merging."
body: |
  Use pc-exec-shell-git-fetch (via shell tool) to fetch all remotes.
  §shell-safe-fixed: 'git fetch --all' has no user input → Tier 0 eligible.
  Fetch is non-destructive (read-only local ref update). Use it before 'git status' to get
  an up-to-date view of remote branches and compare with local state.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Tier-0 Recipe: `shell-git-fetch` (class 21)

> **Tier:** 0 — §shell-safe-fixed. 'git fetch --all' has no user input in the command string.
> Tier 0 is safe: the command is a fixed literal, no injection surface.

```
name:        "shell-git-fetch"
description: "Fetch all remote branches without merging (git fetch --all). Tier 0 — read-only."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-git-fetch>"],
    "label":   "PythonCode calls __execute_action__(shell, {command:'git fetch --all'})"
  }
]
intent_examples: [
  {"input": "git fetch",                               "class": 1},
  {"input": "fetch all remote branches",               "class": 1},
  {"input": "git fetch all",                           "class": 1},
  {"input": "update my remote tracking branches",      "class": 2},
  {"input": "fetch from origin",                       "class": 2},
  {"input": "get latest remote refs",                  "class": 2},
  {"input": "fetch without merging",                   "class": 1},
  {"input": "update remote branch info",               "class": 2},
  {"input": "git fetch --all",                         "class": 1},
  {"input": "pull down remote branch list",            "class": 2}
]
source: "system"
validation_status: "validated"
```

### Tier-1 Recipe: `shell-git-commit` (class 21)

> **Tier:** 1 — §shell-guard-custom. Commit message is user/LLM-supplied.

```
name:        "shell-git-commit"
description: "Commit staged changes with a user-confirmed message."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-shell-git-commit>", "<uuid:skill-shell-git-status>"],
    "label":   "Load git-commit + git-status leaf skills"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run binding"
  },
  {
    "step_id": "step-3",
    "type":    "llm",
    "label":   "LLM checks git status, composes commit message, confirms with user, dispatches commit"
  }
]
intent_examples: [
  {"input": "git commit",                              "class": 1},
  {"input": "commit my changes",                       "class": 1},
  {"input": "commit staged files",                     "class": 1},
  {"input": "commit with message",                     "class": 2},
  {"input": "save my changes with a commit",           "class": 2},
  {"input": "create a git commit",                     "class": 2},
  {"input": "commit everything I staged",              "class": 2},
  {"input": "make a commit with this message",         "class": 2},
  {"input": "git commit -m",                           "class": 1},
  {"input": "finalize my changes with a git commit",   "class": 3}
]
source: "system"
validation_status: "validated"
```

### Tier-1 Recipe: `shell-git-push` (class 21)

> **Tier:** 1 — §shell-guard-custom. Remote and branch involve user intent.

```
name:        "shell-git-push"
description: "Push local commits to a remote repository branch."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-shell-git-push>", "<uuid:skill-shell-git-log>"],
    "label":   "Load git-push + git-log leaf skills"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run binding"
  },
  {
    "step_id": "step-3",
    "type":    "llm",
    "label":   "LLM shows recent commits, confirms remote/branch, dispatches push"
  }
]
intent_examples: [
  {"input": "git push",                                "class": 1},
  {"input": "push my commits",                         "class": 1},
  {"input": "push to origin",                          "class": 1},
  {"input": "push to main",                            "class": 2},
  {"input": "upload my changes to github",             "class": 2},
  {"input": "push local branch to remote",             "class": 2},
  {"input": "git push origin main",                    "class": 1},
  {"input": "send my commits to the remote",           "class": 2},
  {"input": "push my work to the repository",          "class": 2},
  {"input": "deploy my commits to origin",             "class": 2}
]
source: "system"
validation_status: "validated"
```

### Tier-1 Recipe: `shell-git-pull` (class 21)

> **Tier:** 1 — §shell-guard-custom. Remote/branch involve user intent; merge conflicts require LLM.

```
name:        "shell-git-pull"
description: "Pull remote changes and merge into the current branch."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-shell-git-pull>", "<uuid:skill-shell-git-status>"],
    "label":   "Load git-pull + git-status leaf skills"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run binding"
  },
  {
    "step_id": "step-3",
    "type":    "llm",
    "label":   "LLM checks for local changes, confirms remote/branch, handles conflicts on failure"
  }
]
intent_examples: [
  {"input": "git pull",                                "class": 1},
  {"input": "pull latest changes",                     "class": 1},
  {"input": "update from remote",                      "class": 1},
  {"input": "pull from origin",                        "class": 2},
  {"input": "get latest code from github",             "class": 2},
  {"input": "sync with remote branch",                 "class": 2},
  {"input": "git pull origin main",                    "class": 1},
  {"input": "pull remote commits",                     "class": 2},
  {"input": "update my local branch from remote",      "class": 2},
  {"input": "fetch and merge remote changes",          "class": 2}
]
source: "system"
validation_status: "validated"
```

---

## Step 2.x — Additional File-Read Tier-0 Recipes

> Three additional Tier-0 file-read variants: reading the first N lines (head), reading the
> last N lines (tail), and checking whether a file exists before reading. Each is a distinct
> use-case that routes directly to the right variant without LLM disambiguation.

### PythonCode: `pc-exec-read-file-head` (class 22)

> §shell-safe-fixed for line count: head reads lines 1-N deterministically.

```
name:        "pc-exec-read-file-head"
description: "Orchestrator executor: reads the first 50 lines of a file via builtin.read_file.
              Input: vars.slot0 = file path. Output: {content, line_count, path}."
content: |
  # Pre-baked head variant: reads lines 1-50. No user input in range → Tier 0 safe.
  _path = "{{vars.slot0}}"
  result = __execute_action__("read_file", {"path": _path, "range": "1-50"})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### PythonCode: `pc-exec-read-file-tail` (class 22)

```
name:        "pc-exec-read-file-tail"
description: "Orchestrator executor: reads the last 50 lines of a file (lines -50 onward).
              Input: vars.slot0 = file path. Output: {content, line_count, path}."
content: |
  # Tail variant: reads from line (total - 50) onward. First get line_count, then slice.
  _path = "{{vars.slot0}}"
  _info = __execute_action__("read_file", {"path": _path, "range": "1-1"})
  _total = _info.get("line_count", 1) if isinstance(_info, dict) else 1
  _start = max(1, _total - 49)
  result = __execute_action__("read_file", {"path": _path, "range": str(_start) + "-" + str(_total)})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### PythonCode: `pc-exec-file-exists` (class 22)

```
name:        "pc-exec-file-exists"
description: "Orchestrator executor: checks whether a file exists by attempting to read line 1.
              Input: vars.slot0 = file path. Output: {exists: bool, path: string}."
content: |
  # Check existence by reading line 1. If tool returns an error, file doesn't exist.
  _path = "{{vars.slot0}}"
  try:
      _r = __execute_action__("read_file", {"path": _path, "range": "1-1"})
      result = {"exists": True, "path": _path, "line_count": _r.get("line_count", 0) if isinstance(_r, dict) else 0}
  except Exception:
      result = {"exists": False, "path": _path}
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Leaf Skill: `skill-read-file-head` (class 1)

```
name:        "skill-read-file-head"
class_code:  1
description: "Leaf skill: how to read the first N lines of a file (head pattern)."
body: |
  Use pc-exec-read-file-head to read the first 50 lines of a file without loading the
  whole file. Useful for inspecting file headers, licence blocks, or configuration prefixes.
  For a custom line count (other than 50), use skill-read-file-range with an explicit range.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Leaf Skill: `skill-read-file-tail` (class 1)

```
name:        "skill-read-file-tail"
class_code:  1
description: "Leaf skill: how to read the last N lines of a file (tail pattern)."
body: |
  Use pc-exec-read-file-tail to read the last 50 lines of a file without loading the whole file.
  Useful for reading logs, recent entries, or the end of an append-only file.
  The helper first probes line_count via a range='1-1' read, then fetches the tail window.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Leaf Skill: `skill-file-exists` (class 1)

```
name:        "skill-file-exists"
class_code:  1
description: "Leaf skill: how to check whether a file exists before reading or writing."
body: |
  Use pc-exec-file-exists to probe whether a file exists before attempting a full read or
  write. Returns {exists: bool, path}. Use this before skill-read-file to avoid surfacing
  a 'file not found' error to the user when existence is uncertain. Also use before
  skill-write-file-replace to confirm whether to overwrite or create-new.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Recipe: `file-read-head` (class 21)

> **Tier:** 0 — reads the first 50 lines. Fixed line-count range → no LLM needed.

```
name:        "file-read-head"
description: "Read the first 50 lines of a file (head)."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-read-file>"],
    "label":   "Pre-load ts-read-file ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-read-file-head>"],
    "label":   "PythonCode calls __execute_action__(read_file, {path, range:'1-50'})"
  }
]
intent_examples: [
  {"input": "show me the top of this file",            "class": 2},
  {"input": "read the first few lines",                "class": 1},
  {"input": "show the beginning of the file",          "class": 1},
  {"input": "head of this file",                       "class": 1},
  {"input": "first 50 lines",                          "class": 1},
  {"input": "show me the file header",                 "class": 1},
  {"input": "read the start of this file",             "class": 2},
  {"input": "show the top lines of this log",          "class": 2},
  {"input": "first lines of this config file",         "class": 2},
  {"input": "file head",                               "class": 1}
]
source: "system"
validation_status: "validated"
```

### Recipe: `file-read-tail` (class 21)

> **Tier:** 0 — reads the last 50 lines. No LLM needed.

```
name:        "file-read-tail"
description: "Read the last 50 lines of a file (tail)."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-read-file>"],
    "label":   "Pre-load ts-read-file ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-read-file-tail>"],
    "label":   "PythonCode probes line_count then calls __execute_action__(read_file, {path, range:N-total})"
  }
]
intent_examples: [
  {"input": "show me the end of this file",            "class": 2},
  {"input": "tail of this file",                       "class": 1},
  {"input": "read the last few lines",                 "class": 1},
  {"input": "last 50 lines",                           "class": 1},
  {"input": "show the bottom of the log",              "class": 2},
  {"input": "show recent log entries",                 "class": 2},
  {"input": "read the end of this file",               "class": 2},
  {"input": "show latest lines in this log file",      "class": 2},
  {"input": "file tail",                               "class": 1},
  {"input": "last lines of the file",                  "class": 1}
]
source: "system"
validation_status: "validated"
```

### Recipe: `file-exists` (class 21)

> **Tier:** 0 — existence check via read probe. No LLM needed.

```
name:        "file-exists"
description: "Check whether a file exists at the given path."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-read-file>"],
    "label":   "Pre-load ts-read-file ToolSkill binding (used for existence probe)"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-file-exists>"],
    "label":   "PythonCode tries reading line 1; returns {exists: bool, path}"
  }
]
intent_examples: [
  {"input": "does this file exist",                    "class": 1},
  {"input": "check if a file exists",                  "class": 1},
  {"input": "file exists check",                       "class": 1},
  {"input": "does the path exist",                     "class": 1},
  {"input": "is there a file at this path",            "class": 2},
  {"input": "check whether this path is valid",        "class": 2},
  {"input": "verify the file is present",              "class": 2},
  {"input": "file existence check",                    "class": 1},
  {"input": "does config.toml exist",                  "class": 2},
  {"input": "is this file present in the workspace",   "class": 2}
]
source: "system"
validation_status: "validated"
```

---

## Step 11.x — Memory Write Append Variant

> `memory-write-append` is a Tier-1 variant for appending new content to an existing memory
> document without replacing it. This is a common pattern for logs, running notes, and
> incremental context updates. The LLM reads the existing content first, then writes the
> combined result.

### PythonCode: `pc-exec-memory-append` (class 22)

> Tier 1 helper: reads existing content and appends the new text.

```
name:        "pc-exec-memory-append"
description: "Orchestrator executor: appends text to an existing memory document.
              Reads the current content via memory_read, then writes combined content via memory_write.
              Input: vars.slot0 = path, vars.slot1 = text to append."
content: |
  # Append pattern: read existing, concat new content, write back.
  _path    = "{{vars.slot0}}"
  _new_txt = "{{vars.slot1}}"
  _existing = __execute_action__("memory_read", {"path": _path})
  _current = _existing.get("content", "") if isinstance(_existing, dict) else ""
  _combined = _current.rstrip("\n") + "\n\n" + _new_txt
  result = __execute_action__("memory_write", {"path": _path, "content": _combined})
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Leaf Skill: `skill-memory-write-append` (class 1)

```
name:        "skill-memory-write-append"
class_code:  1
description: "Leaf skill: how to append new text to an existing memory document."
body: |
  Use pc-exec-memory-append to add content to an existing memory document without overwriting
  it. This pattern reads the current content, appends the new text (with blank line separation),
  and writes back. Use for:
  - Running logs (CHANGELOG.md, decision_log.md)
  - Incremental session notes where each session adds an entry
  - Any document that grows over time
  If the document does not exist yet, use skill-memory-write-log to create it first.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Recipe: `memory-write-append` (class 21)

> **Tier:** 1 — the LLM decides what text to append based on context.

```
name:        "memory-write-append"
description: "Append new content to an existing memory document (read-concat-write)."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-memory-write-append>", "<uuid:skill-memory-read>"],
    "label":   "Load append + read leaf skills"
  },
  {
    "step_id": "step-2",
    "type":    "llm",
    "label":   "LLM composes the new text to append based on current context"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-memory-read>", "<uuid:ts-memory-write>"],
    "label":   "Pre-load ts-memory-read and ts-memory-write ToolSkill bindings"
  }
]
intent_examples: [
  {"input": "append to my memory document",            "class": 2},
  {"input": "add a note to my memory file",            "class": 2},
  {"input": "log this to my memory",                   "class": 2},
  {"input": "add an entry to the log",                 "class": 2},
  {"input": "append to CHANGELOG.md",                  "class": 1},
  {"input": "add this to my running notes",            "class": 2},
  {"input": "update memory log with this entry",       "class": 2},
  {"input": "memory append",                           "class": 1},
  {"input": "add a new session entry to memory",       "class": 2},
  {"input": "log this decision to memory",             "class": 2}
]
source: "system"
validation_status: "validated"
```

---

## Step 20.x.2 — Additional Pure-Logic PythonCode Helpers (path, number, regex)

> These helpers extend the string/list/dict/csv set from Step 20.x.
> All are pure-logic, no I/O, no imports, no `__execute_action__` calls.
> They transform data from preceding tool results.

### PythonCode: `pc-path-join` (class 22)

```
name:        "pc-path-join"
description: "PythonCode helper: join two path segments with a '/' separator.
              Input: vars.slot0 = base path, vars.slot1 = sub-path. Output: joined path string."
content: |
  # Pure path join — no os.path import needed. Uses string concat with normalization.
  _base = "{{vars.slot0}}".rstrip("/")
  _sub  = "{{vars.slot1}}".lstrip("/")
  result = (_base + "/" + _sub) if _sub else _base
consumer_tags: ["02:orchestrator"]
source:        "system"
validation_status: "validated"
```

### PythonCode: `pc-path-basename` (class 22)

```
name:        "pc-path-basename"
description: "PythonCode helper: extract the filename (last path component) from a path.
              Input: vars.slot0 = path string. Output: basename string."
content: |
  # Pure basename — split on '/' and take the last non-empty component.
  _path = "{{vars.slot0}}"
  _parts = [p for p in _path.split("/") if p]
  result = _parts[-1] if _parts else ""
consumer_tags: ["02:orchestrator"]
source:        "system"
validation_status: "validated"
```

### PythonCode: `pc-path-dirname` (class 22)

```
name:        "pc-path-dirname"
description: "PythonCode helper: extract the directory part of a path.
              Input: vars.slot0 = path string. Output: directory path string."
content: |
  # Pure dirname — split on '/' and drop the last component.
  _path = "{{vars.slot0}}"
  _parts = [p for p in _path.split("/") if p]
  result = "/" + "/".join(_parts[:-1]) if len(_parts) > 1 else "/"
consumer_tags: ["02:orchestrator"]
source:        "system"
validation_status: "validated"
```

### PythonCode: `pc-number-parse` (class 22)

```
name:        "pc-number-parse"
description: "PythonCode helper: parse a string to int or float.
              Input: vars.slot0 = string value. Output: int or float, or None on failure."
content: |
  # Try int first, then float, then return None.
  _val = "{{vars.slot0}}".strip()
  try:
      result = int(_val)
  except ValueError:
      try:
          result = float(_val)
      except ValueError:
          result = None
consumer_tags: ["02:orchestrator"]
source:        "system"
validation_status: "validated"
```

### PythonCode: `pc-regex-match` (class 22)

> Note: the `re` module may or may not be available in the sandbox. This helper uses
> a conservative approach — if re is unavailable, falls back to substring check.

```
name:        "pc-regex-match"
description: "PythonCode helper: test whether a string matches a regex pattern.
              Input: vars.slot0 = text, vars.slot1 = pattern. Output: {matched: bool, groups: []}."
content: |
  # Regex match — pure substring fallback (no imports). For full regex, use grep tool.
  # This helper performs simple 'in' containment check as a pattern-match approximation.
  # For actual regex matching, route through skill-grep-content (which uses builtin.grep).
  _text    = "{{vars.slot0}}"
  _pattern = "{{vars.slot1}}"
  # Substring containment — deterministic, no imports needed.
  _matched = _pattern in _text
  result = {"matched": _matched, "groups": [], "note": "substring containment check; use grep tool for full regex"}
consumer_tags: ["02:orchestrator"]
source:        "system"
validation_status: "validated"
```

### PythonCode: `pc-string-format` (class 22)

```
name:        "pc-string-format"
description: "PythonCode helper: format a string template with slot values.
              Input: vars.slot0 = template (uses {0}, {1} placeholders), vars.slot1 = first arg,
              vars.slot2 = second arg. Output: formatted string."
content: |
  # String format with positional arguments.
  _tmpl  = "{{vars.slot0}}"
  _arg0  = "{{vars.slot1}}"
  _arg1  = "{{vars.slot2}}"
  try:
      result = _tmpl.format(_arg0, _arg1)
  except Exception as _e:
      result = {"error": str(_e), "template": _tmpl}
consumer_tags: ["02:orchestrator"]
source:        "system"
validation_status: "validated"
```

---

## Step 2.x.2 — Combined Workflow Recipes

> Orchestrator-first combined workflows that chain two Tier-0 operations without LLM.
> These cover the most common two-step read+search patterns. Each recipe chains
> two PythonCode executors: the first does a read, the second does a search on the result.

### PythonCode: `pc-exec-read-then-grep` (class 22)

```
name:        "pc-exec-read-then-grep"
description: "Orchestrator executor: reads a file then greps the content for a pattern.
              Input: vars.slot0 = path, vars.slot1 = grep pattern.
              Output: matching lines list."
content: |
  # Read file content, then filter lines matching the pattern.
  _path    = "{{vars.slot0}}"
  _pattern = "{{vars.slot1}}"
  _file_result = __execute_action__("read_file", {"path": _path})
  _content = _file_result.get("content", "") if isinstance(_file_result, dict) else str(_file_result)
  _lines = _content.split("\n")
  result = [_l for _l in _lines if _pattern in _l]
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### PythonCode: `pc-exec-list-then-grep` (class 22)

```
name:        "pc-exec-list-then-grep"
description: "Orchestrator executor: lists directory entries then filters by name substring.
              Input: vars.slot0 = directory path, vars.slot1 = name filter substring.
              Output: filtered entry names list."
content: |
  # List directory, then filter entries by name substring.
  _dir    = "{{vars.slot0}}"
  _filter = "{{vars.slot1}}"
  _list_result = __execute_action__("list_dir", {"path": _dir})
  _entries = _list_result if isinstance(_list_result, list) else (
      _list_result.get("entries", []) if isinstance(_list_result, dict) else []
  )
  result = [_e for _e in _entries if _filter in str(_e)]
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

### Leaf Skill: `skill-read-and-grep` (class 1)

```
name:        "skill-read-and-grep"
class_code:  1
description: "Leaf skill: how to read a file and filter its content by a pattern in one step."
body: |
  Use pc-exec-read-then-grep when you need to find specific lines in a known file without
  running a separate grep tool call. This is more efficient than read_file + grep as a
  separate step for small-to-medium files. For large files or multi-file searches, prefer
  skill-grep-content instead. Returns a list of matching lines.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Leaf Skill: `skill-list-and-filter` (class 1)

```
name:        "skill-list-and-filter"
class_code:  1
description: "Leaf skill: how to list a directory and filter the entries by name in one step."
body: |
  Use pc-exec-list-then-grep when you need to enumerate a directory and immediately
  narrow results by a name substring (e.g. "show me all Python files in src/"). This
  avoids a separate glob call for simple substring name filters. For extension-based
  filtering, prefer skill-glob-by-extension for exact extension matching.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

### Recipe: `file-read-and-grep` (class 21)

> **Tier:** 0 — reads a file and filters lines by pattern. No LLM needed.

```
name:        "file-read-and-grep"
description: "Read a file and return lines matching a pattern (combined read+filter, Tier 0)."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-read-file>"],
    "label":   "Pre-load ts-read-file ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-read-then-grep>"],
    "label":   "PythonCode reads file then filters lines matching vars.slot1 pattern"
  }
]
intent_examples: [
  {"input": "read this file and find lines with X",    "class": 2},
  {"input": "show me lines matching this pattern",     "class": 2},
  {"input": "read and filter this file",               "class": 2},
  {"input": "find matching lines in this file",        "class": 2},
  {"input": "search this file for a string",           "class": 2},
  {"input": "grep inside a specific file",             "class": 2},
  {"input": "file read and grep",                      "class": 1},
  {"input": "read file and show matching lines only",  "class": 2},
  {"input": "filter lines in this log file",           "class": 2},
  {"input": "what lines in this file contain X",       "class": 2}
]
source: "system"
validation_status: "validated"
```

### Recipe: `file-list-and-filter` (class 21)

> **Tier:** 0 — lists directory and filters entries by name substring. No LLM needed.

```
name:        "file-list-and-filter"
description: "List a directory and return entries whose name contains a filter string."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-list-dir>"],
    "label":   "Pre-load ts-list-dir ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-list-then-grep>"],
    "label":   "PythonCode lists directory then filters entries by name substring"
  }
]
intent_examples: [
  {"input": "list files containing test in the name",  "class": 2},
  {"input": "show only config files in this directory","class": 2},
  {"input": "find files with this name pattern",       "class": 2},
  {"input": "list and filter directory entries",       "class": 2},
  {"input": "show all test files",                     "class": 2},
  {"input": "filter directory listing by name",        "class": 2},
  {"input": "file list filtered by name",              "class": 1},
  {"input": "list entries matching this substring",    "class": 2},
  {"input": "which files in this dir have this word",  "class": 2},
  {"input": "directory filtered listing",              "class": 1}
]
source: "system"
validation_status: "validated"
```

---

## Final — Component Summary & Seeding Order

### Complete Component Count (v3 builtin stack)

| Class | Type | Count | Component names |
|-------|------|-------|-----------------|
| 0 | Tool | 23 | builtin.shell, read_file, write_file, list_dir, glob, grep, apply_patch, http, http.save, memory_search, memory_write, memory_read, memory_tree, time, json, skill_list, skill_install, skill_remove, trigger_create, trigger_list, trigger_remove, spawn_subagent, echo |
| 13 | ToolSkill | 30 | ts-shell-run, ts-read-file, ts-write-file, ts-list-dir, ts-glob, ts-grep, ts-apply-patch, ts-http-fetch, ts-http-save, ts-memory-search, ts-memory-write, ts-memory-read, ts-memory-tree, ts-time-now, ts-time-parse, ts-time-convert, **ts-time-diff**, **ts-time-format**, ts-json-query, ts-json-stringify, ts-json-validate, ts-skill-list, ts-skill-install, ts-skill-remove, ts-trigger-create, ts-trigger-list, ts-trigger-remove, ts-spawn-subagent, ts-web-search, ts-echo |
| 22 | PythonCode | 98 | pc-exec-read-file, pc-exec-write-file, pc-exec-list-dir, pc-exec-list-filter-by-type, pc-exec-glob, pc-exec-grep, pc-exec-grep-case-insensitive, pc-exec-grep-type-filtered, **pc-exec-grep-invert**, pc-exec-apply-patch, pc-exec-http-get, pc-exec-http-get-authenticated, pc-exec-http-post, pc-exec-http-head, pc-exec-http-put, pc-exec-http-patch, pc-exec-http-delete, pc-exec-http-save, pc-exec-memory-search, pc-exec-memory-write, pc-exec-memory-patch, pc-exec-memory-read, pc-exec-memory-tree, pc-exec-time-now, pc-exec-time-parse, pc-exec-time-convert, **pc-exec-time-diff**, **pc-exec-time-format**, pc-exec-json-query, pc-exec-json-stringify, pc-exec-json-validate, pc-exec-skill-list, pc-exec-trigger-list, **pc-exec-trigger-list-active**, **pc-exec-trigger-list-scheduled**, **pc-exec-trigger-resolve-and-remove**, pc-http-status-check, pc-json-extract-field, pc-memory-extract-section, pc-memory-format-entry, pc-url-encode, pc-web-search-extract, pc-web-search-query-build, pc-exec-echo, pc-exec-shell-git-status, pc-exec-shell-git-log, pc-exec-shell-git-diff-stat, pc-exec-shell-git-branch, pc-exec-shell-git-stash-list, pc-exec-shell-git-log-n, pc-exec-shell-git-remote, pc-exec-shell-git-show-stat, pc-exec-shell-git-tag-list, **pc-exec-shell-git-diff-name-only**, **pc-exec-shell-git-log-stat**, **pc-exec-shell-git-stash-show**, **pc-exec-shell-git-config-list**, **pc-exec-shell-git-add**, pc-exec-shell-pwd, pc-exec-shell-df, pc-exec-shell-ps, pc-exec-shell-env, pc-exec-shell-uname, pc-exec-shell-which, pc-exec-shell-date, pc-exec-shell-hostname, pc-exec-shell-whoami, pc-exec-shell-uptime, pc-exec-shell-free, pc-exec-shell-wc-l, **pc-string-split**, **pc-string-join**, **pc-string-strip**, **pc-string-replace**, **pc-string-contains**, **pc-list-filter-nonempty**, **pc-list-slice**, **pc-list-unique**, **pc-dict-pick**, **pc-dict-merge**, **pc-csv-parse-lines**, **pc-csv-rows-to-text** |
| 1 | Leaf Skill | 105 | skill-shell-run, skill-shell-safe-check, skill-shell-git-status, skill-shell-git-log, skill-shell-git-diff-stat, skill-shell-git-branch, skill-shell-git-stash-list, skill-shell-pwd, skill-shell-df, skill-shell-ps, skill-shell-env, skill-shell-uname, skill-shell-which, skill-shell-git-remote, skill-shell-git-show-stat, skill-shell-git-tag-list, skill-shell-date, skill-shell-hostname, skill-shell-whoami, skill-shell-uptime, skill-shell-free, skill-shell-wc-l, **skill-shell-git-diff-name-only**, **skill-shell-git-log-stat**, **skill-shell-git-stash-show**, **skill-shell-git-config-list**, **skill-shell-git-commit**, **skill-shell-git-push**, **skill-shell-git-pull**, **skill-shell-git-fetch**, skill-read-file, skill-read-file-range, **skill-read-file-head**, **skill-read-file-tail**, **skill-file-exists**, skill-write-file-new, skill-write-file-replace, skill-write-file-template, skill-list-dir, skill-list-dir-recursive, skill-list-dir-files-only, skill-list-dir-dirs-only, skill-glob-by-extension, skill-glob-by-name, skill-glob-in-subdir, skill-grep-files, skill-grep-content, skill-grep-count, skill-grep-case-insensitive, skill-grep-type-filtered, **skill-grep-invert**, **skill-read-and-grep**, **skill-list-and-filter**, skill-apply-patch-single, skill-apply-patch-all, skill-http-get, skill-http-post, skill-http-authenticated, skill-http-head, skill-http-put, skill-http-patch, skill-http-delete, skill-http-save-download, skill-http-save-api, skill-memory-search, skill-memory-search-broad, skill-memory-write-log, skill-memory-write-main, skill-memory-write-patch, **skill-memory-write-append**, skill-memory-read, skill-memory-tree, **skill-memory-search-and-read**, skill-time-now, skill-time-parse, skill-time-convert, **skill-time-diff**, **skill-time-format**, skill-json-query, skill-json-stringify, skill-json-parse, skill-json-validate, **skill-json-parse-and-query**, skill-skill-list, skill-skill-install, skill-skill-remove, skill-trigger-list, skill-trigger-create, skill-trigger-remove, **skill-trigger-list-active**, **skill-trigger-list-scheduled**, skill-spawn-subagent, skill-spawn-named-procedure, skill-web-search, **skill-spawn-research**, **skill-spawn-coding**, **skill-spawn-exploration**, **skill-spawn-query**, **skill-shell-git-add** (105 total) |
| 2 | Domain Skill | 9 | skill-filesystem, skill-http, skill-memory, skill-shell, skill-skills, skill-triggers, skill-subagent, skill-time, skill-json |
| 21 | Recipe | 118 | file-read, file-read-range, **file-read-head**, **file-read-tail**, **file-exists**, **file-read-and-grep**, **file-list-and-filter**, file-write, file-write-template, file-list, file-list-recursive, file-list-files-only, file-list-dirs-only, file-glob, file-glob-by-extension, file-glob-by-name, file-glob-in-subdir, file-glob-recent, file-grep, file-grep-files, file-grep-content, file-grep-count, file-grep-case-insensitive, file-grep-type-filtered, **file-grep-invert**, file-patch, file-patch-replace-all, http-get, http-get-json, http-authenticated-get, http-head, http-post, http-post-json-webhook, http-put, http-patch, http-delete, http-save, http-save-large, memory-search, memory-search-broad, memory-write, memory-write-log, memory-write-main, memory-write-patch, **memory-write-append**, memory-read, memory-read-main, memory-read-heartbeat, memory-tree, memory-tree-deep, **memory-search-and-read**, time-now, time-now-tz, time-parse, time-convert, **time-diff**, **time-format**, json-query, json-stringify, json-parse, json-validate, **json-parse-and-query**, skill-list, skill-list-user-only, skill-list-system-only, skill-install, skill-remove, trigger-list, trigger-create, trigger-remove, trigger-remove-by-name, **trigger-list-active**, **trigger-list-scheduled**, subagent-spawn, **subagent-research**, **subagent-coding**, **subagent-exploration**, **subagent-query**, web-search, echo-ping, shell-run, shell-script, **shell-git-fetch**, **shell-git-add**, **shell-git-commit**, **shell-git-push**, **shell-git-pull**, shell-git-status, shell-git-log, shell-git-diff-stat, shell-git-branch, shell-git-stash-list, shell-git-remote, shell-git-show-stat, shell-git-tag-list, shell-pwd, shell-df, shell-ps, shell-env, shell-uname, shell-which, shell-date, shell-hostname, shell-whoami, shell-uptime, shell-free, shell-wc-l, **shell-git-diff-name-only**, **shell-git-log-stat**, **shell-git-stash-show**, **shell-git-config-list** |
| 23 | ExtensionCatalogue | 24 | builtin-filesystem, builtin-network, builtin-memory, builtin-process, builtin-management, ext-read-file, ext-write-file, ext-list-dir, ext-glob, ext-grep, ext-apply-patch, ext-http, ext-http-save, ext-memory-search, ext-memory-write, ext-memory-read, ext-memory-tree, ext-time, ext-json, ext-shell, ext-skill-management, ext-trigger-management, ext-spawn-subagent, ext-web-search |

> **Actual totals (v3-final, all optimizations applied):** 23 Tools + 30 ToolSkills + 98 PythonCode + 105 Leaf Skills + 9 Domain Skills + 118 Recipes + 24 ExtensionCatalogues = **407 components**
>
> **Changes in this revision (v3-revised, since v3-base 368):**
>
> *Bug fixes (interrupted optimization resolved):*
> - All ToolSkill `tool_name:` fields now use the short name without `builtin.` prefix — consistent with all other ToolSkills and with `__execute_action__()` call sites. Affected: `ts-skill-list/install/remove`, `ts-trigger-create/list/remove`, `ts-spawn-subagent`, `ts-echo`, `ts-web-search`.
> - `pc-web-search-extract` removed illegal `import json` statement — replaced with pure dict access (http tool returns parsed dict).
> - `pc-web-search-query-build` removed illegal `import urllib.parse` — now delegates to `url_encode` action.
> - Duplicate `pc-path-dirname` definition removed; `validation_exists` typo corrected to `validation_status`.
>
> *New components (+16 PythonCode, +10 Leaf Skills, +10 Recipes):*
> - +16 PythonCode: `pc-exec-read-file-head`, `pc-exec-read-file-tail`, `pc-exec-file-exists`, `pc-exec-read-then-grep`, `pc-exec-list-then-grep`, `pc-exec-memory-append`, `pc-exec-shell-git-commit`, `pc-exec-shell-git-push`, `pc-exec-shell-git-pull`, `pc-exec-shell-git-fetch`, `pc-path-join`, `pc-path-basename`, `pc-path-dirname`, `pc-number-parse`, `pc-regex-match`, `pc-string-format`
> - +10 Leaf Skills: `skill-read-file-head`, `skill-read-file-tail`, `skill-file-exists`, `skill-read-and-grep`, `skill-list-and-filter`, `skill-memory-write-append`, `skill-shell-git-commit`, `skill-shell-git-push`, `skill-shell-git-pull`, `skill-shell-git-fetch`
> - +10 Recipes: `file-read-head` (T0), `file-read-tail` (T0), `file-exists` (T0), `file-read-and-grep` (T0), `file-list-and-filter` (T0), `memory-write-append` (T1), `shell-git-fetch` (T0), `shell-git-commit` (T1), `shell-git-push` (T1), `shell-git-pull` (T1)
>
> *Cumulative delta since first published plan (319 components to 404):*
> - PythonCode: 62 to 97 (+35)
> - Leaf Skills: 76 to 104 (+28)
> - Recipes: 95 to 117 (+22)
>
> **Tier-0 recipe count: 89 out of 118** (75%).
> Orchestrator handles 75% of all built-in tasks completely autonomously.
> LLM involvement required for 25% (shell-run/script, git-add, git-commit/push/pull, spawn variants, file-write, file-patch, memory-write*, memory-write-append, http-save-large, http-post-webhook, trigger-create, skill-install/remove, web-search, memory-search-and-read — all creative, write, spawn, or destructive operations).
>
> **Design invariants enforced (v3-revised):**
> - All ToolSkill `tool_name:` use short names (no `builtin.` prefix) — matches Tool `name:` field and `__execute_action__()` calls
> - All PythonCode bodies use no `import` statements (pure sandbox execution only)
> - All Tier-0 recipes have both `channel:"rust"` pre-load step AND `channel:"orchestrator"` PythonCode dispatch step
> - `shell-guard-custom` enforced: all git-write recipes are `llm_call_required: true`
> - `shell-safe-fixed`: `shell-git-fetch` uses fixed literal `"git fetch --all"`, is Tier 0

---

### Seeding Order (builtin_bootstrap.rs per group)

Each group follows this invariant insertion order to satisfy FK references:

```
For each domain group:
  1. ExtensionCatalogue row  (class 23 — owns all children by UUID ref)
  2. Tool rows               (class  0 — capability_id = "builtin.X")
  3. ToolSkill rows          (class 13 — references tool_name)
  4. PythonCode rows         (class 22 — standalone, no FK deps)
  5. Leaf Skill rows         (class  1 — reference ToolSkill names in body text)
  6. Domain Skill rows       (class  2 — reference leaf skill names in body text)
  7. Recipe rows             (class 21 — step_descriptions reference UUIDs of all above)
     → for each Recipe: run IBS build_instruction pre-flight before insert
     → seed intent_examples into reborn_intent_inputs
```

#### Group insertion order

| Pass | Group | Primary ExtCatalogue | Per-tool ExtCatalogues | Tools | ToolSkills | PythonCode | Leaf Skills | Domain Skills | Recipes |
|------|-------|----------------------|------------------------|-------|------------|------------|-------------|---------------|---------|
| 1 | filesystem | builtin-filesystem | ext-read-file, ext-write-file, ext-list-dir, ext-glob, ext-grep, ext-apply-patch | 6 | 6 | 19 | 30 | 1 | 35 |
| 2 | network | builtin-network | ext-http, ext-http-save, ext-web-search | 2 | 3 | 14 | 11 | 1 | 14 |
| 3 | memory | builtin-memory | ext-memory-search, ext-memory-write, ext-memory-read, ext-memory-tree | 4 | 4 | 9 | 9 | 1 | 14 |
| 4 | process | builtin-process | ext-shell, ext-spawn-subagent, ext-trigger-management | 5 | 1 | 42 | 43 | 3 | 41 |
| 5 | management | builtin-management | ext-skill-management, ext-time, ext-json | 6 | 9 | 9 | 15 | 3 | 14 |

> *(Counts updated to reflect all additions in v3-final. filesystem group gained +7 PythonCode (head/tail/exists/read-then-grep/list-then-grep + 2 path helpers), +8 Leaf Skills, +10 Recipes. memory group gained +2 PythonCode, +1 Leaf Skill, +2 Recipes. process/shell group gained +9 PythonCode (git-write + fetch + git-add), +5 Leaf Skills, +5 Recipes. Total: 407 components.)*

---

### Idempotency Guard

The seeder checks for existing builtin components before any insert:

```rust
// Before inserting any group:
let existing = sqlx::query_scalar!(
    "SELECT COUNT(*) FROM reborn_components WHERE source = 'system'"
)
.fetch_one(pool)
.await?;

if existing > 0 {
    return Ok(()); // Already seeded — skip
}
```

This ensures the seeder is safe to call on every composition boot without
producing duplicate rows.

---

### Q1 Pre-flight Invariants (checked inside seeder, panic in debug builds on failure)

| Rule | What is checked |
|------|----------------|
| §shell-guard-custom | Every recipe with `builtin.shell` in rust_steps AND command derived from user input has `llm_call_required=true` |
| §shell-safe-fixed | Every recipe with `builtin.shell` that is `llm_call_required=false` MUST have a PythonCode executor that hardcodes the command as a fixed literal (no slot interpolation of command content) |
| §spawn_subagent-guard | Every recipe with `builtin.spawn_subagent` in rust_steps has `llm_call_required=true` |
| §tier0-orchestrator-channel Rule 1 | Every Tier-0 orchestrator step contains only PythonCode (class 22) UUIDs |
| §tier0-orchestrator-channel Rule 2 | Every recipe with `llm_call_required=false` AND rust_steps tool_bindings has ≥1 PythonCode UUID in orchestrator_steps |
| §capability-id | Every Tool row's capability_id matches a known `BuiltinFirstPartyTools` variant |
| §non-empty-overview | Every ExtensionCatalogue has non-empty `overview_doc` |
| §non-empty-body | Every ToolSkill, Leaf Skill, Domain Skill has non-empty body/content |

---

*End of builtin_stuff_v3.md — all Steps + Final section complete. v3-final: 407 components, 118 Recipes (89 Tier-0 = 75%), 24 ExtensionCatalogues (5 global domain + 19 per-tool), orchestrator-first design.*

*Full builtin coverage: all 23 tools covered; tool_name prefix inconsistency fixed (no more `builtin.X` in ToolSkill tool_name fields); illegal PythonCode imports removed (pc-web-search-extract, pc-web-search-query-build); 5 git write recipes (add/commit/push/pull/fetch); 3 file-read variants (head/tail/exists); 2 combined workflow recipes (file-read-and-grep, file-list-and-filter); memory-write-append; 7 new pure-logic PythonCode helpers (path-join/basename/dirname, number-parse, regex-match, string-format, pc-exec-memory-append). Design principle section expanded with explicit two-channel model and orchestrator-first hierarchy. The orchestrator handles 75% of all built-in tasks autonomously — LLM involvement required for only 25% (creative, write, spawn, and destructive operations).*

---

## Step 27 — Host-Call Component Stack (orchestrator infrastructure → components)

> **Purpose:** The Monty Orchestrator talks to the Rust Executioner through the
> **Recipe-Skill-Tool System** — not bare `__host_call__` intrinsics. Per the
> locked architecture (CLAUDE.md "Execution model — Orchestrator + Executioner"),
> the host calls are dissected into the **same v3 component vocabulary** already
> used by Steps 1–26 — class-0 Tools (Rust verbs), class-13 ToolSkills (call
> syntax), class-1/2 Skills (Orchestrator how-to), class-21 Recipes (intent
> flows), class-22 PythonCode (step bodies), class-23 ExtensionCatalogues.
>
> **Key principle — Rust holds only generic verbs; the logic lives in
> Recipes/Skills:**
> - **Tool (Rust)** = a capability/verb group (`component_store`, `intent`,
>   `composition`, `chat`, `http`, `memory`, `regex`, `signal`, `validation`).
>   The LLM is **NOT** a Rust Tool — see "LLM invocation = Kohai-mediated" below.
> - **ToolSkill (Rust)** = one verb under a Tool + its call syntax/params
>   (`component.fetch`, `intent.resolve`, `composition.compose`,
>   `chat.post_reply`, `memory.write`, …).
> - **Skill (Monty)** = narrative how-to for the Orchestrator, hierarchical.
> - **Recipe (Monty)** = the intent-specific step-script (PythonCode steps using
>   Skills/ToolSkills) the Orchestrator runs.
>
> **One Rust Tool/ToolSkill is reused by many Orchestrator Skills + Recipes**
> across different intents. The high-level flows that **compose** generic verbs —
> `assemble_prior_knowledge`, `non_match_llm_answer`, `save_history` — are
> **Recipes, NOT Rust Tools**. The flows that **are** a single Rust mechanism —
> `resolve_intent` (the whole intent system is one Tool), `compose_orchestrator`,
> `post_reply` — are **Tools** (see the 27.0 table).
>
> **LLM invocation = Kohai-mediated (NOT a Rust Tool, LOCKED):** the Orchestrator
> composes the prompt (a Recipe) + adds a prefix-placeholder, then sends the
> prompt to **Kohai** (Python). Kohai saves the prompt; if a **Sempai** is
> connected it adds an optimization-prefix → Sempai optimizes → returns without
> the prefix → Kohai saves the optimized prompt beside the original → Kohai adds
> the provider-LLM prefix (the one for that placeholder); if no Sempai, Kohai
> just adds the provider prefix. Kohai then sends the prompt to the provider LLM
> **by calling the existing Rust `first_party_tools/http.rs` HTTP tool**, receives
> the answer, saves it beside its prompt, and returns it to the Orchestrator.
> **`handle_llm_complete` / `LlmBackend` RETIRE as Rust host tools.** Rust↔LLM
> never communicate directly — only over the Orchestrator/Kohai.
>
> **Thread state is owned by the Orchestrator (LOCKED):** the main process is one
> long-persisting Monty run; the Orchestrator **inherently knows its own state**
> (where it is in its step sequence). The Rust stage-pipeline machinery —
> `save_checkpoint`, `transition_to`, `check_budget`, `log_budget_warning`,
> `emit_event`, `get_actions`, `record_skill_usage` — is **ALL RETIRED**: the
> agent-loop stage pipeline is no longer the driver, and no universal per-call
> wrapper means no per-step state/budget/event host verbs are needed. Chat event
> emission goes through `host.post_reply`. Security is mode-driven (Matching-Mode
> all-off / Non-Matching-Mode wrapper-on) — see CLAUDE.md.
>
> **Tool invocation = first-class callables (no `__execute_action__`):** a recipe
> step's PythonCode calls a ToolSkill directly, e.g.
> `result = host.resolve_intent(user_input=text)`. The rust-channel step binds the
> ToolSkill into the Monty namespace; the orchestrator-channel PythonCode calls
> the bound callable. `__execute_action__` / `__execute_code_step__` are
> **retired** (Model-A relics); `__execute_actions_parallel__` becomes a Python
> helper. A future MCP bridge hits the **same namespace-registry surface** — no
> string-intrinsic needed.
>
> The Rust backing fns (`handle_*` in `orchestrator.rs`) **stay** as the
> implementation behind the `host.*` Tool rows — **except** `handle_llm_complete`
> / `LlmBackend` (retired — Kohai-mediated) and `handle_execute_action` (retired
> as a universal per-call wrapper — security is now mode-driven).
>
> **capability_id prefix:** built-in system Tools use `host.<verb>` (e.g.
> `host.resolve_intent`, `host.compose_orchestrator`, `host.fetch_component`,
> `host.post_reply`). They are first-party system tools (`source: "system"`,
> `validation_status: "validated"`), registered in `BuiltinFirstPartyTools`
> alongside the `builtin.*` set.
>
> **Two-channel rule still applies:** a Tier-0 recipe using a `host.*` verb has a
> `channel: "rust"` step that binds the `ts-host-*` ToolSkill into the namespace
> and a `channel: "orchestrator"` PythonCode step that calls the bound callable
> directly.

### Step 27.0 — Dissection + reuse map (LOCKED classification)

**Recipes (no Tool row) — high-level flows that compose generic verbs:**

| Host call | Disposition | Composes |
|-----------|-------------|----------|
| `__non_match_llm_answer__` (missing) | **Recipe** `host-non-match-llm-answer` | Orchestrator assembles prompt + placeholder → **Kohai** (saves; optional Sempai optimize; adds provider prefix; calls `first_party_tools/http`; saves answer) → answer back |
| `__assemble_prior_knowledge__` (deleted) | **Recipe** `host-assemble-prior-knowledge` (**fallback ONLY — when NO prefix is present**) | PythonCode adds basic "what is going on" context so the LLM understands; no retrieval verbs (those are dropped) |
| `__save_history__` (missing) | **Recipe** `host-save-history` | PythonCode formatter + **`builtin.memory_write`** (Step 11) — the shared SQL-saving Tool — to the same store Kohai saved the prompt/answer |

**Tools (Rust, registered as `host.*`) — single Rust mechanisms:**

| Host call | Tool + ToolSkill | Backs / notes |
|-----------|------------------|---------------|
| `__resolve_intent__` (missing) | `host.resolve_intent` + `ts-host-resolve-intent` | the **whole intent system is ONE Tool** — `intent_system.rs::resolve_intent` logic stays in Rust |
| `__compose_orchestrator__` (missing; collapses `__fetch_recipe__`) | `host.compose_orchestrator` + `ts-host-compose-orchestrator` | **REWRITE** — fetch + split formatters; Rust part significantly reduced; Recipe + Component structure reworked to match |
| `__post_reply__` (missing) | `host.post_reply` + `ts-host-post-reply` | **A1 (LOCKED):** Monty is sandboxed (no direct chat socket) → only Rust touches the chat window → `post_reply` is a Tool the Orchestrator calls |
| `__fetch_component__` | `host.fetch_component` + `ts-host-fetch-component` | component store (SEC-01 gate) — **KEPT** |
| `__resolve_component_by_name__` | `host.resolve_component_by_name` + `ts-host-resolve-component-by-name` | component store (SEC-01 gate) — **KEPT** |
| `__validate_component__` | `host.validate_component` + `ts-host-validate-component` | validation queue (kohai/sempai) |
| `__check_signals__` | `host.check_signals` + `ts-host-check-signals` | signal receiver |
| `__kohai_complete__` (new — implied by the Kohai-mediated LLM model) | `host.kohai_complete` + `ts-host-kohai-complete` | wraps the existing `brassclaw_interceptor` ingress — the Orchestrator→Kohai handoff (Kohai saves prompt / optional Sempai optimize / adds provider prefix / calls `first_party_tools/http` / saves answer / returns). No new logic — wiring. Defined at 27.10.3 |

**Reused existing Tools (no new Tool row):**

| Mechanism | Reused Tool | Source |
|-----------|-------------|--------|
| SQL saving (Kohai persist prompt/answer + `host-save-history`) | `builtin.memory_write` (Step 11) | the shared SQL-saving primitive — **no new SQL tool** (LOCKED Q-C) |
| LLM HTTP transport (Kohai → provider LLM) | `first_party_tools/http.rs` | existing generic HTTP tool — **no new Rust** (LOCKED Q-B) |
| Skill listing | `builtin.skill_list` (Step 16) | |
| Regex match | `pc-regex-match` (Step 20.x.2) exposed as `host.regex_match` | |

**DROPPED / RETIRED (no Tool, no Recipe):**

| Host call | Disposition |
|-----------|-------------|
| `__retrieve_docs__` | **DROPPED** (LOCKED) — prior knowledge is the fallback Recipe, not a retrieval verb |
| `__get_reduction_rules__` | **DROPPED** (LOCKED) — same |
| `__llm_complete__` | **RETIRED** — LLM invocation is Kohai-mediated (Recipe); `handle_llm_complete` / `LlmBackend` retire |
| `__save_checkpoint__`, `__transition_to__`, `__check_budget__`, `__log_budget_warning__`, `__emit_event__`, `__get_actions__`, `__record_skill_usage__` | **ALL RETIRED** (LOCKED Q-D) — the Orchestrator owns thread state (it knows where it is in its own step sequence); the agent-loop stage pipeline is no longer the driver; chat event emission goes via `host.post_reply` |
| `__execute_action__`, `__execute_code_step__` | **RETIRED** meta-primitives — the recipe step calls the ToolSkill directly |
| `__execute_actions_parallel__` | **RETIRED** meta-primitive — "call N tools" is a sequential recipe with N steps (Monty is single-threaded, so a `pc-host-execute-parallel` Python helper would degrade to sequential anyway). A parallel-step-group recipe extension may be added later only if a real concurrent case appears |

> **Net new `host.*` Tool rows:** `resolve_intent`, `compose_orchestrator`
> (rewrite), `post_reply`, `fetch_component`, `resolve_component_by_name`,
> `validate_component`, `check_signals`, `kohai_complete` (8). **Reused existing:**
> `builtin.memory_write` (shared SQL saver), `first_party_tools/http` (Kohai
> transport), `builtin.skill_list`, `pc-regex-match`. **Recipes (no Tool row):**
> `host-non-match-llm-answer`, `host-assemble-prior-knowledge` (fallback),
> `host-save-history`. **DROPPED:** `retrieve_docs`, `get_reduction_rules`.
> **RETIRED:** `llm_complete` (Rust) + 7 stage-machinery verbs + 3 meta-primitives.
> **New ExtensionCatalogue:** `builtin-host` (class 23).
>
> Substeps 27.1–27.11 below are **being re-authored to this locked
> classification** (the prior 27.1–27.11 text reflected the stale table and is
> being replaced one-by-one). 27.1–27.4 = main-process control batch; 27.5–27.11
> = the Tools, the Non-Matching-Mode + prior-knowledge Recipes, and the
> `builtin-host` catalogue.

### Step 27.1 — `host.resolve_intent` (Phase 2 — intent match)

> **Capability:** `host.resolve_intent` · **Effect:** `read` · **Permission:** Auto
> Backs `__resolve_intent__(user_input)`. Calls `intent_system.rs::resolve_intent`
> + `fetch_for_turn`. Returns `{matched, component_id, intent_id, score,
> disambiguation?}`.

#### Step 27.1.1 — Tool row (class 0)
```
name:            "host.resolve_intent"
description:     "Resolve a user input against the intent system. Returns a match
                  descriptor {matched, component_id, intent_id, score, disambiguation}.
                  Phase 2 of the basic-mode main process."
capability_id:   "host.resolve_intent"
effect_type:     "read"
param_schema: {
  "type": "object",
  "properties": {
    "user_input":   {"type": "string", "description": "The user's prompt text"},
    "chat_history": {"type": "array",  "description": "Recent turn messages (few tokens)"}
  },
  "required": ["user_input"]
}
param_template:  {"user_input": ""}
preconditions:   ""
error_handling:  "No match → {matched: false}; never raises."
consumer_tags:   ["00:rusty", "02:orchestrator"]
source:          "system"
validation_status: "validated"
```

#### Step 27.1.2 — ToolSkill: `ts-host-resolve-intent` (class 13)
```
name:          "ts-host-resolve-intent"
tool_name:     "host.resolve_intent"
description:   "Resolve user input to a component id via the intent system."
param_schema:  [
  {name: "user_input",   param_type: "string", required: true,  description: "User prompt text"},
  {name: "chat_history", param_type: "array",  required: false, description: "Recent messages for context"}
]
param_template: {"user_input": "{{user_input}}"}
preconditions:  "Intent system + component stores wired."
error_handling: "matched=false is a normal result, not an error."
category:       "management"
source:         "system"
validation_status: "validated"
```

#### Step 27.1.3 — PythonCode: `pc-host-resolve-intent` (class 22)
```python
# Channel: orchestrator | Class: 22 | No I/O, no imports, no network, no DB.
# IBS bakes in {{vars.slotN}} values before execution.
result = host.resolve_intent(user_input="{{vars.slot0}}")
```

#### Step 27.1.4 — Leaf Skill: `skill-host-resolve-intent` (class 1)
```
name:        "skill-host-resolve-intent"
class_code:  1
description: "Leaf skill: how to resolve user input to a component id (Phase 2)."
body: |
  Use `ts-host-resolve-intent` at the start of every turn to decide whether a
  recipe/instruction matches. Inspect `matched` in the result. If true, hand the
  `component_id` to `host.compose_orchestrator` (Matching-Mode). If false, fall
  through to the Non-Matching-Mode routine. Never treat `matched=false` as an
  error — it means the LLM path is required.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

#### Step 27.1.5 — Recipe: `host-resolve-intent` (class 21, Tier 0)
```
name:              "host-resolve-intent"
description:       "Resolve user input to a component id (Phase 2). Tier 0 — no LLM."
llm_call_required: false
intent_examples:   ["what can you do", "run git status", "read the readme",
                    "search memory for x", "list files", "show me the plan",
                    "grep for foo", "write a file", "parse this json", "what time is it"]
rust_steps:        [tool_binding: ts-host-resolve-intent]
orchestrator_steps:[pc-host-resolve-intent]
step_descriptions: [{"step": 0, "action": "resolve_intent", "desc": "Resolve user input to a component id or no-match."}]
tier:              0
```

### Step 27.2 — `host.compose_orchestrator` (Phase 3 — Matching-Mode compose)

> **Capability:** `host.compose_orchestrator` · **Effect:** `read` · **Permission:** Auto
> Backs `__compose_orchestrator__(component_id)`. Collapses the planned
> `__fetch_recipe__`: the composition system fetches the recipe by id, **splits**
> the rust vs orchestrator parts, loads rust bindings, and assembles the
> orchestrator-channel program + inputs. Returns `{orchestrator_program,
> rust_inputs, recipe_hint, tier}`. Monty receives the ready-to-run program and
> runs it as one continuous Python program (Option 2 — Rust does not sequence steps).

#### Step 27.2.1 — Tool row (class 0)
```
name:            "host.compose_orchestrator"
description:     "Fetch + split + assemble a recipe by component id. Returns the
                  ready-to-run orchestrator program + rust inputs + tier. Monty
                  runs the program; Rust does not sequence steps."
capability_id:   "host.compose_orchestrator"
effect_type:     "read"
param_schema: {
  "type": "object",
  "properties": {
    "component_id": {"type": "string",  "description": "UUID or name of the matched recipe/instruction"},
    "class_code":   {"type": "integer", "description": "Component class (21 recipe, 11 action, …)"}
  },
  "required": ["component_id"]
}
param_template:  {"component_id": ""}
preconditions:   "Composition recipe store + rust/orchestrator splitter wired."
error_handling:  "Miss/parse failure → {orchestrator_program: null}; caller degrades to Non-Matching-Mode."
consumer_tags:   ["00:rusty", "02:orchestrator"]
source:          "system"
validation_status: "validated"
```

#### Step 27.2.2 — ToolSkill: `ts-host-compose-orchestrator` (class 13)
```
name:          "ts-host-compose-orchestrator"
tool_name:     "host.compose_orchestrator"
description:   "Compose the orchestrator program for a matched component id."
param_schema:  [
  {name: "component_id", param_type: "string", required: true,  description: "Matched component UUID or name"},
  {name: "class_code",   param_type: "number", required: false, description: "Component class code (default 21)"}
]
param_template: {"component_id": "{{component_id}}"}
preconditions:  "Recipe store + rust/orchestrator splitter available."
error_handling: "null program → degrade to Non-Matching-Mode."
category:       "management"
source:         "system"
validation_status: "validated"
```

#### Step 27.2.3 — PythonCode: `pc-host-compose-orchestrator` (class 22)
```python
# Channel: orchestrator | Class: 22 | No I/O, no imports.
# Fetch+split+assemble the matched recipe into a runnable program.
composed = host.compose_orchestrator(component_id="{{vars.slot0}}")
# composed = {orchestrator_program, rust_inputs, recipe_hint, tier}
```

#### Step 27.2.4 — Leaf Skill: `skill-host-compose-orchestrator` (class 1)
```
name:        "skill-host-compose-orchestrator"
class_code:  1
description: "Leaf skill: how to compose a matched recipe into a runnable program."
body: |
  After `host.resolve_intent` returns a match, call `ts-host-compose-orchestrator`
  with the component_id. The host fetches the recipe, splits the rust vs
  orchestrator parts, and hands back a ready-to-run orchestrator program plus
  rust inputs and a tier hint. Run the returned program directly — do NOT
  re-sequence its steps from Rust. If `orchestrator_program` is null, degrade to
  the Non-Matching-Mode routine.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

#### Step 27.2.5 — Recipe: `host-compose-and-run-orchestrator` (class 21, Tier 0/1)
```
name:              "host-compose-and-run-orchestrator"
description:       "Matching-Mode: compose the matched recipe then run its orchestrator program. Tier depends on the composed recipe (0 or 1)."
llm_call_required: false   # the composed program decides its own LLM need
intent_examples:   ["(internal Matching-Mode driver — not user-routed)"]
rust_steps:        [tool_binding: ts-host-compose-orchestrator]
orchestrator_steps:[pc-host-compose-orchestrator, <composed-program-placeholder>]
step_descriptions: [{"step": 0, "action": "compose", "desc": "Fetch+split+assemble the matched recipe."}, {"step": 1, "action": "run", "desc": "Run the assembled orchestrator program (Monty, one continuous program)."}]
tier:              0
```

### Step 27.3 — `host.post_reply` (Phase 3 — answer post)

> **Capability:** `host.post_reply` · **Effect:** `write` · **Permission:** Auto
> Backs `__post_reply__(answer_text)`. Posts the final answer into the user chat.
> Called at the end of both Matching-Mode and Non-Matching-Mode.

#### Step 27.3.1 — Tool row (class 0)
```
name:            "host.post_reply"
description:     "Post the final answer text into the user chat. End-of-turn emit
                  for both Matching- and Non-Matching-Mode."
capability_id:   "host.post_reply"
effect_type:     "write"
param_schema: {
  "type": "object",
  "properties": {
    "answer": {"type": "string", "description": "The final answer to post"}
  },
  "required": ["answer"]
}
param_template:  {"answer": ""}
preconditions:   "Active chat session."
error_handling:  "Post failure → raise; caller retries."
consumer_tags:   ["00:rusty", "02:orchestrator"]
source:          "system"
validation_status: "validated"
```

#### Step 27.3.2 — ToolSkill: `ts-host-post-reply` (class 13)
```
name:          "ts-host-post-reply"
tool_name:     "host.post_reply"
description:   "Post the final answer into the user chat."
param_schema:  [
  {name: "answer", param_type: "string", required: true, description: "Final answer text"}
]
param_template: {"answer": "{{answer}}"}
preconditions:  "Active chat session."
error_handling: "Raise on post failure."
category:       "management"
source:         "system"
validation_status: "validated"
```

#### Step 27.3.3 — PythonCode: `pc-host-post-reply` (class 22)
```python
# Channel: orchestrator | Class: 22 | No I/O, no imports.
host.post_reply(answer="{{vars.slot0}}")
```

#### Step 27.3.4 — Leaf Skill: `skill-host-post-reply` (class 1)
```
name:        "skill-host-post-reply"
class_code:  1
description: "Leaf skill: how to post the final answer into the chat."
body: |
  Call `ts-host-post-reply` once with the final answer text after the turn's work
  is complete. This is the single end-of-turn emit for both modes. After posting,
  call the `host-save-history` recipe so kohai/sempai can mint new components.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

#### Step 27.3.5 — Recipe: `host-post-reply` (class 21, Tier 0)
```
name:              "host-post-reply"
description:       "Post the final answer into the user chat. Tier 0 — no LLM."
llm_call_required: false
intent_examples:   ["(internal end-of-turn emit — not user-routed)"]
rust_steps:        [tool_binding: ts-host-post-reply]
orchestrator_steps:[pc-host-post-reply]
step_descriptions: [{"step": 0, "action": "post_reply", "desc": "Post the final answer."}]
tier:              0
```

### Step 27.4 — `host-save-history` (Recipe over `builtin.memory_write` — no new Tool)

> **Not a new Tool.** `__save_history__(turn_summary)` is a **Recipe** that reuses
> `builtin.memory_write` (Step 11) to append a structured turn summary to the
> daily memory log, plus a PythonCode formatter leaf. This is the kohai/sempai
> input path: the saved history lets the self-improvement system mint new
> intents/skills/recipes/tools so the LLM is not needed next time.

#### Step 27.4.1 — PythonCode formatter: `pc-host-history-format` (class 22)
```python
# Channel: orchestrator | Class: 22 | No I/O, no imports, no network, no DB.
# Compose a structured turn-summary doc body from slot vars.
summary = {
  "user_input": "{{vars.slot0}}",
  "answer": "{{vars.slot1}}",
  "mode": "{{vars.slot2}}",
  "matched_component": "{{vars.slot3}}",
  "timestamp": "{{vars.slot4}}"
}
body = "## Turn summary\n"
for k, v in summary.items():
    body += f"- **{k}**: {v}\n"
# handed to the following memory_write step
```

#### Step 27.4.2 — Recipe: `host-save-history` (class 21, Tier 0)
```
name:              "host-save-history"
description:       "Save a structured turn summary to the daily memory log for kohai/sempai. Tier 0 — no LLM. Reuses builtin.memory_write."
llm_call_required: false
intent_examples:   ["(internal end-of-turn history save — not user-routed)"]
rust_steps:        [tool_binding: ts-memory-write]
orchestrator_steps:[pc-host-history-format, pc-memory-write]
step_descriptions: [{"step": 0, "action": "format", "desc": "Format the turn summary body."}, {"step": 1, "action": "memory_write", "desc": "Append the summary to the daily memory log."}]
tier:              0
# path convention: memory/turn-history/YYYY-MM-DD.log
```

### Step 27.5 — LLM invocation + retrieval + reduction — RETIRED / DROPPED (LOCKED)

> **No new Rust host-service Tools in this slot.** Under the locked architecture
> the LLM is **NOT a Rust Tool** — LLM invocation is **Kohai-mediated** (a Recipe,
> see 27.10), and the provider-LLM HTTP call reuses the **existing**
> `first_party_tools/http.rs` tool (see the 27.0 reused-Tools table). Prior
> knowledge is the **fallback Recipe** (27.10.1, used only when no prefix is
> present), not a retrieval verb.
>
> **RETIRED (no Tool row, no ToolSkill, no PythonCode, no Skill):**
> - `host.llm_complete` (+ `ts-host-llm-complete` / `pc-host-llm-complete` /
>   `skill-host-llm-complete`) — **RETIRED**. LLM invocation is Kohai-mediated.
>   The Rust backing `handle_llm_complete` / `LlmBackend` **retire**. Rust↔LLM
>   never communicate directly — only over the Orchestrator/Kohai.
>
> **DROPPED (no Tool row, no Recipe):**
> - `host.retrieve_docs` (+ `ts-host-retrieve-docs` / `pc-host-retrieve-docs`) —
>   **DROPPED**. Prior knowledge is the fallback Recipe (27.10.1), not a retrieval
>   verb.
> - `host.get_reduction_rules` (+ `ts-host-get-reduction-rules` /
>   `pc-host-get-reduction-rules`) — **DROPPED**. Same — prompt reduction is not a
>   host verb; if ever needed it is a Recipe step.
>
> The Kohai-mediated LLM flow (Orchestrator composes prompt + placeholder → Kohai
> saves → optional Sempai optimize → Kohai adds provider prefix → Kohai calls
> `first_party_tools/http` → Kohai saves answer → answer back to Orchestrator) is
> authored in **Step 27.10.2** (`host-non-match-llm-answer`) and the Tier-1
> LLM-guided flow in 27.10. See the 27.0 table + CLAUDE.md "LLM invocation =
> Kohai-mediated".

### Step 27.6 — Dispatch meta-primitives + action inventory — RETIRED except the parallel helper (LOCKED)

> Under the locked architecture tools are **first-class callables in the Monty
> namespace** — a recipe's PythonCode calls `host.<name>(…)` directly (see the
> 27.0 table + intro). So the string-intrinsic dispatchers and the per-step
> sandbox executor are **RETIRED**, and the action-inventory tool is **RETIRED**
> (the bound namespace already exposes the callable tools). Only the
> **parallel-dispatch helper** survives, as a Python helper (not a Rust intrinsic).
>
> **RETIRED (no Tool row, no intrinsic, no component):**
> - `__execute_action__(name, params, call_id=…)` — **RETIRED**. Tools are called
>   directly as first-class callables; there is no string-name dispatch intrinsic.
>   The old Rust `handle_execute_action` policy/lease/gate/event wrapper is also
>   retired as a universal per-call babysitter — security is now mode-driven
>   (Matching-Mode all-off / Non-Matching-Mode wrapper-on; see CLAUDE.md).
> - `__execute_code_step__(code, state)` — **RETIRED** (Model-A per-step relic).
>   Monty runs a recipe's PythonCode as one continuous program; per-step sandbox
>   re-entry is not needed.
> - `host.get_actions` (+ `ts-host-get-actions` / `pc-host-get-actions` /
>   `skill-host-get-actions`) — **RETIRED**. With first-class callables + the
>   namespace registry, the Orchestrator already has its bound tools; enumerating
>   a callable inventory via a host verb is not needed. (A future MCP bridge
>   advertises tools from the same namespace registry — no `get_actions` verb
>   required.)
>
> **KEPT — Python helper (not a Rust intrinsic):**

#### Step 27.6.1 — `pc-host-execute-parallel` PythonCode helper (class 22)
```python
# Channel: orchestrator | Class: 22 | No I/O, no imports.
# Thin convenience helper: call several host.* first-class callables concurrently.
# {{vars.slot0}} = a list of {name, args} dicts (IBS-baked before execution).
# Each entry dispatches to the bound callable in the Monty namespace.
results = __execute_actions_parallel__({{vars.slot0}})
```

> Implementation note: `__execute_actions_parallel__` is a small Python helper
> that fans out to the bound `host.*` callables concurrently and gathers their
> results — it is no longer a Rust meta-primitive routing through
> `__execute_action__`. If a shared gate/lease batch ever proves cheaper in Rust,
> a thin Rust intrinsic can be added later as a new Tool; until then it stays
> Python-side.

### Step 27.7 — Component-store host calls (SEC-01 fetch + validation queue)

> Three host calls over the component store (LOCKED): `host.fetch_component` +
> `host.resolve_component_by_name` (SEC-01 validated fetch by UUID / by name — the
> §0.9 Option A/B lookups `call_action` uses), and `host.validate_component`
> (intercepts self-improvement writes for protected components → Q1 pending
> update-candidate; kohai/sempai input path). Each is one Tool row (class 0) + one
> ToolSkill (class 13) + one PythonCode (class 22) + one leaf Skill (class 1).
> The Rust `handle_*` backing fns stay as the impl.

#### Step 27.7.1 — `host.record_skill_usage` (telemetry) — RETIRED (LOCKED Q-D)

> **RETIRED.** The Orchestrator owns its own run; skill-usage telemetry is not a
> runtime host verb. No Tool row, no ToolSkill, no PythonCode, no Skill. (If
> skill-usage tracking is ever needed it becomes a Recipe step, not a Rust host
> tool.) The KEPT component-store host calls are 27.7.2 `host.fetch_component`,
> 27.7.3 `host.resolve_component_by_name`, 27.7.4 `host.validate_component`.

#### Step 27.7.2 — `host.fetch_component` (SEC-01 fetch by UUID)
```
# Tool row (class 0)
name:              "host.fetch_component"
capability_id:     "host.fetch_component"
effect_type:       "read"
description:       "Fetch a single validated component by UUID + class code (SEC-01
                    gate). Returns {id,class_code,name,description,content,
                    override_prompt_creation,steps?,allowed_tools?} or null."
param_schema:      {"type":"object","properties":{
                      "uuid":       {"type":"string","description":"Component UUID"},
                      "class_code": {"type":"integer","description":"Component class code"}
                    },"required":["uuid","class_code"]}
param_template:    {"uuid":"","class_code":0}
preconditions:     "skills-db pool wired (returns null without it)."
error_handling:    "Missing/invalid/absent → null; never raises."
consumer_tags:     ["00:rusty","02:orchestrator"]
source:            "system"
validation_status: "validated"

# ToolSkill (class 13)
name:              "ts-host-fetch-component"
tool_name:         "host.fetch_component"
param_schema:      [{name:"uuid",param_type:"string",required:true},
                    {name:"class_code",param_type:"number",required:true}]
param_template:    {"uuid":"{{uuid}}","class_code":{{class_code}}}
category:          "management"

# PythonCode (class 22)
comp = host.fetch_component(uuid="{{vars.slot0}}", class_code={{vars.slot1}})

# Leaf Skill (class 1)
name:              "skill-host-fetch-component"
description:       "Fetch a validated component by UUID for nested call_action lookups (§0.9 Option A)."
body:              "Use ts-host-fetch-component when you hold a component UUID + class code. A null result means the component is absent or not validated — do not invent one."
```

#### Step 27.7.3 — `host.resolve_component_by_name` (SEC-01 fetch by name — §0.9 Option B)
```
# Tool row (class 0)
name:              "host.resolve_component_by_name"
capability_id:     "host.resolve_component_by_name"
effect_type:       "read"
description:       "Fetch a single validated component by NAME + class code (SEC-01
                    gate) — the §0.9 Option B fallback when only a step name is
                    held. Same dict shape as host.fetch_component, or null."
param_schema:      {"type":"object","properties":{
                      "name":       {"type":"string","description":"Component name"},
                      "class_code": {"type":"integer","description":"Component class code"}
                    },"required":["name","class_code"]}
param_template:    {"name":"","class_code":0}
preconditions:     "skills-db pool wired (returns null without it)."
error_handling:    "Missing/invalid/absent → null; never raises."
consumer_tags:     ["00:rusty","02:orchestrator"]
source:            "system"
validation_status: "validated"

# ToolSkill (class 13)
name:              "ts-host-resolve-component-by-name"
tool_name:         "host.resolve_component_by_name"
param_schema:      [{name:"name",param_type:"string",required:true},
                    {name:"class_code",param_type:"number",required:true}]
param_template:    {"name":"{{name}}","class_code":{{class_code}}}
category:          "management"

# PythonCode (class 22)
comp = host.resolve_component_by_name(name="{{vars.slot0}}", class_code={{vars.slot1}})

# Leaf Skill (class 1)
name:              "skill-host-resolve-component-by-name"
description:       "Resolve a component by name (§0.9 Option B) when only a step name is held."
body:              "Use ts-host-resolve-component-by-name when call_action holds a step name, not a UUID. Same null-means-absent contract as fetch_component."
```

#### Step 27.7.4 — `host.validate_component` (kohai/sempai → Q1 pending queue)
```
# Tool row (class 0)
name:              "host.validate_component"
capability_id:     "host.validate_component"
effect_type:       "write"
description:       "Intercept a self-improvement component write. Protected titles
                    (orchestrator:main, prompt:codeact_preamble) become a Q1
                    pending update-candidate (llm_audit_required for class 10/50)
                    instead of a direct write. Returns {queued, reason?, candidate_id?}."
param_schema:      {"type":"object","properties":{
                      "title":    {"type":"string","description":"Component title"},
                      "content":  {"type":"string","description":"Proposed component content"},
                      "doc_type": {"type":"string","description":"skill|recipe|tool_skill|lesson|spec|plan|note"},
                      "metadata": {"type":"object","description":"Extra metadata (non-overriding on validation fields)"}
                    },"required":["title","content"]}
param_template:    {"title":"","content":"","doc_type":"note","metadata":{}}
preconditions:     "Store wired."
error_handling:    "Empty payload → {queued:false,reason:'empty payload'}; no store → {queued:false,reason:'no_store'}."
consumer_tags:     ["00:rusty","02:orchestrator","05:validator"]
source:            "system"
validation_status: "validated"

# ToolSkill (class 13)
name:              "ts-host-validate-component"
tool_name:         "host.validate_component"
param_schema:      [{name:"title",param_type:"string",required:true},
                    {name:"content",param_type:"string",required:true},
                    {name:"doc_type",param_type:"string",required:false},
                    {name:"metadata",param_type:"object",required:false}]
param_template:    {"title":"{{title}}","content":"{{content}}","doc_type":"{{doc_type}}","metadata":{{metadata}}}
category:          "management"

# PythonCode (class 22)
res = host.validate_component(title="{{vars.slot0}}", content="{{vars.slot1}}", doc_type="{{vars.slot2}}", metadata={{vars.slot3}})

# Leaf Skill (class 1)
name:              "skill-host-validate-component"
description:       "Route a self-improvement component proposal into the validation queue."
body:              "When the kohai/sempai system proposes a new/updated component, call ts-host-validate-component with title, content, doc_type, and any extra metadata. Inspect `queued`; protected components go to Q1 pending with an LLM-audit gate before Q2 manual validation."
```

### Step 27.8 — Reuse unification: `host.regex_match` + `__list_skills__`

> Two host calls are **not** new logic — they reuse existing components. This
> step pins the unification so the host-call surface and the user-facing builtin
> tools share ONE Rust backing implementation (no duplicated behavior).

#### Step 27.8.1 — `host.regex_match` over the `pc-regex-match` backing (Step 20.x.2)
```
# `__regex_match__(pattern, text) -> bool` already has a backing fn
# (handle_regex_match) and a PythonCode (pc-regex-match, Step 20.x.2) used by the
# skill selector. For host-call uniformity we expose a thin Tool row that routes
# to the SAME handle_regex_match — no new logic, no new Rust fn.
#
# Tool row (class 0)
name:              "host.regex_match"
capability_id:     "host.regex_match"
effect_type:       "read"
description:       "Evaluate a regex against text (linear-time regex crate; invalid
                    pattern → False). Same backing as pc-regex-match (Step 20.x.2)."
param_schema:      {"type":"object","properties":{
                      "pattern": {"type":"string","description":"Regex pattern"},
                      "text":    {"type":"string","description":"Text to test"}
                    },"required":["pattern","text"]}
param_template:    {"pattern":"","text":""}
preconditions:     "None."
error_handling:    "Invalid pattern / size-limit → False; never raises."
consumer_tags:     ["00:rusty","02:orchestrator"]
source:            "system"
validation_status: "validated"

# ToolSkill (class 13)
name:              "ts-host-regex-match"
tool_name:         "host.regex_match"
param_schema:      [{name:"pattern",param_type:"string",required:true},
                    {name:"text",param_type:"string",required:true}]
param_template:    {"pattern":"{{pattern}}","text":"{{text}}"}
category:          "utility"

# PythonCode (class 22) — delegates to __regex_match__ (one backing)
ok = host.regex_match(pattern="{{vars.slot0}}", text="{{vars.slot1}}")
```

#### Step 27.8.2 — `__list_skills__` reuses `builtin.skill_list` (Step 16) — no new Tool
```
# `__list_skills__(max_candidates, max_tokens)` and the user-facing
# builtin.skill_list (Step 16) MUST share one Rust backing implementation
# (handle_list_skills). No new host.* Tool row is created: the host call and the
# builtin tool are the same capability. Recipes that need the skill catalogue call
# __execute_action__("builtin.skill_list", {max_candidates, max_tokens}) — the
# identical surface Monty and the future MCP bridge use. The only work here is
# implementation hygiene: ensure builtin.skill_list and __list_skills__ route to
# the same handler (they already do in orchestrator.rs); no component row is added.
```

### Step 27.9 — VM-control + telemetry host calls

> One KEPT host call (LOCKED): `host.check_signals` (stop/suspend/inject poll) —
> the only VM-control verb that survives, because external signals (stop/suspend/
> inject) arrive asynchronously from outside the Orchestrator's own step
> sequence. It is one Tool row (class 0) + ToolSkill (class 13) + PythonCode
> (class 22) + leaf Skill (class 1); the Rust `handle_*` backing fn stays as the
> impl.
>
> **RETIRED (LOCKED Q-D — the Orchestrator owns thread state):** `host.emit_event`
> (chat event emission goes via `host.post_reply`), `host.save_checkpoint`,
> `host.transition_to`, `host.check_budget`, `host.log_budget_warning`. The
> agent-loop stage pipeline is no longer the driver, and no universal per-call
> wrapper means no per-step state/budget/event host verbs are needed — the
> Orchestrator inherently knows where it is in its own step sequence. No Tool rows,
> no ToolSkills, no PythonCode, no Skills for these five.

#### Step 27.9.1 — `host.check_signals` (stop/suspend/inject poll)
```
# Tool row (class 0)
name:              "host.check_signals"
capability_id:     "host.check_signals"
effect_type:       "read"
description:       "Poll the thread signal channel. Returns 'stop' on stop/suspend,
                    {inject: msg} on an injected message, or None when clear."
param_schema:      {"type":"object","properties":{},"required":[]}
param_template:    {}
preconditions:     "Signal receiver wired."
error_handling:    "No signal → None; never raises."
consumer_tags:     ["00:rusty","02:orchestrator"]
source:            "system"
validation_status: "validated"

# ToolSkill (class 13)
name:              "ts-host-check-signals"
tool_name:         "host.check_signals"
param_schema:      []
param_template:    {}
category:          "management"

# PythonCode (class 22)
sig = host.check_signals()

# Leaf Skill (class 1)
name:              "skill-host-check-signals"
description:       "Poll for stop/suspend/inject signals between steps."
body:              "Call ts-host-check-signals between orchestrator steps. On 'stop', halt cleanly. On {inject: msg}, fold the message in and continue. On None, proceed."
```

#### Step 27.9.2 — `host.emit_event` — RETIRED (LOCKED Q-D)

> **RETIRED.** Chat event emission goes through `host.post_reply` (the
> Orchestrator posts the final answer; Rust owns the chat socket). No Tool row,
> no ToolSkill, no PythonCode, no Skill. The `handle_emit_event` Rust backing fn
> is reused only by `post_reply` / the event broadcast path — not exposed as a
> host verb.

#### Step 27.9.3 — `host.save_checkpoint` — RETIRED (LOCKED Q-D)

> **RETIRED.** The Orchestrator owns its own run state — it inherently knows
> where it is in its own step sequence; cross-turn resumption (D-C1) is handled
> by the cross-turn persistent Monty session, not a per-step checkpoint verb. No
> Tool row, no ToolSkill, no PythonCode, no Skill.

#### Step 27.9.4 — `host.transition_to` — RETIRED (LOCKED Q-D)

> **RETIRED.** There is no Rust stage-pipeline state machine to drive any more;
> the Orchestrator is the sole sequencer and already knows its phase. No Tool
> row, no ToolSkill, no PythonCode, no Skill.

#### Step 27.9.5 — `host.check_budget` — RETIRED (LOCKED Q-D)

> **RETIRED.** Budget hard-stops are enforced where the Orchestrator decides
> (it tracks its own counters); there is no universal per-call wrapper needing a
> budget-read verb. No Tool row, no ToolSkill, no PythonCode, no Skill.

#### Step 27.9.6 — `host.log_budget_warning` — RETIRED (LOCKED Q-D)

> **RETIRED.** Soft budget telemetry is not a runtime host verb; if a soft
> warning is ever needed it is a Recipe step, not a Rust host tool. No Tool row,
> no ToolSkill, no PythonCode, no Skill.

### Step 27.10 — Recipes: prior-knowledge assembly + Non-Matching-Mode LLM answer

> Two **recipe-only** components (class 21) composed over the `host.*` tools — no
> new Tool rows except the one **Orchestrator→Kohai handoff** tool
> `host.kohai_complete` (wraps the existing `brassclaw_interceptor` ingress; no new
> logic — wiring, like `host.resolve_intent`; added to the 27.0 table below).
> `host-assemble-prior-knowledge` is the **fallback** prior-knowledge bundle, used
> **only when no prefix is present**: it adds basic "what is going on" context so
> the LLM understands; it calls **no retrieval verbs** (`retrieve_docs` /
> `get_reduction_rules` are dropped). `host-non-match-llm-answer` is the **Tier-2
> Non-Matching-Mode** routine: assemble the prompt (chat history + user question +
> a prefix-placeholder) → hand to **Kohai** (saves; optional Sempai optimize; adds
> the provider prefix; calls `first_party_tools/http`; saves the answer) → answer
> back. **No `host.llm_complete`**. Both are instruction-driven, so prompt
> additions / prefixes / query-type routing evolve with **no code changes** — only
> the recipe is altered.

#### Step 27.10.1 — Recipe `host-assemble-prior-knowledge` (class 21, Tier 1 — fallback)
```
name:              "host-assemble-prior-knowledge"
description:       "FALLBACK prior-knowledge bundle, used ONLY when no prefix is
                    present. Adds basic 'what is going on' context so the LLM
                    understands the run. Calls NO retrieval verbs (retrieve_docs /
                    get_reduction_rules are dropped). Recipe-only — one PythonCode
                    formatter, no tool bindings."
llm_call_required: false   # builds the bundle; the caller does the (Kohai-mediated) LLM call
intent_examples:   ["(internal Tier-1 prior-knowledge fallback — not user-routed)"]
rust_steps:        []
orchestrator_steps:[pc-host-fallback-prior-knowledge]
step_descriptions: [
  {"step":0,"action":"fallback_context","desc":"Add basic 'what is going on' context so the LLM understands (no retrieval)."}
]
tier:              1

# pc-host-fallback-prior-knowledge (class 22) — pure-logic formatter, no host call:
#   user_query = {{vars.slot0}}
#   bundle = {
#     "context": "You are running inside BrassClaw's orchestrator. Answer the user's request.",
#     "user_query": user_query,
#     "assembled_at": __now__()
#   }
# (No retrieve_docs / get_reduction_rules — those are dropped. This bundle is only
#  used when the caller has no precompiled prefix.)
```

#### Step 27.10.2 — Recipe `host-non-match-llm-answer` (class 21, Tier 2 — Non-Matching-Mode, Kohai-mediated)
```
name:              "host-non-match-llm-answer"
description:       "Non-Matching-Mode (Tier 2): no intent matched. The Orchestrator
                    assembles the standard prompt (chat history + user question + a
                    prefix-PLACEHOLDER) and hands it to Kohai. Kohai saves the prompt;
                    if a Sempai is connected, Kohai adds an optimization-prefix →
                    Sempai optimizes → returns without prefix → Kohai saves the
                    optimized prompt beside the original; Kohai adds the provider-LLM
                    prefix (the one for that placeholder) and sends the prompt to the
                    provider LLM by calling first_party_tools/http; Kohai receives the
                    answer, saves it beside its prompt, and returns it to the
                    Orchestrator. NO host.llm_complete. Recipe-driven so prompt
                    additions / prefixes / query-type routing evolve with no code
                    changes."
llm_call_required: true
intent_examples:   ["(internal Non-Matching-Mode fallback — not user-routed)"]
rust_steps:        [tool_binding: ts-host-kohai-complete]
orchestrator_steps:[pc-host-assemble-non-match-prompt,
                    pc-host-kohai-complete]
step_descriptions: [
  {"step":0,"action":"assemble_prompt","desc":"Assemble chat history + user question + a prefix-PLACEHOLDER into the prompt (kohai swaps the placeholder for the provider prefix last)."},
  {"step":1,"action":"kohai_complete","desc":"Hand the assembled prompt to Kohai via host.kohai_complete; Kohai saves, optional Sempai optimize, adds provider prefix, calls first_party_tools/http, saves the answer, and returns it."}
]
tier:              2

# pc-host-assemble-non-match-prompt (class 22) — pure-logic assembler, no host call:
#   chat_history = {{vars.slot0}}      # few tokens, belongs to this exact user-input
#   user_query   = {{vars.slot1}}      # few tokens
#   placeholder  = {{vars.slot2}}      # prefix-PLACEHOLDER (kohai swaps it last)
#   prompt = {"chat_history": chat_history, "user_query": user_query, "prefix_placeholder": placeholder}
# (The composition system precompiles the prefix chunks indexed by placeholder, but
#  does NOT bake them into the prompt — Kohai swaps the placeholder for the provider
#  prefix. IBS binds chat_history/user_query/placeholder into slots 0/1/2.)

# pc-host-kohai-complete (class 22) — the Orchestrator→Kohai handoff (first-class call):
#   prompt = <result of pc-host-assemble-non-match-prompt>
#   answer = host.kohai_complete(prompt=prompt)
# (host.kohai_complete wraps the existing brassclaw_interceptor ingress; Kohai does
#  the save / optional-Sempai / provider-prefix / first_party_tools/http / save-answer
#  dance and returns the answer. No host.llm_complete — Rust↔LLM never talk directly.)
```

#### Step 27.10.3 — `host.kohai_complete` (Orchestrator→Kohai handoff Tool)
```
# Tool row (class 0)
name:              "host.kohai_complete"
capability_id:     "host.kohai_complete"
effect_type:       "write"
description:       "Hand an assembled LLM prompt (with a prefix-placeholder) to Kohai.
                    Kohai saves the prompt; if a Sempai is connected, adds an
                    optimization-prefix → Sempai optimizes → returns without prefix →
                    Kohai saves the optimized prompt beside the original; Kohai adds
                    the provider-LLM prefix for that placeholder and sends the prompt
                    to the provider LLM by calling first_party_tools/http; receives the
                    answer, saves it beside its prompt, and returns it. Wraps the
                    existing brassclaw_interceptor ingress — no new logic, wiring only."
param_schema:      {"type":"object","properties":{
                      "prompt": {"type":"object","description":"Assembled prompt {chat_history, user_query, prefix_placeholder}"}
                    },"required":["prompt"]}
param_template:    {"prompt":{}}
preconditions:     "Interceptor (Kohai) ingress wired; provider-LLM prefix chunk precompiled for the placeholder."
error_handling:    "Provider/HTTP failure → raises; Orchestrator catches and surfaces via post_reply."
consumer_tags:     ["00:rusty","02:orchestrator"]
source:            "system"
validation_status: "validated"

# ToolSkill (class 13)
name:              "ts-host-kohai-complete"
tool_name:         "host.kohai_complete"
param_schema:      [{name:"prompt",param_type:"object",required:true}]
param_template:    {"prompt":{{prompt}}}
category:          "llm"

# PythonCode (class 22) — first-class callable (no __execute_action__):
answer = host.kohai_complete(prompt=prompt)

# Leaf Skill (class 1)
name:              "skill-host-kohai-complete"
description:       "Hand an assembled prompt to Kohai and await the provider-LLM answer."
body:              "Call host.kohai_complete with the assembled prompt (chat history + user query + a prefix-placeholder). Kohai saves it, optionally Sempai-optimizes it, swaps the placeholder for the provider prefix, calls the provider LLM via first_party_tools/http, saves the answer, and returns it. Use this for every Orchestrator-side LLM call — Rust never talks to the LLM directly."
```

> **Why a Tool, not a Recipe step:** the interceptor ingress is a Rust mechanism
> (the `brassclaw_interceptor` crate owns Kohai/Sempai mode/packet/proposal_sink/
> pg_store/config). Per the locked architecture every Rust mechanism the
> Orchestrator needs is a host Tool — so the handoff is `host.kohai_complete`,
> wiring the existing ingress (like `host.resolve_intent` wires the existing intent
> system). The *prompt-assembly* before it is the Recipe; the *provider-HTTP call*
> inside Kohai reuses `first_party_tools/http`. No `host.llm_complete`.

### Step 27.11 — ExtensionCatalogue: `builtin-host` (class 23)

> Owns the **orchestrator infrastructure** components — the `host.*` Tools,
> ToolSkills, PythonCode snippets, leaf skills, and internal Recipes that Monty
> and the composition system use to run the main process. Tools are **first-class
> callables in the Monty namespace** (recipe PythonCode calls `host.<name>(…)`
> directly — no `__execute_action__`); this is the catalogue a future MCP bridge
> enumerates to advertise host capabilities from the **same namespace registry**.
> It is `source: "system"`, `validation_status: "validated"` — first-party, not
> user-editable.

```
name:         "builtin-host"
class_code:   23
overview_doc: |
  # Orchestrator Host Capabilities

  The host domain covers the `host.*` service Tools the Monty main process calls
  as first-class namespace callables, plus the internal main-process Recipes.
  These are first-party system components; the old `__host_call__` 23-arm match
  and the `__execute_action__` string-intrinsic are RETIRED into this registry.

  ## Tools in this domain (class 0) — 8 net new
  - host.resolve_intent            — Phase 2 intent match (whole intent system = one Tool)
  - host.compose_orchestrator      — Phase 3 Matching-Mode fetch+split+assemble (REWRITE)
  - host.post_reply                — end-of-turn answer post (Rust owns the chat socket)
  - host.fetch_component           — SEC-01 fetch by UUID (§0.9 Option A)
  - host.resolve_component_by_name — SEC-01 fetch by name (§0.9 Option B)
  - host.validate_component        — kohai/sempai → Q1 pending queue
  - host.check_signals             — stop/suspend/inject poll (the only VM-control verb kept)
  - host.kohai_complete            — Orchestrator→Kohai handoff (wraps brassclaw_interceptor ingress)

  ## Reused existing Tools (no new row, unified backing)
  - builtin.memory_write (Step 11) — shared SQL saver (Kohai persist + host-save-history)
  - first_party_tools/http (Step 19) — Kohai→provider-LLM HTTP transport
  - builtin.skill_list (Step 16) — __list_skills__ routes to the same handler
  - pc-regex-match (Step 20.x.2) — host.regex_match routes to the same handler

  ## Internal Recipes (class 21 — not user-routed)
  - host-resolve-intent                  — Phase 2 (over host.resolve_intent)
  - host-compose-and-run-orchestrator    — Phase 3 Matching-Mode (over host.compose_orchestrator)
  - host-post-reply                      — Phase 3 end-of-turn (over host.post_reply)
  - host-save-history                    — history save (over builtin.memory_write)
  - host-assemble-prior-knowledge        — Tier-1 FALLBACK prior-knowledge (no retrieval verbs)
  - host-non-match-llm-answer            — Tier-2 Non-Matching-Mode (Kohai-mediated, over host.kohai_complete)

  ## RETIRED / DROPPED (no component)
  - host.llm_complete (+ handle_llm_complete / LlmBackend) — RETIRED (Kohai-mediated)
  - host.retrieve_docs / host.get_reduction_rules — DROPPED (prior knowledge = fallback Recipe)
  - host.get_actions / host.record_skill_usage — RETIRED (Q-D: Orchestrator owns its run)
  - host.emit_event / host.save_checkpoint / host.transition_to / host.check_budget / host.log_budget_warning — RETIRED (Q-D: Orchestrator owns thread state; chat via host.post_reply)
  - __execute_action__ / __execute_code_step__ — RETIRED meta-primitives (first-class callables)
  - __execute_actions_parallel__ — KEPT as the Python helper pc-host-execute-parallel (not a Rust intrinsic)

task_groups:
  - group_name:  "main-process-control"
    description: "Phase 2/3 of the basic-mode main process: intent, compose, post, history"
  - group_name:  "llm-via-kohai"
    description: "Orchestrator→Kohai handoff (host.kohai_complete) + Non-Matching-Mode + fallback prior-knowledge"
  - group_name:  "component-store"
    description: "SEC-01 fetch (UUID/name), validation queue"
  - group_name:  "vm-control"
    description: "Signal poll (the only VM-control verb kept)"

child_component_ids: [
  "<uuid:host.resolve_intent>",            "<uuid:ts-host-resolve-intent>",
  "<uuid:pc-host-resolve-intent>",         "<uuid:skill-host-resolve-intent>",
  "<uuid:host-resolve-intent recipe>",
  "<uuid:host.compose_orchestrator>",      "<uuid:ts-host-compose-orchestrator>",
  "<uuid:pc-host-compose-orchestrator>",   "<uuid:skill-host-compose-orchestrator>",
  "<uuid:host-compose-and-run-orchestrator recipe>",
  "<uuid:host.post_reply>",                "<uuid:ts-host-post-reply>",
  "<uuid:pc-host-post-reply>",             "<uuid:skill-host-post-reply>",
  "<uuid:host-post-reply recipe>",
  "<uuid:host-save-history recipe>",
  "<uuid:host.kohai_complete>",            "<uuid:ts-host-kohai-complete>",
  "<uuid:pc-host-kohai-complete>",         "<uuid:skill-host-kohai-complete>",
  "<uuid:host.fetch_component>",           "<uuid:ts-host-fetch-component>",
  "<uuid:pc-host-fetch-component>",        "<uuid:skill-host-fetch-component>",
  "<uuid:host.resolve_component_by_name>", "<uuid:ts-host-resolve-component-by-name>",
  "<uuid:pc-host-resolve-component-by-name>","<uuid:skill-host-resolve-component-by-name>",
  "<uuid:host.validate_component>",        "<uuid:ts-host-validate-component>",
  "<uuid:pc-host-validate-component>",     "<uuid:skill-host-validate-component>",
  "<uuid:host.regex_match>",               "<uuid:ts-host-regex-match>",
  "<uuid:pc-host-regex-match>",
  "<uuid:host.check_signals>",             "<uuid:ts-host-check-signals>",
  "<uuid:pc-host-check-signals>",          "<uuid:skill-host-check-signals>",
  "<uuid:pc-host-execute-parallel>",
  "<uuid:host-assemble-prior-knowledge recipe>",
  "<uuid:host-non-match-llm-answer recipe>"
]
```

> **Step 27 complete.** The 23 old host calls are dissected into the v3 component
> vocabulary under the locked Orchestrator/Executioner classification: **8 net new
> `host.*` Tool rows** (resolve_intent, compose_orchestrator [rewrite], post_reply,
> fetch_component, resolve_component_by_name, validate_component, check_signals,
> kohai_complete), **4 reused existing** (builtin.memory_write, first_party_tools/
> http, builtin.skill_list, pc-regex-match), **3 Recipes** (host-non-match-llm-answer
> [Kohai-mediated], host-assemble-prior-knowledge [fallback], host-save-history),
> **1 Python helper** (pc-host-execute-parallel), **DROPPED** retrieve_docs +
> get_reduction_rules, and **RETIRED** host.llm_complete (+ handle_llm_complete /
> LlmBackend) + 7 stage-machinery verbs (Q-D) + __execute_action__ /
> __execute_code_step__. Tools are first-class namespace callables (no
> `__execute_action__`); the future MCP bridge advertises the same registry. Phase
> C Rust implementation (C.1 tool registry + first-class callables, C.2 reclassify
> host calls, C.3 cdylib dynamic loading, C.4 mode-driven security + WebUI panel,
> C.5 basic-mode orchestrator script, C.6 production driver switch, C.7 retire dead
> Model-A code + both configs green) follows this spec.
