# Built-in Functionality — v3 Artifact Plan (Revised)

> **Purpose:** For every built-in first-party capability this document defines the exact v3
> artifacts: class-0 Tools (full DB row spec), class-13 ToolSkills (executor-facing only),
> class-22 PythonCode (pure logic, no I/O), class-1–3 Skills (leaf + domain, orchestrator-facing),
> class-21 Recipes (with `step_descriptions` JSONB + intent examples), class-16 Actions
> (deterministic no-LLM procedures), and class-23 ExtensionCatalogues (five, one per domain).
>
> **Review corrections applied:** F-01 PythonCode I/O removed; F-02 Tool rows fully specified;
> F-03 five catalogues instead of one; F-04 leaf Skills added per tool; F-05 ToolSkill bodies
> executor-only; F-07 shell/subagent Tier-1 enforced; F-08 step_descriptions JSONB per recipe;
> F-09 tool_name = Tool row name not capability ID; F-10 Q1/Q2 bypass claim corrected;
> F-11 LLM call as type:llm step; F-12 Action artifacts added; F-13 intent_examples on all recipes.
>
> **Prerequisite phases:** A–C (V050–V053), L (V057 adds `source='system'` to older tables).
> `reborn_python_code` (V052) and `reborn_extension_catalogues` (V053) must exist before seeding.
>
> **Architecture rules enforced throughout:**
> - Tool (class 0): full DB row, opaque to LLM/orchestrator, `effect_type` drives approval gate.
> - ToolSkill (class 13): executor-facing only — param schema + Rust-level preconditions/errors.
>   The orchestrator **never** reads ToolSkill bodies.
> - Skill (class 1–3): orchestrator-facing narrative. Two grains: **leaf** (one tool) and
>   **domain** (references leaves by name, no duplication).
> - PythonCode (class 22): pure logic, no I/O, no DB, no network, no system clock.
> - Recipe (class 21): composition of leaves via `step_descriptions` JSONB.
>   `type:"component"` steps include leaf UUIDs; `type:"llm"` steps call the LLM.
> - Action (class 16): deterministic, no LLM, `execute_action_procedure` path.
> - ExtensionCatalogue (class 23): `overview_doc` + `task_groups`. Never re-documents children.
>
> **Q1/Q2 gate:** Q1 runs inline in the seeder (`builtin_bootstrap.rs`) — errors are
> build-time bugs. Q2 is automated-but-auditable via the validation-system extension
> (Phase P.0). There is **no `source='system'` Q2 bypass** — `source` is provenance only.
>
> **No code changes are made by this document.**

---

## Step 1 — `builtin.shell` (Shell Command Execution)

> **Capability:** `builtin.shell` · **Effect:** `mixed` · **Permission:** Ask
> **Timeout:** 120 s wall clock · **Output cap:** 1 MiB inline, overflow saved to scoped file
> **§shell-guard:** any Recipe referencing this tool is `llm_call_required: true` — **never Tier 0**.

---

### Step 1.1 — Tool row (class 0)

```
name:            "shell"
description:     "Execute a shell command or script in the sandboxed process executor.
                  Returns {output, exit_code, success, sandboxed}. When stdout+stderr
                  exceeds the inline cap, the full output is saved to a scoped
                  workspace file and the response body contains the saved path."
capability_id:   "builtin.shell"
effect_type:     "mixed"
param_schema:    {
  "type": "object",
  "properties": {
    "command":      {"type": "string", "description": "Shell command or multi-line script body"},
    "workdir":      {"type": "string", "description": "Working directory (must be a backed scoped path)"},
    "timeout_secs": {"type": "number", "description": "Wall-clock timeout, max 120"},
    "extra_env":    {"type": "object", "description": "Additional environment variables"}
  },
  "required": ["command"]
}
param_template:  {"command": "", "workdir": null, "timeout_secs": null}
preconditions:   ""
error_handling:  ""
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 1.2 — ToolSkill: `ts-shell-run` (class 13)

> Executor-facing only. The orchestrator never reads this body.

```
name:          "ts-shell-run"
tool_name:     "shell"
description:   "Run a shell command via builtin.shell. Accepts command, optional
                workdir (must be a backed scoped path), optional timeout_secs
                (1–120). Returns {output, exit_code, success, sandboxed}."
param_schema:  [
  {name: "command",      param_type: "string",  required: true,
   description: "Shell command or multi-line script"},
  {name: "workdir",      param_type: "string",  required: false,
   description: "Backed scoped working directory path"},
  {name: "timeout_secs", param_type: "number",  required: false,
   description: "Timeout in seconds, max 120"}
]
param_template: {"command": "{{command}}"}
preconditions:  "No interactive TTY. workdir must be a mount-backed path with
                 execute permission. Unbacked scoped paths are rejected."
error_handling: "exit_code != 0: surface to orchestrator for decision.
                 output contains saved_file path: orchestrator must call
                 read_file to retrieve full content.
                 RuntimeDispatchErrorKind::Resource: timeout exceeded."
code_snippet:   null
category:       "process"
source:         "system"
validation_status: "validated"
```

---

### Step 1.3 — Leaf Skill: `skill-shell-run` (class 1)

> Orchestrator-facing narrative. One tool — `builtin.shell` — one concern.

```
name:        "skill-shell-run"
class_code:  1
description: "Leaf skill: how to drive the executor to run a single shell command."
body: |
  Use the `ts-shell-run` ToolSkill when you need to execute one shell command.
  Pass the command string verbatim; do NOT construct it from unvalidated user
  input. Check `success` in the result; a false value means the command returned
  a non-zero exit code — inspect `output` for details and decide whether to retry,
  report, or continue.
  When the result contains a file path (large output was saved), call
  `skill-read-file` on that path to retrieve the content before proceeding.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

---

### Step 1.4 — Domain Skill: `skill-shell` (class 1)

> References leaf skills by name. Does not re-describe tool params.

```
name:        "skill-shell"
class_code:  1
description: "Domain skill: when and how to use shell execution safely."
body: |
  Shell execution is the most powerful and most dangerous builtin. Use it only
  when no higher-level tool covers the need (prefer `skill-read-file`,
  `skill-list-dir`, `skill-glob`, `skill-grep`, or `skill-apply-patch` for
  filesystem work; prefer `skill-http-fetch` for network work).

  How to run a command → `skill-shell-run`.

  Security rules (Q1-enforced):
  - Never pass user-supplied strings directly into the command without escaping.
  - Never run a command that modifies security-critical system files.
  - When output may exceed 1 MiB, add output-limiting flags (e.g. `head -n 200`);
    otherwise use the saved-file path returned in the response plus `skill-read-file`.

  Approval: `builtin.shell` requires user approval (PermissionMode::Ask).
  Every recipe that uses this skill is Tier 1 (`llm_call_required: true`).
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

---

### Step 1.5 — Recipe: `shell-run` (class 21)

> **Tier:** 1 (`llm_call_required: true`) — §shell-guard hard rule. Never Tier 0.

```
name:        "shell-run"
description: "Run a single shell command and return its output."
llm_call_required: true
step_descriptions: [
  {
    "step_id":   "step-0",
    "type":      "component",
    "channel":   "orchestrator",
    "include":   ["<uuid:skill-shell>", "<uuid:skill-shell-run>"],
    "label":     "Load shell domain + leaf skill context"
  },
  {
    "step_id":   "step-1",
    "type":      "llm",
    "label":     "LLM assembles and validates the command, then dispatches to executor",
    "note":      "Executor calls ts-shell-run via RecipeStage rust_items"
  },
  {
    "step_id":   "step-2",
    "type":      "component",
    "channel":   "rust",
    "include":   ["<uuid:ts-shell-run>"],
    "label":     "Executor runs the shell command"
  }
]
intent_examples: [
  {"input": "run a command",              "class": 2},
  {"input": "execute a shell command",    "class": 2},
  {"input": "run ls in the project dir",  "class": 3},
  {"input": "check git status",           "class": 3},
  {"input": "shell",                      "class": 1}
]
source: "system"
validation_status: "validated"
```

---

### Step 1.6 — Recipe: `shell-script` (class 21)

> **Tier:** 1 (`llm_call_required: true`) — same §shell-guard applies.

```
name:        "shell-script"
description: "Execute a multi-line shell script authored by the LLM."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-shell>", "<uuid:skill-shell-run>"],
    "label":   "Load shell context"
  },
  {
    "step_id": "step-1",
    "type":    "llm",
    "label":   "LLM writes the script body and validates safety before dispatch"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Executor runs the script"
  }
]
intent_examples: [
  {"input": "run a bash script",          "class": 2},
  {"input": "execute a script",           "class": 2},
  {"input": "write and run a shell script that backs up my files", "class": 3},
  {"input": "bash script",                "class": 1}
]
source: "system"
validation_status: "validated"
```


---

## Step 2 — `builtin.read_file` (File Read)

> **Capability:** `builtin.read_file` · **Effect:** `read` · **Permission:** Allow
> Goes through `CodingCapabilityKind::ReadFile`. Scoped-mount safety enforced by the Rust layer.

---

### Step 2.1 — Tool row (class 0)

```
name:            "read_file"
description:     "Read the full contents of a scoped-workspace file. Supports an
                  optional line-range selector (start-end). Returns {content,
                  line_count, path}."
capability_id:   "builtin.read_file"
effect_type:     "read"
param_schema:    {
  "type": "object",
  "properties": {
    "path":  {"type": "string", "description": "Scoped workspace path to the file"},
    "range": {"type": "string", "description": "Optional line range, format: start-end (1-based)"}
  },
  "required": ["path"]
}
param_template:  {"path": "{{path}}"}
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 2.2 — ToolSkill: `ts-read-file` (class 13)

```
name:          "ts-read-file"
tool_name:     "read_file"
description:   "Read a file from the scoped workspace via builtin.read_file.
                Optional range field narrows to specific lines (format: start-end)."
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

---

### Step 2.3 — Leaf Skill: `skill-read-file` (class 1)

```
name:        "skill-read-file"
class_code:  1
description: "Leaf skill: how to read a file from the workspace."
body: |
  Use `ts-read-file` when you need to inspect a file's current content.
  Always read a file before editing it — never overwrite blindly.
  For large files, use the `range` parameter to read specific line spans
  rather than loading the entire file.
  If the path is unknown, call `skill-list-dir` or `skill-glob` first to
  discover valid paths before attempting to read.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

---

### Step 2.4 — Recipe: `file-read` (class 21)

> **Tier:** 0 eligible after wilson ≥ 0.70 (single deterministic step, no LLM needed).

```
name:        "file-read"
description: "Read a file from the workspace and return its contents."
llm_call_required: false
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-read-file>"],
    "label":   "Load file-read leaf skill context"
  },
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-read-file>"],
    "label":   "Executor reads the file"
  }
]
intent_examples: [
  {"input": "read a file",                          "class": 2},
  {"input": "show me the contents of",              "class": 2},
  {"input": "open file",                            "class": 2},
  {"input": "what is in config.toml",               "class": 3},
  {"input": "read lines 10 to 50 of main.rs",       "class": 3},
  {"input": "file read",                            "class": 1}
]
source: "system"
validation_status: "validated"
```


---

## Step 3 — `builtin.write_file` (File Write)

> **Capability:** `builtin.write_file` · **Effect:** `write` · **Permission:** Allow
> Content size limit: 6 MiB. Overwrites the entire file.

---

### Step 3.1 — Tool row (class 0)

```
name:            "write_file"
description:     "Write or overwrite a file in the scoped workspace. The entire
                  content is replaced. Returns {path, bytes_written}. For targeted
                  edits prefer apply_patch — it is safer and does not require a full
                  read-back."
capability_id:   "builtin.write_file"
effect_type:     "write"
param_schema:    {
  "type": "object",
  "properties": {
    "path":    {"type": "string", "description": "Scoped workspace path"},
    "content": {"type": "string", "description": "Full file content to write"}
  },
  "required": ["path", "content"]
}
param_template:  {"path": "{{path}}", "content": "{{content}}"}
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 3.2 — ToolSkill: `ts-write-file` (class 13)

```
name:          "ts-write-file"
tool_name:     "write_file"
description:   "Write or overwrite a file via builtin.write_file. Replaces the
                entire file content. Content limit: 6 MiB."
param_schema:  [
  {name: "path",    param_type: "string", required: true,
   description: "Workspace-relative scoped path"},
  {name: "content", param_type: "string", required: true,
   description: "Complete new file content"}
]
param_template: {"path": "{{path}}", "content": "{{content}}"}
preconditions:  "Path must resolve within a scoped mount with write permission.
                 Content must not exceed 6 MiB."
error_handling: "FilesystemDenied: path outside mounts — report to orchestrator.
                 Resource limit: content too large — split or compress."
category:       "filesystem"
source:         "system"
validation_status: "validated"
```

---

### Step 3.3 — Leaf Skill: `skill-write-file` (class 1)

```
name:        "skill-write-file"
class_code:  1
description: "Leaf skill: how to write or create a file in the workspace."
body: |
  Use `ts-write-file` to create a new file or fully overwrite an existing one.
  IMPORTANT: read the file first with `skill-read-file` before overwriting an
  existing file to avoid data loss. For small, targeted edits (a few lines),
  always prefer `skill-apply-patch` instead — it is safer because it requires
  matching existing content before changing it.
  Only use `ts-write-file` when creating a new file or when the entire content
  is being replaced intentionally.
source:       "system"
validation_status: "validated"
consumer_tags: ["02:orchestrator", "05:validator"]
```

---

### Step 3.4 — Recipe: `file-write` (class 21)

> **Tier:** 1 — read-then-write sequence requires LLM judgment about content.

```
name:        "file-write"
description: "Read a file (to know current content), then write new content."
llm_call_required: true
step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-read-file>", "<uuid:skill-write-file>"],
    "label":   "Load read + write leaf skill context"
  },
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-read-file>"],
    "label":   "Executor reads current file content"
  },
  {
    "step_id": "step-2",
    "type":    "llm",
    "label":   "LLM composes new file content based on current content and instructions"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-write-file>"],
    "label":   "Executor writes the new file"
  }
]
intent_examples: [
  {"input": "write a file",                         "class": 2},
  {"input": "create a file",                        "class": 2},
  {"input": "save content to a file",               "class": 2},
  {"input": "write a README for this project",      "class": 3},
  {"input": "create config.toml with these values", "class": 3},
  {"input": "file write",                           "class": 1}
]
source: "system"
validation_status: "validated"
```


---

## Step 4 — `builtin.list_dir` (Directory Listing)

> **Capability:** `builtin.list_dir` · **Effect:** `read_filesystem` · **Permission:** Allow
> **Input cap:** 1 MiB · **Output cap:** 1 MiB

---

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
    "path":      { "type": "string",  "description": "Scoped directory path. Defaults to workspace root." },
    "recursive": { "type": "boolean", "description": "Whether to list recursively" },
    "max_depth": { "type": "integer", "minimum": 0, "description": "Maximum recursive depth" }
  },
  "additionalProperties": false
}
param_template:  '{"path":"{{path}}"}'
preconditions:   ["path must be within the active workspace mount"]
error_handling:  "Returns tool error on path-not-found or permission denied; output is scoped — paths outside the mount are rejected before execution"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 4.2 — ToolSkill `ts-list-dir` (class 13)

```
name:        "ts-list-dir"
tool_name:   "list_dir"
description: "Executor binding for list_dir. Lists directory contents through scoped mounts.
             Optional recursive flag and max_depth limit. path defaults to workspace root."
param_schema: {
  "type": "object",
  "properties": {
    "path":      { "type": "string",  "description": "Scoped directory path (omit for workspace root)" },
    "recursive": { "type": "boolean", "description": "Recurse into subdirectories" },
    "max_depth": { "type": "integer", "minimum": 0, "description": "Depth cap for recursive listing" }
  },
  "additionalProperties": false
}
param_template:  '{"path":"{{path}}"}'
preconditions:   ["path within workspace mount scope"]
error_handling:  "path-not-found → tool error with safe summary; permission denied → tool error; output truncated at 1 MiB"
category:        "filesystem"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 4.3 — Leaf Skill `skill-list-dir` (class 1)

```
name:        "skill-list-dir"
description: "Use list_dir to inspect the contents of a directory. When the user wants to
             browse, explore, or enumerate files and folders in a directory — including
             recursive listing — call the ts-list-dir ToolSkill. Provide the scoped path;
             omit it to default to the workspace root. Use max_depth to control how deep a
             recursive scan goes. Interpret the returned entries (names, types, sizes) and
             present them clearly. If the listing is large, summarise by grouping or filtering
             to the entries most relevant to the task."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 4.4 — Recipe `file-list` (class 21)

```
name:        "file-list"
description: "List the contents of a directory."
llm_call_required: false
tier:        0
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-list-dir>"],
    "label":   "Executor lists directory contents"
  }
]
intent_examples: [
  {"input": "list files in this directory",        "class": 1},
  {"input": "show directory contents",             "class": 1},
  {"input": "what files are in the project root",  "class": 2},
  {"input": "list all files recursively",          "class": 2},
  {"input": "ls",                                  "class": 1}
]
source: "system"
validation_status: "validated"
```


---

## Step 5 — `builtin.glob` (Glob File Search)

> **Capability:** `builtin.glob` · **Effect:** `read_filesystem` · **Permission:** Allow
> **Input cap:** 1 MiB · **Output cap:** 1 MiB

---

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
    "pattern":     { "type": "string",  "description": "Glob pattern relative to path" },
    "path":        { "type": "string",  "description": "Scoped root path. Defaults to workspace root." },
    "max_results": { "type": "integer", "minimum": 0, "description": "Maximum number of results" }
  },
  "required": ["pattern"],
  "additionalProperties": false
}
param_template:  '{"pattern":"{{pattern}}"}'
preconditions:   ["pattern required", "path must be within the active workspace mount"]
error_handling:  "Returns tool error on invalid pattern or path outside mount; empty match returns empty list"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 5.2 — ToolSkill `ts-glob` (class 13)

```
name:        "ts-glob"
tool_name:   "glob"
description: "Executor binding for glob. Required: pattern (glob expression). Optional: path
             (scoped root, defaults to workspace root), max_results (cap on returned paths).
             Returns a list of matching paths sorted by modification time."
param_schema: {
  "type": "object",
  "properties": {
    "pattern":     { "type": "string",  "description": "Glob pattern (e.g. '**/*.rs', 'src/**/*.test.ts')" },
    "path":        { "type": "string",  "description": "Scoped root path (omit for workspace root)" },
    "max_results": { "type": "integer", "minimum": 0 }
  },
  "required": ["pattern"],
  "additionalProperties": false
}
param_template:  '{"pattern":"{{pattern}}"}'
preconditions:   ["pattern must not be empty"]
error_handling:  "invalid pattern → tool error; empty result → empty list (not an error)"
category:        "filesystem"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 5.3 — Leaf Skill `skill-glob` (class 1)

```
name:        "skill-glob"
description: "Use glob to find files matching a pattern. When the user needs to locate files
             by name pattern across the project — such as finding all TypeScript files, all
             test files, or any specific filename — call the ts-glob ToolSkill with the
             appropriate glob pattern. The pattern supports ** for recursive directory
             matching and * for single-level wildcards. Set path to restrict the search to a
             subdirectory. Inspect the returned file list and use it to inform subsequent
             reads, writes, or analysis steps."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 5.4 — Recipe `file-glob` (class 21)

```
name:        "file-glob"
description: "Find files matching a glob pattern."
llm_call_required: false
tier:        0
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-glob>"],
    "label":   "Executor finds matching files"
  }
]
intent_examples: [
  {"input": "find all TypeScript files",           "class": 1},
  {"input": "find files matching *.rs",            "class": 1},
  {"input": "search for test files in src",        "class": 2},
  {"input": "glob pattern **/*.json",              "class": 1},
  {"input": "find all config files in this repo",  "class": 2}
]
source: "system"
validation_status: "validated"
```


---

## Step 6 — `builtin.grep` (Content Search)

> **Capability:** `builtin.grep` · **Effect:** `read_filesystem` · **Permission:** Allow
> **Input cap:** 1 MiB · **Output cap:** 1 MiB

---

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
    "pattern":         { "type": "string",  "description": "Regular expression to search for" },
    "path":            { "type": "string",  "description": "Scoped file or directory path. Defaults to workspace root." },
    "glob":            { "type": "string",  "description": "Optional glob filter relative to path" },
    "type_filter":     { "type": "string",  "description": "Optional file type filter" },
    "output_mode":     { "type": "string",  "enum": ["content", "files_with_matches", "count"],
                         "description": "Output mode. Defaults to files_with_matches." },
    "case_insensitive":{ "type": "boolean" },
    "multiline":       { "type": "boolean" },
    "context":         { "type": "integer", "minimum": 0 },
    "before_context":  { "type": "integer", "minimum": 0 },
    "after_context":   { "type": "integer", "minimum": 0 },
    "head_limit":      { "type": "integer", "minimum": 0 },
    "offset":          { "type": "integer", "minimum": 0 }
  },
  "required": ["pattern"],
  "additionalProperties": false
}
param_template:  '{"pattern":"{{pattern}}"}'
preconditions:   ["pattern required", "path must be within the active workspace mount"]
error_handling:  "Invalid regex → tool error with safe summary; empty results → empty list; output truncated at 1 MiB"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 6.2 — ToolSkill `ts-grep` (class 13)

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
    "pattern":         { "type": "string" },
    "path":            { "type": "string" },
    "glob":            { "type": "string" },
    "output_mode":     { "type": "string", "enum": ["content", "files_with_matches", "count"] },
    "case_insensitive":{ "type": "boolean" },
    "multiline":       { "type": "boolean" },
    "context":         { "type": "integer", "minimum": 0 },
    "before_context":  { "type": "integer", "minimum": 0 },
    "after_context":   { "type": "integer", "minimum": 0 },
    "head_limit":      { "type": "integer", "minimum": 0 },
    "offset":          { "type": "integer", "minimum": 0 }
  },
  "required": ["pattern"],
  "additionalProperties": false
}
param_template:  '{"pattern":"{{pattern}}"}'
preconditions:   ["pattern must be a valid regex"]
error_handling:  "invalid regex → tool error; no matches → empty result (not an error); output capped at 1 MiB"
category:        "filesystem"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 6.3 — Leaf Skill `skill-grep` (class 1)

```
name:        "skill-grep"
description: "Use grep to search file contents by regular expression. When the user wants to
             find which files contain a pattern — a function name, a constant, an error
             message, a class reference — call the ts-grep ToolSkill. Choose output_mode:
             use 'files_with_matches' when you only need which files match; use 'content'
             when you need the matching lines (add context lines if surrounding code helps);
             use 'count' when only occurrence counts are needed. Use glob to restrict the
             file types searched. Use case_insensitive when the user's pattern is conceptual
             rather than exact. Interpret and summarise the results in the context of the task."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 6.4 — Recipe `file-grep` (class 21)

```
name:        "file-grep"
description: "Search file contents using a regular expression."
llm_call_required: false
tier:        0
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-grep>"],
    "label":   "Executor searches file contents"
  }
]
intent_examples: [
  {"input": "find all uses of function foo",       "class": 1},
  {"input": "search for TODO comments in src",     "class": 2},
  {"input": "which files import React",            "class": 2},
  {"input": "grep for error handling patterns",    "class": 2},
  {"input": "find all occurrences of FIXME",       "class": 1}
]
source: "system"
validation_status: "validated"
```


---

## Step 7 — `builtin.apply_patch` (Targeted File Edit)

> **Capability:** `builtin.apply_patch` · **Effect:** `read_filesystem` + `write_filesystem` · **Permission:** Ask
> **Input cap:** 21 MiB · **Output cap:** 1 MiB

---

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
    "path":        { "type": "string", "description": "Scoped file path to patch" },
    "old_string":  { "type": "string", "description": "Exact text to replace" },
    "new_string":  { "type": "string", "description": "Replacement text" },
    "replace_all": { "type": "boolean", "description": "Replace every match instead of exactly one" }
  },
  "required": ["path", "old_string", "new_string"],
  "additionalProperties": false
}
param_template:  '{"path":"{{path}}","old_string":"{{old_string}}","new_string":"{{new_string}}"}'
preconditions:   ["path within workspace mount scope", "old_string must appear in file exactly once unless replace_all is true"]
error_handling:  "old_string not found → tool error with safe summary; multiple matches without replace_all → tool error; path not found → tool error"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 7.2 — ToolSkill `ts-apply-patch` (class 13)

```
name:        "ts-apply-patch"
tool_name:   "apply_patch"
description: "Executor binding for apply_patch. Required: path (scoped file), old_string
             (exact text to replace), new_string (replacement). Optional: replace_all
             (replace every occurrence; default replaces exactly one match and errors if
             the string appears more than once). old_string must include enough surrounding
             context to be unique within the file."
param_schema: {
  "type": "object",
  "properties": {
    "path":        { "type": "string" },
    "old_string":  { "type": "string" },
    "new_string":  { "type": "string" },
    "replace_all": { "type": "boolean" }
  },
  "required": ["path", "old_string", "new_string"],
  "additionalProperties": false
}
param_template:  '{"path":"{{path}}","old_string":"{{old_string}}","new_string":"{{new_string}}"}'
preconditions:   ["old_string must be unique in file unless replace_all is set", "path within mount scope"]
error_handling:  "not-found → tool error; ambiguous match without replace_all → tool error"
category:        "filesystem"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 7.3 — Leaf Skill `skill-apply-patch` (class 1)

```
name:        "skill-apply-patch"
description: "Use apply_patch to make a targeted edit to an existing file. When the user
             wants to change a specific section of a file — fix a bug, update a value,
             rename a symbol, or replace a code block — call the ts-apply-patch ToolSkill.
             old_string must be an exact copy of the current file content you want to replace,
             with enough surrounding context (at least 3–5 lines) to be unique in the file.
             new_string is what it should become. If the same string appears multiple times
             and all occurrences should change, set replace_all to true. Read the file first
             with read_file when you are uncertain of the exact current content."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 7.4 — Recipe `file-patch` (class 21)

```
name:        "file-patch"
description: "Apply a targeted search-replace edit to a file."
llm_call_required: true
tier:        1
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-read-file>"],
    "label":   "Executor reads current file content for context"
  },
  {
    "step_id": "step-2",
    "type":    "llm",
    "label":   "LLM determines exact old_string and new_string for the requested change"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-apply-patch>"],
    "label":   "Executor applies the patch"
  }
]
intent_examples: [
  {"input": "fix this bug in the function",         "class": 3},
  {"input": "rename variable foo to bar in utils",  "class": 3},
  {"input": "update the default timeout value",     "class": 2},
  {"input": "replace the old error message",        "class": 2},
  {"input": "apply patch to file",                  "class": 2}
]
source: "system"
validation_status: "validated"
```


---

## Step 7.x — Domain Skill `skill-filesystem` (class 2)

> Covers all five filesystem leaf skills: read_file, write_file, list_dir, glob, apply_patch.
> grep is a search operation but referenced here as part of the filesystem domain.

```
name:        "skill-filesystem"
description: "The filesystem domain gives you six scoped tools for working with the workspace.
             Use each for its specific purpose:
             — skill-read-file: Read a file's content (full or paginated by line range).
             — skill-write-file: Create or completely overwrite a file with new content.
             — skill-apply-patch: Make a targeted edit to an existing file using exact
               search-replace. Prefer this over write_file when changing a specific section.
             — skill-list-dir: List directory contents (optionally recursive).
             — skill-glob: Find files by name pattern (e.g. **/*.rs, src/**/*.test.ts).
             — skill-grep: Search file contents by regular expression across the workspace.

             Decision guide:
             • Exploring structure → skill-list-dir or skill-glob
             • Searching content → skill-grep
             • Reading a specific file → skill-read-file
             • Making a targeted change → skill-apply-patch (not write_file)
             • Creating a file or replacing all content → skill-write-file
             • Large edits where the entire file must be rewritten → skill-write-file

             All paths are scoped to the workspace mount; paths outside the mount are
             rejected. Output is capped at 1 MiB per call."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```


---

## Step 8 — `builtin.http` (HTTP Request, Inline Response)

> **Capability:** `builtin.http` · **Effect:** `network_egress` · **Permission:** Ask
> **Timeout:** 30 s · **Response body cap:** 256 KiB inline

---

### Step 8.1 — Tool row (class 0)

```
name:            "http"
description:     "Perform an HTTP or HTTPS request and return the response inline. Supports
                 GET, POST, PUT, PATCH, DELETE, HEAD. Request body may be a string or JSON
                 value. Response body is capped at 256 KiB for inline delivery; larger
                 responses should use builtin.http.save."
capability_id:   "builtin.http"
effect_type:     "network_egress"
param_schema: {
  "type": "object",
  "properties": {
    "url":                 { "type": "string",  "description": "Absolute HTTP or HTTPS URL" },
    "method":              { "type": "string",  "enum": ["get","post","put","patch","delete","head"],
                             "description": "HTTP method. Defaults to get." },
    "headers":             { "description": "HTTP headers as an object or array of {name,value} entries" },
    "body":                { "description": "String or JSON request body" },
    "body_base64":         { "type": "string",  "description": "Base64-encoded request body" },
    "response_body_limit": { "type": "integer", "minimum": 1, "maximum": 262144, "default": 49152,
                             "description": "Max inline response bytes. Capped at 256 KiB." },
    "timeout_ms":          { "type": "integer", "minimum": 1, "maximum": 30000, "default": 10000 }
  },
  "required": ["url"],
  "additionalProperties": false
}
param_template:  '{"url":"{{url}}"}'
preconditions:   ["url must be absolute http/https", "network egress must be permitted by policy"]
error_handling:  "connection failure → tool error with safe summary; response body over limit → truncated with guidance to use http.save; non-2xx status returned in output (not a tool error)"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 8.2 — ToolSkill `ts-http-fetch` (class 13)

```
name:        "ts-http-fetch"
tool_name:   "http"
description: "Executor binding for builtin.http. Required: url. Optional: method (default
             get), headers (object or [{name,value}] array), body (string or JSON),
             body_base64, response_body_limit (max 256 KiB), timeout_ms (max 30 000).
             Non-2xx status codes are returned in the output — they are not tool errors."
param_schema: {
  "type": "object",
  "properties": {
    "url":                 { "type": "string" },
    "method":              { "type": "string", "enum": ["get","post","put","patch","delete","head"] },
    "headers":             {},
    "body":                {},
    "body_base64":         { "type": "string" },
    "response_body_limit": { "type": "integer", "minimum": 1, "maximum": 262144 },
    "timeout_ms":          { "type": "integer", "minimum": 1, "maximum": 30000 }
  },
  "required": ["url"],
  "additionalProperties": false
}
param_template:  '{"url":"{{url}}"}'
preconditions:   ["url must begin with http:// or https://"]
error_handling:  "network failure → tool error; body truncation → status field in output; timeout_ms capped at 30 000"
category:        "network"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 8.3 — Leaf Skill `skill-http-fetch` (class 1)

```
name:        "skill-http-fetch"
description: "Use the http tool to make an HTTP request and receive the response inline.
             When the user needs to fetch data from an API, download a web resource, or
             call a webhook, call the ts-http-fetch ToolSkill. For GET requests provide
             only the url. For POST/PUT/PATCH supply method and body. Add headers when
             authentication or content-type is required. The response body is limited to
             256 KiB inline — if the response is expected to be large, use skill-http-save
             instead to stream it to a file. Non-2xx status codes appear in the output and
             are not tool errors; inspect the status field and handle error responses."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 8.4 — Recipe `http-get` (class 21)

```
name:        "http-get"
description: "Fetch a URL via HTTP GET and return the response."
llm_call_required: false
tier:        0
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-http-fetch>"],
    "label":   "Executor performs GET request"
  }
]
intent_examples: [
  {"input": "fetch this URL",                       "class": 1},
  {"input": "GET https://api.example.com/data",     "class": 1},
  {"input": "download the JSON from this endpoint", "class": 2},
  {"input": "make an HTTP request to this API",     "class": 2},
  {"input": "check if this URL is reachable",       "class": 1}
]
source: "system"
validation_status: "validated"
```

---

### Step 8.5 — Recipe `http-post` (class 21)

```
name:        "http-post"
description: "Send an HTTP POST request with a JSON body."
llm_call_required: true
tier:        1
step_descriptions: [
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
    "label":   "Executor performs POST request"
  }
]
intent_examples: [
  {"input": "POST this data to the API",            "class": 2},
  {"input": "send a webhook notification",          "class": 2},
  {"input": "submit a form to this endpoint",       "class": 3},
  {"input": "call this API with a JSON body",       "class": 3},
  {"input": "create a GitHub issue via API",        "class": 3}
]
source: "system"
validation_status: "validated"
```


---

## Step 9 — `builtin.http.save` (HTTP Request, Response Saved to File)

> **Capability:** `builtin.http.save` · **Effect:** `network_egress` + `write_filesystem` · **Permission:** Ask
> **Timeout:** 30 s · **Response body cap:** 10 MiB saved

---

### Step 9.1 — Tool row (class 0)

```
name:            "http.save"
description:     "Perform an HTTP or HTTPS request and save the sanitized response body to a
                 scoped file path. Accepts up to 10 MiB of response body. Used when the
                 response is too large to return inline or must be persisted for later reading."
capability_id:   "builtin.http.save"
effect_type:     "mixed"
param_schema: {
  "type": "object",
  "properties": {
    "url":                 { "type": "string",  "description": "Absolute HTTP or HTTPS URL" },
    "save_to":             { "type": "string",  "description": "Scoped path to save the response body" },
    "method":              { "type": "string",  "enum": ["get","post","put","patch","delete","head"] },
    "headers":             { "description": "HTTP headers as an object or array of {name,value} entries" },
    "body":                { "description": "String or JSON request body" },
    "body_base64":         { "type": "string" },
    "response_body_limit": { "type": "integer", "minimum": 1, "maximum": 10485760, "default": 10485760,
                             "description": "Max response body bytes to save. Defaults to 10 MiB." },
    "timeout_ms":          { "type": "integer", "minimum": 1, "maximum": 30000, "default": 10000 }
  },
  "required": ["url", "save_to"],
  "additionalProperties": false
}
param_template:  '{"url":"{{url}}","save_to":"{{save_to}}"}'
preconditions:   ["url must be absolute http/https", "save_to must be within workspace mount"]
error_handling:  "connection failure → tool error; save_to path outside mount → tool error; response body over limit → truncated"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 9.2 — ToolSkill `ts-http-save` (class 13)

```
name:        "ts-http-save"
tool_name:   "http.save"
description: "Executor binding for builtin.http.save. Required: url, save_to (scoped path).
             Optional: method, headers, body, body_base64, response_body_limit (default and
             max 10 MiB), timeout_ms (max 30 000). Saves the sanitized response body to
             save_to; returns metadata (status, headers, bytes_saved) inline."
param_schema: {
  "type": "object",
  "properties": {
    "url":                 { "type": "string" },
    "save_to":             { "type": "string" },
    "method":              { "type": "string", "enum": ["get","post","put","patch","delete","head"] },
    "headers":             {},
    "body":                {},
    "body_base64":         { "type": "string" },
    "response_body_limit": { "type": "integer", "minimum": 1, "maximum": 10485760 },
    "timeout_ms":          { "type": "integer", "minimum": 1, "maximum": 30000 }
  },
  "required": ["url", "save_to"],
  "additionalProperties": false
}
param_template:  '{"url":"{{url}}","save_to":"{{save_to}}"}'
preconditions:   ["save_to within workspace mount scope"]
error_handling:  "network failure → tool error; save_to outside mount → tool error"
category:        "network"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 9.3 — Leaf Skill `skill-http-save` (class 1)

```
name:        "skill-http-save"
description: "Use http.save to fetch a large HTTP response and save it directly to a file.
             When a download is expected to exceed 256 KiB, or when the content must be
             persisted for later reading or processing, call the ts-http-save ToolSkill
             instead of skill-http-fetch. Provide the url and a scoped save_to path. After
             the call, use skill-read-file to inspect the saved content or inform the user
             of the file location."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 9.4 — Recipe `http-save` (class 21)

```
name:        "http-save"
description: "Fetch a URL and save the response body to a file."
llm_call_required: false
tier:        0
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-http-save>"],
    "label":   "Executor fetches URL and saves response to file"
  }
]
intent_examples: [
  {"input": "download this file and save it",          "class": 2},
  {"input": "fetch the API response and write to disk","class": 2},
  {"input": "save the download to workspace",          "class": 2},
  {"input": "GET this URL and save the result",        "class": 1},
  {"input": "download a large JSON response",          "class": 2}
]
source: "system"
validation_status: "validated"
```


---

## Step 9.x — HTTP Domain Skill + PythonCode Helpers

---

### Step 9.x.1 — Domain Skill `skill-http` (class 2)

```
name:        "skill-http"
description: "The http domain provides two tools for making HTTP requests:
             — skill-http-fetch: Makes a request and returns the response body inline.
               Use for API calls, webhooks, and small downloads (≤256 KiB).
             — skill-http-save: Makes a request and saves the response body to a workspace
               file. Use for large downloads (>256 KiB) or when the content must be persisted.

             Decision guide:
             • Response expected to be small / will be processed immediately → skill-http-fetch
             • Response expected to be large, or must be saved → skill-http-save (then read file)
             • POST/PUT/PATCH with a body → supply method and body in either skill
             • Authenticated request → supply Authorization header

             Non-2xx HTTP responses are not tool errors — they appear in the output status
             field. Always inspect the status code and handle API errors explicitly.
             Use pc-http-status-check to test whether a response was successful."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 9.x.2 — PythonCode `pc-http-status-check` (class 22)

> Pure logic: takes an integer status code, returns a boolean success flag.
> No I/O, no imports, no network.

```
name:        "pc-http-status-check"
description: "Pure-logic helper: returns True when the HTTP status code indicates success
             (2xx range), False otherwise. Input: status_code (integer). Output: is_success
             (boolean)."
content: |
  # IBS substitution delivers the literal integer value before this body runs.
  # At runtime there is no vars dict — {{vars.slot0}} is replaced by the IBS.
  # After substitution the body looks like: status_code = 200
  status_code = {{vars.slot0}}
  is_success = 200 <= status_code < 300
  result = {"is_success": is_success, "status_code": status_code}
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 9.x.3 — PythonCode `pc-json-extract-field` (class 22)

> Pure logic: dot-path JSON field extractor. No I/O, no imports, no network.

```
name:        "pc-json-extract-field"
description: "Pure-logic helper: extracts a value from a JSON object by dot-separated path.
             Input: data (dict), path (dot-separated string e.g. 'result.items.0').
             Output: the extracted value or None if the path is not found."
content: |
  # No I/O, no imports. IBS bakes in 'data' and 'path' values before execution.
  # After IBS substitution, 'data' is a Python dict literal and 'path' is a string.
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


---

## Step 10 — `builtin.memory_search` (Persistent Memory Search)

> **Capability:** `builtin.memory_search` · **Effect:** `read_memory` · **Permission:** Allow

---

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
    "query": { "type": "string", "description": "Natural language search query" },
    "q":     { "type": "string", "description": "Alias for query" },
    "text":  { "type": "string", "description": "Alias for query" },
    "pattern": { "type": "string", "description": "Alias for query" },
    "limit": { "type": "integer", "minimum": 1, "maximum": 20, "default": 5 }
  },
  "required": ["query"],
  "additionalProperties": false
}
param_template:  '{"query":"{{query}}"}'
preconditions:   ["query must not be empty"]
error_handling:  "empty result is not an error; memory backend unavailable → tool error"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 10.2 — ToolSkill `ts-memory-search` (class 13)

```
name:        "ts-memory-search"
tool_name:   "memory_search"
description: "Executor binding for memory_search. Required: query (natural language).
             Optional: limit (1–20, default 5). Aliases: q, text, pattern all accepted
             as query. Returns ranked memory documents with content and relevance scores."
param_schema: {
  "type": "object",
  "properties": {
    "query": { "type": "string" },
    "limit": { "type": "integer", "minimum": 1, "maximum": 20, "default": 5 }
  },
  "required": ["query"],
  "additionalProperties": false
}
param_template:  '{"query":"{{query}}"}'
preconditions:   ["query must not be empty"]
error_handling:  "no results → empty list (not an error)"
category:        "memory"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 10.3 — Leaf Skill `skill-memory-search` (class 1)

```
name:        "skill-memory-search"
description: "Use memory_search to retrieve relevant information from the agent's persistent
             memory. When the user's task requires recalling past work, finding saved notes,
             or checking whether something was previously recorded, call the ts-memory-search
             ToolSkill with a natural language query. Set limit higher (up to 20) when you
             need broader recall coverage. Review the returned documents and surface only
             those relevant to the current context. If no relevant results are found, proceed
             with what is known from the conversation."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 10.4 — Recipe `memory-search` (class 21)

```
name:        "memory-search"
description: "Search the agent's persistent memory."
llm_call_required: false
tier:        0
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-memory-search>"],
    "label":   "Executor searches persistent memory"
  }
]
intent_examples: [
  {"input": "what do you remember about this project",  "class": 2},
  {"input": "search memory for authentication notes",   "class": 2},
  {"input": "find any saved notes about this topic",    "class": 2},
  {"input": "recall what we discussed last time",       "class": 2},
  {"input": "memory search",                            "class": 1}
]
source: "system"
validation_status: "validated"
```


---

## Step 11 — `builtin.memory_write` (Persistent Memory Write)

> **Capability:** `builtin.memory_write` · **Effect:** `write_memory` · **Permission:** Allow

---

### Step 11.1 — Tool row (class 0)

```
name:            "memory_write"
description:     "Write or append content to the agent's persistent memory. Default target is
                 'daily_log' (today's dated log). Other targets: 'memory' (MEMORY.md),
                 'heartbeat' (HEARTBEAT.md checklist), 'bootstrap' (clears BOOTSTRAP.md),
                 or any relative memory document path. Supports patch mode (old_string /
                 new_string) for targeted updates."
capability_id:   "builtin.memory_write"
effect_type:     "write_memory"
param_schema: {
  "type": "object",
  "properties": {
    "content":     { "type": "string",  "description": "Content to write or append" },
    "target":      { "type": "string",  "description": "Destination: 'memory', 'daily_log' (default), 'heartbeat', 'bootstrap', or relative path" },
    "append":      { "type": "boolean", "description": "Append when true; replace when false", "default": true },
    "metadata":    { "type": "object",  "description": "Optional document metadata" },
    "old_string":  { "type": "string",  "description": "Exact text to replace (patch mode)" },
    "new_string":  { "type": "string",  "description": "Replacement text (patch mode)" },
    "replace_all": { "type": "boolean", "description": "Replace every old_string occurrence" },
    "timezone":    { "type": "string",  "description": "IANA timezone for daily_log date resolution" }
  },
  "additionalProperties": false
}
param_template:  '{"content":"{{content}}"}'
preconditions:   ["content required unless using bootstrap target"]
error_handling:  "old_string not found in patch mode → tool error; write failure → tool error"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 11.2 — ToolSkill `ts-memory-write` (class 13)

```
name:        "ts-memory-write"
tool_name:   "memory_write"
description: "Executor binding for memory_write. Default writes to 'daily_log' (append mode).
             To write to MEMORY.md: set target='memory'. To do a targeted edit: supply
             old_string and new_string. Setting append=false replaces the full document.
             bootstrap target clears BOOTSTRAP.md (content ignored)."
param_schema: {
  "type": "object",
  "properties": {
    "content":     { "type": "string" },
    "target":      { "type": "string" },
    "append":      { "type": "boolean" },
    "old_string":  { "type": "string" },
    "new_string":  { "type": "string" },
    "replace_all": { "type": "boolean" },
    "timezone":    { "type": "string" }
  },
  "additionalProperties": false
}
param_template:  '{"content":"{{content}}"}'
preconditions:   ["patch mode requires both old_string and new_string"]
error_handling:  "patch not found → tool error with safe summary"
category:        "memory"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 11.3 — Leaf Skill `skill-memory-write` (class 1)

```
name:        "skill-memory-write"
description: "Use memory_write to persist information in the agent's memory. When the user
             wants to save notes, record decisions, log work progress, or update persistent
             context, call the ts-memory-write ToolSkill. The default target is 'daily_log'
             which appends to today's dated log — use this for session notes and progress.
             Use target='memory' to update the main MEMORY.md document. For a targeted
             edit to existing memory content, supply old_string and new_string. Use
             append=false only when fully replacing a document's content is intended."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 11.4 — Recipe `memory-write` (class 21)

```
name:        "memory-write"
description: "Write or append content to the agent's persistent memory."
llm_call_required: false
tier:        0
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-memory-write>"],
    "label":   "Executor writes to persistent memory"
  }
]
intent_examples: [
  {"input": "save this to memory",                    "class": 2},
  {"input": "remember this for later",                "class": 2},
  {"input": "log this progress note",                 "class": 2},
  {"input": "update MEMORY.md with this decision",    "class": 2},
  {"input": "add this to my daily log",               "class": 1}
]
source: "system"
validation_status: "validated"
```

---

## Step 12 — `builtin.memory_read` (Persistent Memory Read by Path)

> **Capability:** `builtin.memory_read` · **Effect:** `read_memory` · **Permission:** Allow

---

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
    "path": { "type": "string", "description": "Relative memory document path to read" }
  },
  "required": ["path"],
  "additionalProperties": false
}
param_template:  '{"path":"{{path}}"}'
preconditions:   ["path must not be empty"]
error_handling:  "document not found → tool error with safe summary"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 12.2 — ToolSkill `ts-memory-read` (class 13)

```
name:        "ts-memory-read"
tool_name:   "memory_read"
description: "Executor binding for memory_read. Required: path (relative memory document
             path). Returns the full document content. Use when you know the exact path;
             use ts-memory-search for semantic discovery."
param_schema: {
  "type": "object",
  "properties": {
    "path": { "type": "string" }
  },
  "required": ["path"],
  "additionalProperties": false
}
param_template:  '{"path":"{{path}}"}'
preconditions:   ["path must not be empty"]
error_handling:  "not found → tool error"
category:        "memory"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 12.3 — Leaf Skill `skill-memory-read` (class 1)

```
name:        "skill-memory-read"
description: "Use memory_read to fetch a specific memory document by its exact path. When
             you know the path of a document in the agent's memory — such as MEMORY.md,
             HEARTBEAT.md, or a specific note file — call the ts-memory-read ToolSkill.
             If you do not know the exact path, use skill-memory-search to discover it first,
             or use skill-memory-tree to browse the directory structure."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 12.4 — Recipe `memory-read` (class 21)

```
name:        "memory-read"
description: "Read a specific memory document by path."
llm_call_required: false
tier:        0
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-memory-read>"],
    "label":   "Executor reads memory document by path"
  }
]
intent_examples: [
  {"input": "read MEMORY.md",                         "class": 1},
  {"input": "show me the contents of HEARTBEAT.md",   "class": 1},
  {"input": "read my memory document",                "class": 2},
  {"input": "open this memory file",                  "class": 2},
  {"input": "show memory at this path",               "class": 1}
]
source: "system"
validation_status: "validated"
```

---

## Step 13 — `builtin.memory_tree` (Memory Directory Tree)

> **Capability:** `builtin.memory_tree` · **Effect:** `read_memory` · **Permission:** Allow

---

### Step 13.1 — Tool row (class 0)

```
name:            "memory_tree"
description:     "List the directory tree of the agent's persistent memory. Returns entry
                 names and types up to the specified depth. Used to discover the memory
                 structure before targeted reads."
capability_id:   "builtin.memory_tree"
effect_type:     "read_memory"
param_schema: {
  "type": "object",
  "properties": {
    "path":  { "type": "string",  "description": "Relative memory directory path (omit for root)", "default": "" },
    "depth": { "type": "integer", "minimum": 1, "maximum": 10, "default": 1,
               "description": "Maximum directory depth to include" }
  },
  "additionalProperties": false
}
param_template:  '{}'
preconditions:   []
error_handling:  "path not found in memory → tool error"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 13.2 — ToolSkill `ts-memory-tree` (class 13)

```
name:        "ts-memory-tree"
tool_name:   "memory_tree"
description: "Executor binding for memory_tree. Optional: path (relative memory dir, defaults
             to root), depth (1–10, default 1). Returns the directory tree of persistent
             memory. Use to discover what memory documents exist before reading them."
param_schema: {
  "type": "object",
  "properties": {
    "path":  { "type": "string" },
    "depth": { "type": "integer", "minimum": 1, "maximum": 10, "default": 1 }
  },
  "additionalProperties": false
}
param_template:  '{}'
preconditions:   []
error_handling:  "path not found → tool error"
category:        "memory"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 13.3 — Leaf Skill `skill-memory-tree` (class 1)

```
name:        "skill-memory-tree"
description: "Use memory_tree to browse the structure of the agent's persistent memory. When
             you need to discover what memory documents exist — without knowing the exact
             path — call the ts-memory-tree ToolSkill. Increase depth to see deeper levels
             of the hierarchy. Use the returned structure to decide which documents to read
             with skill-memory-read or to inform a skill-memory-search query."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 13.4 — Recipe `memory-tree` (class 21)

```
name:        "memory-tree"
description: "List the directory structure of the agent's persistent memory."
llm_call_required: false
tier:        0
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-memory-tree>"],
    "label":   "Executor lists memory directory tree"
  }
]
intent_examples: [
  {"input": "what files are in my memory",            "class": 2},
  {"input": "show me the memory directory structure", "class": 2},
  {"input": "list all memory documents",              "class": 1},
  {"input": "browse my memory files",                 "class": 2},
  {"input": "memory tree",                            "class": 1}
]
source: "system"
validation_status: "validated"
```


---

## Step 13.x — Memory PythonCode Helpers + Domain Skill

---

### Step 13.x.1 — PythonCode `pc-memory-extract-section` (class 22)

> Pure logic: regex-based section extractor from a Markdown memory document.
> No I/O, no imports, no network.

```
name:        "pc-memory-extract-section"
description: "Pure-logic helper: extracts a named section from a Markdown document using
             heading regex. Input: document content (string), heading (string — exact match
             of the heading text without the # prefix). Output: section_content (string or
             None if not found)."
content: |
  # No I/O, no imports. IBS bakes in 'content' and 'heading' before execution.
  content = "{{vars.slot0}}"
  heading = "{{vars.slot1}}"
  import_free_regex_result = None
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

---

### Step 13.x.2 — PythonCode `pc-memory-format-entry` (class 22)

> Pure logic: formats a new memory entry for appending. Timestamp passed as a string param,
> NOT obtained via datetime (no I/O, no imports).

```
name:        "pc-memory-format-entry"
description: "Pure-logic helper: formats a memory entry string ready for appending to a
             memory document. Input: text (string), timestamp_str (string — caller supplies
             the pre-fetched timestamp). Output: formatted_entry (string)."
content: |
  # No I/O, no imports, no datetime. Caller must supply timestamp_str.
  # IBS bakes in 'text' and 'timestamp_str' before execution.
  text = "{{vars.slot0}}"
  timestamp_str = "{{vars.slot1}}"
  formatted_entry = f"### {timestamp_str}\n\n{text}\n"
  result = {"formatted_entry": formatted_entry}
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 13.x.3 — Domain Skill `skill-memory` (class 2)

```
name:        "skill-memory"
description: "The memory domain provides four tools for working with the agent's persistent
             memory store:
             — skill-memory-search: Semantic search across all memory documents.
               Use when you need to find information by topic without knowing the exact path.
             — skill-memory-read: Read a specific document by exact path.
               Use when you already know the file path (e.g. MEMORY.md, HEARTBEAT.md).
             — skill-memory-write: Write or append to a memory document.
               Default target is 'daily_log'. Use target='memory' for the main doc.
             — skill-memory-tree: Browse the directory structure of the memory store.
               Use to discover what documents exist.

             Decision guide:
             • Recalling information by topic → skill-memory-search
             • Reading a known file → skill-memory-read
             • Saving notes or progress → skill-memory-write (daily_log)
             • Updating permanent context → skill-memory-write (target='memory')
             • Discovering what files exist → skill-memory-tree

             Memory documents persist across sessions. Prefer explicit, structured writes
             (clear headings, concise facts) to maximise retrieval quality."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```


---

## Step 14 — `builtin.time` (Time Operations)

> **Capability:** `builtin.time` · **Effect:** `read_only` · **Permission:** Allow
> **Operations:** now, parse, convert, format, diff — all routed through one Tool.

---

### Step 14.1 — Tool row (class 0)

```
name:            "time"
description:     "Perform time and timezone operations: get the current time (now), parse a
                 timestamp string (parse), convert between timezones (convert), format a
                 timestamp (format), or compute the difference between two timestamps (diff)."
capability_id:   "builtin.time"
effect_type:     "read_only"
param_schema: {
  "type": "object",
  "properties": {
    "operation":   { "type": "string", "enum": ["now","parse","convert","format","diff"],
                     "description": "Time operation to perform. Defaults to now." },
    "input":       { "type": "string", "description": "Timestamp input for parse, convert, format, or diff" },
    "timestamp":   { "type": "string", "description": "Alias for input" },
    "timestamp2":  { "type": "string", "description": "Second timestamp for diff" },
    "timezone":    { "type": "string", "description": "IANA timezone name" },
    "from_timezone":{ "type": "string", "description": "IANA timezone for interpreting the input" },
    "to_timezone": { "type": "string", "description": "IANA timezone for conversion output" },
    "format":      { "type": "string", "description": "chrono format string for format operation" },
    "format_string":{ "type": "string", "description": "Alias for format" }
  },
  "additionalProperties": false
}
param_template:  '{"operation":"now"}'
preconditions:   []
error_handling:  "invalid timezone → tool error with safe summary; invalid timestamp format → tool error"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 14.2 — ToolSkill `ts-time-now` (class 13)

```
name:        "ts-time-now"
tool_name:   "time"
description: "Executor binding: get the current UTC timestamp. Operation: 'now'. Optional:
             timezone (IANA name) to return the current time in a specific timezone."
param_schema: {
  "type": "object",
  "properties": {
    "operation": { "type": "string", "enum": ["now"], "default": "now" },
    "timezone":  { "type": "string", "description": "IANA timezone name" }
  },
  "additionalProperties": false
}
param_template:  '{"operation":"now"}'
preconditions:   []
error_handling:  "invalid timezone → tool error"
category:        "time"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 14.3 — Leaf Skill `skill-time-now` (class 1)

```
name:        "skill-time-now"
description: "Use the time tool to get the current date and time. When the user asks what
             time or date it is, or when a recipe step needs the current timestamp, call
             the ts-time-now ToolSkill. Provide a timezone parameter if the user specified
             a timezone or locale. The returned timestamp can be used as input to other
             time operations or to stamp memory entries."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 14.4 — Recipe `time-now` (class 21)

```
name:        "time-now"
description: "Get the current date and time."
llm_call_required: false
tier:        0
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-time-now>"],
    "label":   "Executor returns current timestamp"
  }
]
intent_examples: [
  {"input": "what time is it",                  "class": 1},
  {"input": "what is today's date",             "class": 1},
  {"input": "current time in Tokyo",            "class": 2},
  {"input": "get the current UTC timestamp",    "class": 1},
  {"input": "what day is it",                   "class": 1}
]
source: "system"
validation_status: "validated"
```

---

### Step 14.5 — ToolSkill `ts-time-parse` (class 13)

```
name:        "ts-time-parse"
tool_name:   "time"
description: "Executor binding: parse a timestamp string into a structured time object.
             Operation: 'parse'. Required: input (timestamp string). Optional: timezone
             (IANA timezone to interpret the input)."
param_schema: {
  "type": "object",
  "properties": {
    "operation": { "type": "string", "enum": ["parse"] },
    "input":     { "type": "string", "description": "Timestamp string to parse" },
    "timezone":  { "type": "string" }
  },
  "required": ["operation", "input"],
  "additionalProperties": false
}
param_template:  '{"operation":"parse","input":"{{input}}"}'
preconditions:   ["input must be a recognisable timestamp string"]
error_handling:  "unrecognised format → tool error"
category:        "time"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 14.6 — Leaf Skill `skill-time-parse` (class 1)

```
name:        "skill-time-parse"
description: "Use the time tool's parse operation to interpret a timestamp string. When the
             user provides a date or time in text form and you need a structured representation,
             or to convert it, call the ts-time-parse ToolSkill with the input string.
             Supports ISO 8601, RFC 2822, and common human-readable formats."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 14.7 — Recipe `time-parse` (class 21)

```
name:        "time-parse"
description: "Parse a timestamp string into a structured time value."
llm_call_required: false
tier:        0
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-time-parse>"],
    "label":   "Executor parses the timestamp string"
  }
]
intent_examples: [
  {"input": "parse this date string",            "class": 1},
  {"input": "what timestamp is 2024-01-15T10:30","class": 1},
  {"input": "interpret this date format",        "class": 2},
  {"input": "parse the timestamp from this log", "class": 2},
  {"input": "convert date string to timestamp",  "class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Step 14.8 — ToolSkill `ts-time-convert` (class 13)

```
name:        "ts-time-convert"
tool_name:   "time"
description: "Executor binding: convert a timestamp from one timezone to another. Operation:
             'convert'. Required: input (timestamp string). Optional: from_timezone (IANA,
             default UTC), to_timezone (IANA, default UTC)."
param_schema: {
  "type": "object",
  "properties": {
    "operation":    { "type": "string", "enum": ["convert"] },
    "input":        { "type": "string" },
    "from_timezone":{ "type": "string" },
    "to_timezone":  { "type": "string" }
  },
  "required": ["operation", "input"],
  "additionalProperties": false
}
param_template:  '{"operation":"convert","input":"{{input}}"}'
preconditions:   ["input must be a recognisable timestamp"]
error_handling:  "invalid timezone → tool error"
category:        "time"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 14.9 — Leaf Skill `skill-time-convert` (class 1)

```
name:        "skill-time-convert"
description: "Use the time tool's convert operation to convert a timestamp between timezones.
             When the user needs a time expressed in a different timezone, call the
             ts-time-convert ToolSkill with the input timestamp and the target timezone.
             IANA timezone names (e.g. 'America/New_York', 'Europe/Berlin') are required."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 14.10 — Recipe `time-convert` (class 21)

```
name:        "time-convert"
description: "Convert a timestamp to a different timezone."
llm_call_required: false
tier:        0
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-time-convert>"],
    "label":   "Executor converts timestamp to target timezone"
  }
]
intent_examples: [
  {"input": "convert this time to New York timezone",    "class": 2},
  {"input": "what is 3pm UTC in Tokyo",                  "class": 2},
  {"input": "timezone conversion for this timestamp",    "class": 2},
  {"input": "convert 14:00 London to Sydney time",       "class": 2},
  {"input": "what time is this in EST",                  "class": 2}
]
source: "system"
validation_status: "validated"
```


---

## Step 15 — `builtin.json` (JSON Operations)

> **Capability:** `builtin.json` · **Effect:** `read_only` · **Permission:** Allow
> **Operations:** parse, stringify, query, validate

---

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
    "operation": { "type": "string", "enum": ["parse","stringify","query","validate"] },
    "data":      { "description": "JSON string or JSON value to process" },
    "path":      { "type": "string", "description": "Dot/bracket path for query operation" }
  },
  "required": ["operation", "data"],
  "additionalProperties": false
}
param_template:  '{"operation":"{{operation}}","data":{{data}}}'
preconditions:   ["operation required", "data required"]
error_handling:  "invalid JSON for parse/query → tool error; path not found in query → null result"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 15.2 — ToolSkill `ts-json-query` (class 13)

```
name:        "ts-json-query"
tool_name:   "json"
description: "Executor binding for json query operation. Required: operation='query', data
             (JSON string or value), path (dot/bracket path expression). Returns the value
             at the given path, or null if not found."
param_schema: {
  "type": "object",
  "properties": {
    "operation": { "type": "string", "enum": ["query"] },
    "data":      {},
    "path":      { "type": "string" }
  },
  "required": ["operation", "data", "path"],
  "additionalProperties": false
}
param_template:  '{"operation":"query","data":{{data}},"path":"{{path}}"}'
preconditions:   ["data must be valid JSON", "path must not be empty"]
error_handling:  "invalid JSON → tool error; path not found → null (not a tool error)"
category:        "data"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 15.3 — Leaf Skill `skill-json-query` (class 1)

```
name:        "skill-json-query"
description: "Use the json tool's query operation to extract a specific value from a JSON
             structure. When you have a JSON response or document and need to extract a
             specific field or nested value, call the ts-json-query ToolSkill with the data
             and a dot-separated path (e.g. 'user.address.city' or 'items.0.name'). Returns
             null if the path does not exist. For complex extraction across multiple fields,
             consider using pc-json-extract-field PythonCode instead."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 15.4 — Recipe `json-query` (class 21)

```
name:        "json-query"
description: "Extract a value from a JSON structure by path."
llm_call_required: false
tier:        0
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-json-query>"],
    "label":   "Executor extracts value at JSON path"
  }
]
intent_examples: [
  {"input": "extract the user name from this JSON",  "class": 2},
  {"input": "get the value at this JSON path",       "class": 1},
  {"input": "query this JSON for the id field",      "class": 2},
  {"input": "json query items.0.name",               "class": 1},
  {"input": "extract nested field from API response","class": 2}
]
source: "system"
validation_status: "validated"
```

---

### Step 15.5 — ToolSkill `ts-json-stringify` (class 13)

```
name:        "ts-json-stringify"
tool_name:   "json"
description: "Executor binding for json stringify and parse operations. Required: operation
             ('stringify' or 'parse'), data (JSON value to stringify, or JSON string to
             parse). Stringify converts a value to a formatted JSON string; parse converts
             a JSON string to a structured value."
param_schema: {
  "type": "object",
  "properties": {
    "operation": { "type": "string", "enum": ["stringify","parse"] },
    "data":      {}
  },
  "required": ["operation", "data"],
  "additionalProperties": false
}
param_template:  '{"operation":"{{operation}}","data":{{data}}}'
preconditions:   ["data must be valid for the selected operation"]
error_handling:  "invalid JSON string for parse → tool error"
category:        "data"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 15.6 — Leaf Skill `skill-json-stringify` (class 1)

```
name:        "skill-json-stringify"
description: "Use the json tool to convert between JSON strings and structured values.
             When you need to format a JSON object as a printable string for the user or
             for a write operation, call ts-json-stringify with operation='stringify'.
             When you have a JSON string from a tool response and need to work with it as
             a structured value, use operation='parse'. For validation use operation='validate'."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 15.7 — Recipe `json-stringify` (class 21)

```
name:        "json-stringify"
description: "Stringify or parse a JSON value."
llm_call_required: false
tier:        0
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-json-stringify>"],
    "label":   "Executor serializes or deserializes JSON"
  }
]
intent_examples: [
  {"input": "format this as JSON",                "class": 1},
  {"input": "stringify this object",              "class": 1},
  {"input": "parse this JSON string",             "class": 1},
  {"input": "pretty print this JSON",             "class": 1},
  {"input": "convert this to a JSON string",      "class": 2}
]
source: "system"
validation_status: "validated"
```


---

## Step 16 — `builtin.skill_list/install/remove` (Skill Management)

> Three capabilities sharing the management domain: `builtin.skill_list`, `builtin.skill_install`, `builtin.skill_remove`

---

### Step 16.1 — Tool rows (class 0)

```
--- skill_list ---
name:            "skill_list"
description:     "List all installed filesystem skills."
capability_id:   "builtin.skill_list"
effect_type:     "read_only"
param_schema:    { "type": "object", "properties": {}, "additionalProperties": false }
param_template:  '{}'
preconditions:   []
error_handling:  "returns empty list when no skills are installed"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"

--- skill_install ---
name:            "skill_install"
description:     "Install a skill from inline SKILL.md content or a remote URL (HTTPS, ZIP,
                 or GitHub skill repository). Optional name overrides the skill document name."
capability_id:   "builtin.skill_install"
effect_type:     "write_filesystem"
param_schema: {
  "type": "object",
  "properties": {
    "name":    { "type": "string", "description": "Skill name override" },
    "content": { "type": "string", "description": "Raw SKILL.md content or Markdown" },
    "url":     { "type": "string", "description": "HTTPS URL to a SKILL.md, ZIP, or GitHub repository" }
  },
  "oneOf": [{ "required": ["content"] }, { "required": ["url"] }],
  "additionalProperties": false
}
param_template:  '{"content":"{{content}}"}'
preconditions:   ["one of content or url required"]
error_handling:  "URL fetch failure → tool error; invalid SKILL.md format → tool error"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"

--- skill_remove ---
name:            "skill_remove"
description:     "Remove an installed skill by name."
capability_id:   "builtin.skill_remove"
effect_type:     "write_filesystem"
param_schema: {
  "type": "object",
  "properties": {
    "name": { "type": "string", "description": "Name of the installed skill to remove" }
  },
  "required": ["name"],
  "additionalProperties": false
}
param_template:  '{"name":"{{name}}"}'
preconditions:   ["name must not be empty"]
error_handling:  "skill not found → tool error"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 16.2 — ToolSkills (class 13)

```
--- ts-skill-list ---
name:        "ts-skill-list"
tool_name:   "skill_list"
description: "Executor binding for skill_list. No parameters. Returns the list of all
             installed filesystem skills."
param_schema:   { "type": "object", "properties": {}, "additionalProperties": false }
param_template: '{}'
preconditions:  []
error_handling: "empty list when no skills installed"
category:       "management"
consumer_tags:  ["00:rusty", "05:validator"]
source:         "system"
validation_status: "validated"

--- ts-skill-install ---
name:        "ts-skill-install"
tool_name:   "skill_install"
description: "Executor binding for skill_install. Provide content (inline SKILL.md) or url
             (remote). Optional name. Installs the skill into the filesystem skills directory."
param_schema: {
  "type": "object",
  "properties": {
    "name":    { "type": "string" },
    "content": { "type": "string" },
    "url":     { "type": "string" }
  },
  "oneOf": [{ "required": ["content"] }, { "required": ["url"] }],
  "additionalProperties": false
}
param_template: '{"content":"{{content}}"}'
preconditions:  ["content or url required"]
error_handling: "URL fetch failure → tool error"
category:       "management"
consumer_tags:  ["00:rusty", "05:validator"]
source:         "system"
validation_status: "validated"

--- ts-skill-remove ---
name:        "ts-skill-remove"
tool_name:   "skill_remove"
description: "Executor binding for skill_remove. Required: name. Removes the named skill
             from the filesystem skills directory."
param_schema: {
  "type": "object",
  "properties": {
    "name": { "type": "string" }
  },
  "required": ["name"],
  "additionalProperties": false
}
param_template: '{"name":"{{name}}"}'
preconditions:  ["name must not be empty"]
error_handling: "not found → tool error"
category:       "management"
consumer_tags:  ["00:rusty", "05:validator"]
source:         "system"
validation_status: "validated"
```

---

### Step 16.3 — Leaf Skills (class 1)

```
--- skill-skill-list ---
name:        "skill-skill-list"
description: "Use skill_list to show all installed filesystem skills. When the user wants
             to see what skills are currently installed, call ts-skill-list. Present the
             returned skill names and descriptions in a readable list."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"

--- skill-skill-install ---
name:        "skill-skill-install"
description: "Use skill_install to install a new filesystem skill. When the user wants to
             add a skill — from inline content or a URL — call ts-skill-install. If the
             user provides raw SKILL.md content, pass it as content. If they provide a
             URL (GitHub, ZIP, or direct SKILL.md link), pass it as url. The name parameter
             is optional and overrides the skill document name."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"

--- skill-skill-remove ---
name:        "skill-skill-remove"
description: "Use skill_remove to uninstall a filesystem skill. When the user wants to
             remove a skill, call ts-skill-remove with the exact skill name. Use
             skill-skill-list first to find the correct name if uncertain."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 16.4 — Domain Skill `skill-skills` (class 2)

```
name:        "skill-skills"
description: "The skill management domain provides three tools for managing installed
             filesystem skills:
             — skill-skill-list: List all installed skills.
             — skill-skill-install: Install a skill from inline content or URL.
             — skill-skill-remove: Remove an installed skill by name.

             Skill management affects the filesystem skills directory. Installed skills
             appear at the next session start. Always list skills before removing to
             confirm the exact name."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 16.5 — Recipes (class 21)

```
--- skill-list (recipe) ---
name:        "skill-list"
description: "List all installed filesystem skills."
llm_call_required: false
tier:        0
step_descriptions: [
  { "step_id": "step-1", "type": "component", "channel": "rust",
    "include": ["<uuid:ts-skill-list>"], "label": "Executor lists installed skills" }
]
intent_examples: [
  {"input": "what skills are installed",          "class": 1},
  {"input": "list my skills",                     "class": 1},
  {"input": "show installed skills",              "class": 1},
  {"input": "skill list",                         "class": 1},
  {"input": "what filesystem skills do I have",   "class": 2}
]
source: "system"
validation_status: "validated"

--- skill-install (recipe) ---
name:        "skill-install"
description: "Install a skill from inline content or URL."
llm_call_required: true
tier:        1
step_descriptions: [
  { "step_id": "step-1", "type": "llm", "label": "LLM extracts the skill content or URL from user input" },
  { "step_id": "step-2", "type": "component", "channel": "rust",
    "include": ["<uuid:ts-skill-install>"], "label": "Executor installs the skill" }
]
intent_examples: [
  {"input": "install this skill from URL",       "class": 2},
  {"input": "add a new skill from GitHub",       "class": 2},
  {"input": "install skill from this content",   "class": 2},
  {"input": "add this SKILL.md",                 "class": 2},
  {"input": "install skill",                     "class": 1}
]
source: "system"
validation_status: "validated"

--- skill-remove (recipe) ---
name:        "skill-remove"
description: "Remove an installed skill by name."
llm_call_required: true
tier:        1
step_descriptions: [
  { "step_id": "step-1", "type": "component", "channel": "rust",
    "include": ["<uuid:ts-skill-list>"], "label": "Executor lists skills to confirm name" },
  { "step_id": "step-2", "type": "llm", "label": "LLM identifies the skill name to remove" },
  { "step_id": "step-3", "type": "component", "channel": "rust",
    "include": ["<uuid:ts-skill-remove>"], "label": "Executor removes the skill" }
]
intent_examples: [
  {"input": "remove this skill",                 "class": 2},
  {"input": "uninstall skill named foo",         "class": 2},
  {"input": "delete the git-helper skill",       "class": 2},
  {"input": "remove installed skill",            "class": 1},
  {"input": "uninstall skill",                   "class": 1}
]
source: "system"
validation_status: "validated"
```


---

## Step 17 — `builtin.trigger_create/list/remove` (Trigger Management)

> Three capabilities: `builtin.trigger_create`, `builtin.trigger_list`, `builtin.trigger_remove`

---

### Step 17.1 — Tool rows (class 0)

```
--- trigger_create ---
name:            "trigger_create"
description:     "Create a scheduled trigger that fires a prompt on a cron schedule. Name,
                 prompt, and cron expression are required. Cron must have a minimum interval
                 of one minute."
capability_id:   "builtin.trigger_create"
effect_type:     "mixed"
param_schema: {
  "type": "object",
  "properties": {
    "name":   { "type": "string",  "description": "Human-readable trigger name (max 256 bytes)" },
    "prompt": { "type": "string",  "description": "Prompt submitted when the trigger fires (max 32768 bytes)" },
    "cron":   { "type": "string",  "description": "Five-, six-, or seven-field cron expression (min interval 1 minute)" }
  },
  "required": ["name", "prompt", "cron"],
  "additionalProperties": false
}
param_template:  '{"name":"{{name}}","prompt":"{{prompt}}","cron":"{{cron}}"}'
preconditions:   ["cron must be a valid expression with ≥1 minute interval"]
error_handling:  "invalid cron → tool error; name or prompt too long → tool error"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"

--- trigger_list ---
name:            "trigger_list"
description:     "List all scheduled triggers. Optional limit parameter (max 100, default 100)."
capability_id:   "builtin.trigger_list"
effect_type:     "read_only"
param_schema: {
  "type": "object",
  "properties": {
    "limit": { "type": "integer", "minimum": 0, "maximum": 100 }
  },
  "additionalProperties": false
}
param_template:  '{}'
preconditions:   []
error_handling:  "returns empty list when no triggers are scheduled"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"

--- trigger_remove ---
name:            "trigger_remove"
description:     "Remove a scheduled trigger by its ID."
capability_id:   "builtin.trigger_remove"
effect_type:     "mixed"
param_schema: {
  "type": "object",
  "properties": {
    "trigger_id": { "type": "string", "description": "Trigger id from trigger_create or trigger_list" }
  },
  "required": ["trigger_id"],
  "additionalProperties": false
}
param_template:  '{"trigger_id":"{{trigger_id}}"}'
preconditions:   ["trigger_id must not be empty"]
error_handling:  "trigger not found → tool error"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 17.2 — ToolSkills (class 13)

```
--- ts-trigger-create ---
name:        "ts-trigger-create"
tool_name:   "trigger_create"
description: "Executor binding for trigger_create. Required: name, prompt, cron. Creates a
             scheduled trigger. Cron: standard 5/6/7-field expression, minimum 1-minute
             interval. Returns the assigned trigger_id."
param_schema: {
  "type": "object",
  "properties": {
    "name":   { "type": "string" },
    "prompt": { "type": "string" },
    "cron":   { "type": "string" }
  },
  "required": ["name", "prompt", "cron"],
  "additionalProperties": false
}
param_template: '{"name":"{{name}}","prompt":"{{prompt}}","cron":"{{cron}}"}'
preconditions:  ["cron minimum interval is 1 minute"]
error_handling: "invalid cron → tool error"
category:       "management"
consumer_tags:  ["00:rusty", "05:validator"]
source:         "system"
validation_status: "validated"

--- ts-trigger-list ---
name:        "ts-trigger-list"
tool_name:   "trigger_list"
description: "Executor binding for trigger_list. Optional: limit (0–100, default 100).
             Returns all scheduled triggers with their ids, names, cron expressions, and
             next-fire times."
param_schema: {
  "type": "object",
  "properties": {
    "limit": { "type": "integer", "minimum": 0, "maximum": 100 }
  },
  "additionalProperties": false
}
param_template: '{}'
preconditions:  []
error_handling: "empty list when no triggers exist"
category:       "management"
consumer_tags:  ["00:rusty", "05:validator"]
source:         "system"
validation_status: "validated"

--- ts-trigger-remove ---
name:        "ts-trigger-remove"
tool_name:   "trigger_remove"
description: "Executor binding for trigger_remove. Required: trigger_id (from trigger_create
             or trigger_list). Removes the scheduled trigger permanently."
param_schema: {
  "type": "object",
  "properties": {
    "trigger_id": { "type": "string" }
  },
  "required": ["trigger_id"],
  "additionalProperties": false
}
param_template: '{"trigger_id":"{{trigger_id}}"}'
preconditions:  ["trigger_id must be a valid UUID or trigger identifier"]
error_handling: "not found → tool error"
category:       "management"
consumer_tags:  ["00:rusty", "05:validator"]
source:         "system"
validation_status: "validated"
```

---

### Step 17.3 — Leaf Skills (class 1)

```
--- skill-trigger-create ---
name:        "skill-trigger-create"
description: "Use trigger_create to schedule an automated recurring task. When the user
             wants the agent to run a prompt on a schedule — daily digest, weekly report,
             hourly check — call ts-trigger-create with name, prompt, and cron expression.
             Standard 5-field cron (minute hour day month weekday) is supported. The prompt
             should be self-contained since it will run in a fresh session."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"

--- skill-trigger-list ---
name:        "skill-trigger-list"
description: "Use trigger_list to show all scheduled triggers. When the user wants to see
             their active scheduled tasks, call ts-trigger-list. Present each trigger's
             name, cron schedule, and next-fire time clearly."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"

--- skill-trigger-remove ---
name:        "skill-trigger-remove"
description: "Use trigger_remove to cancel a scheduled trigger. When the user wants to
             stop a recurring task, call ts-trigger-remove with the trigger_id. Use
             skill-trigger-list first to confirm the correct trigger_id."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 17.4 — Domain Skill `skill-triggers` (class 2)

```
name:        "skill-triggers"
description: "The trigger management domain provides three tools for scheduling automated
             recurring tasks:
             — skill-trigger-create: Create a scheduled trigger (name, prompt, cron).
             — skill-trigger-list: List all active scheduled triggers.
             — skill-trigger-remove: Remove a trigger by ID.

             Triggers fire the prompt in a fresh session on the cron schedule. The prompt
             must be self-contained. Minimum cron interval is 1 minute. Always list triggers
             before removing to confirm the correct trigger_id."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 17.5 — Recipes (class 21)

```
--- trigger-list (recipe) ---
name:        "trigger-list"
description: "List all scheduled triggers."
llm_call_required: false
tier:        0
step_descriptions: [
  { "step_id": "step-1", "type": "component", "channel": "rust",
    "include": ["<uuid:ts-trigger-list>"], "label": "Executor lists scheduled triggers" }
]
intent_examples: [
  {"input": "show my scheduled triggers",         "class": 1},
  {"input": "list all triggers",                  "class": 1},
  {"input": "what recurring tasks are scheduled", "class": 2},
  {"input": "trigger list",                       "class": 1},
  {"input": "show me scheduled automations",      "class": 2}
]
source: "system"
validation_status: "validated"

--- trigger-create (recipe) ---
name:        "trigger-create"
description: "Create a scheduled trigger that fires a prompt on a cron schedule."
llm_call_required: true
tier:        1
step_descriptions: [
  { "step_id": "step-1", "type": "llm",
    "label": "LLM extracts name, prompt, and cron expression from user request" },
  { "step_id": "step-2", "type": "component", "channel": "rust",
    "include": ["<uuid:ts-trigger-create>"], "label": "Executor creates the trigger" }
]
intent_examples: [
  {"input": "create a daily trigger to summarise my day",   "class": 3},
  {"input": "schedule a weekly report every Monday",        "class": 3},
  {"input": "run this prompt every hour",                   "class": 2},
  {"input": "set up a cron trigger",                        "class": 2},
  {"input": "automate this task on a schedule",             "class": 3}
]
source: "system"
validation_status: "validated"

--- trigger-remove (recipe) ---
name:        "trigger-remove"
description: "Remove a scheduled trigger by ID."
llm_call_required: true
tier:        1
step_descriptions: [
  { "step_id": "step-1", "type": "component", "channel": "rust",
    "include": ["<uuid:ts-trigger-list>"], "label": "Executor lists triggers to confirm ID" },
  { "step_id": "step-2", "type": "llm", "label": "LLM identifies the trigger_id to remove" },
  { "step_id": "step-3", "type": "component", "channel": "rust",
    "include": ["<uuid:ts-trigger-remove>"], "label": "Executor removes the trigger" }
]
intent_examples: [
  {"input": "cancel my daily trigger",            "class": 2},
  {"input": "remove this scheduled task",         "class": 2},
  {"input": "delete trigger named weekly-report", "class": 2},
  {"input": "stop the scheduled automation",      "class": 2},
  {"input": "remove trigger",                     "class": 1}
]
source: "system"
validation_status: "validated"
```


---

## Step 18 — `builtin.spawn_subagent` (Spawn Sub-Agent)

> **Capability:** `builtin.spawn_subagent` · **Effect:** `mixed` · **Permission:** Ask
> **§spawn_subagent-guard:** any Recipe using this capability is `llm_call_required: true` — **never Tier 0**.

---

### Step 18.1 — Tool row (class 0)

```
name:            "spawn_subagent"
description:     "Spawn a child sub-agent run with a specific flavor and task. The child
                 run executes in its own isolated context and returns a summary to the parent.
                 Use for clearly self-contained parallel or sequential side work whose result
                 can be summarised back."
capability_id:   "builtin.spawn_subagent"
effect_type:     "mixed"
param_schema: {
  "type": "object",
  "properties": {
    "flavor_id": { "type": "string", "enum": ["general","researcher","coder","explorer"],
                   "description": "Subagent flavor: general (read/search), researcher (+web), coder (read/write/shell), explorer (read/search, deep analysis)" },
    "task":      { "type": "string", "description": "Task for the child subagent run" },
    "handoff":   { "type": "string", "description": "Optional context to pass to the child" }
  },
  "required": ["flavor_id", "task"],
  "additionalProperties": false
}
param_template:  '{"flavor_id":"{{flavor_id}}","task":"{{task}}"}'
preconditions:   ["task must be self-contained", "flavor_id must be a valid enum value"]
error_handling:  "child run failure → tool error with summary"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 18.2 — ToolSkill `ts-spawn-subagent` (class 13)

```
name:        "ts-spawn-subagent"
tool_name:   "spawn_subagent"
description: "Executor binding for spawn_subagent. Required: flavor_id (general | researcher
             | coder | explorer), task (self-contained instruction). Optional: handoff
             (context string). Flavors: general=read/search; researcher=read/search/web;
             coder=read/write/shell; explorer=read/search/deep-analysis."
param_schema: {
  "type": "object",
  "properties": {
    "flavor_id": { "type": "string", "enum": ["general","researcher","coder","explorer"] },
    "task":      { "type": "string" },
    "handoff":   { "type": "string" }
  },
  "required": ["flavor_id", "task"],
  "additionalProperties": false
}
param_template:  '{"flavor_id":"{{flavor_id}}","task":"{{task}}"}'
preconditions:   ["task must be a self-contained goal the child can complete independently"]
error_handling:  "child run error → tool error"
category:        "process"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 18.3 — Leaf Skill `skill-spawn-subagent` (class 1)

```
name:        "skill-spawn-subagent"
description: "Use spawn_subagent to delegate a clearly self-contained sub-task to a child
             agent. When a task requires deep parallel exploration, focused code execution,
             or side work whose result can be summarised back, call ts-spawn-subagent.
             Choose the right flavor: 'general' for read/search tasks; 'researcher' for
             tasks requiring web search; 'coder' for tasks requiring file reads, writes, or
             shell commands; 'explorer' for deep analysis without writes. The task must be
             self-contained — do not use subagents for trivial lookups or operations that
             can be done in one direct tool call. Use handoff to pass relevant context."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 18.4 — Domain Skill `skill-subagent` (class 2)

```
name:        "skill-subagent"
description: "The subagent domain provides one tool for spawning isolated child agent runs:
             — skill-spawn-subagent: Spawn a child agent with a self-contained task.

             Use subagents for work that is clearly bounded, can run independently, and
             whose result can be summarised back to the parent. Never use a subagent for
             simple tool calls or trivial lookups.

             §spawn_subagent-guard: All recipes using spawn_subagent must be Tier 1
             (llm_call_required: true). The LLM must always choose and frame the subagent
             task — deterministic dispatch is not safe here."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 18.5 — Recipe `subagent-spawn` (class 21)

```
name:        "subagent-spawn"
description: "Spawn a child sub-agent for a self-contained task."
llm_call_required: true
tier:        1
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "llm",
    "label":   "LLM defines the subagent flavor, task, and any handoff context"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-spawn-subagent>"],
    "label":   "Executor spawns child subagent run"
  }
]
intent_examples: [
  {"input": "spawn a subagent to research this topic",    "class": 3},
  {"input": "delegate this coding task to a subagent",    "class": 3},
  {"input": "run this analysis in a child agent",         "class": 2},
  {"input": "spawn a researcher subagent",                "class": 2},
  {"input": "use a coder subagent to fix this",           "class": 3}
]
source: "system"
validation_status: "validated"
```

---

## Step 19 — `builtin.echo` (Echo Message)

> **Capability:** `builtin.echo` · **Effect:** `read_only` · **Permission:** Allow

---

### Step 19.1 — Tool row (class 0)

```
name:            "echo"
description:     "Echo a message back. Used for testing capability dispatch and verifying
                 that the tool pipeline is working. Returns the message unchanged."
capability_id:   "builtin.echo"
effect_type:     "read_only"
param_schema: {
  "type": "object",
  "properties": {
    "message": { "type": "string", "description": "Message to echo" }
  },
  "required": ["message"],
  "additionalProperties": false
}
param_template:  '{"message":"{{message}}"}'
preconditions:   ["message must not be empty"]
error_handling:  "none"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 19.2 — ToolSkill `ts-echo` (class 13)

> No standalone recipe for echo — it is a diagnostic utility only.

```
name:        "ts-echo"
tool_name:   "echo"
description: "Executor binding for echo. Required: message (string). Returns the message
             unchanged. Used for testing capability dispatch; not for agent tasks."
param_schema: {
  "type": "object",
  "properties": {
    "message": { "type": "string" }
  },
  "required": ["message"],
  "additionalProperties": false
}
param_template:  '{"message":"{{message}}"}'
preconditions:   ["message must not be empty"]
error_handling:  "none"
category:        "management"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```


---

## Step 20 — Web Search (via HTTP)

> Web search is composed from `builtin.http` with a search engine URL. No dedicated capability.
> The ToolSkill and Recipe are compositions over `ts-http-fetch`.

---

### Step 20.1 — ToolSkill `ts-web-search` (class 13)

> Reuses the `http` Tool (capability_id `builtin.http`) with a pre-configured URL template
> for a search engine API. Not a new Tool — a binding-level composition.

```
name:        "ts-web-search"
tool_name:   "http"
description: "Executor binding for web search via HTTP. Issues a GET request to a configured
             search API endpoint. Required param: query (URL-encoded search query string).
             The binding assembles the URL as: https://ddg.search.brassclaw.dev/search?q={{query}}&format=json
             (operator-configurable via the search_api_base setting). Returns JSON with
             results array."
param_schema: {
  "type": "object",
  "properties": {
    "url":   { "type": "string", "description": "Search API URL with query embedded" },
    "method":{ "type": "string", "enum": ["get"], "default": "get" }
  },
  "required": ["url"],
  "additionalProperties": false
}
param_template:  '{"url":"https://ddg.search.brassclaw.dev/search?q={{query}}&format=json","method":"get"}'
preconditions:   ["network egress must be permitted by policy"]
error_handling:  "network failure → tool error; empty results → empty list"
category:        "network"
consumer_tags:   ["00:rusty", "05:validator"]
source:          "system"
validation_status: "validated"
```

---

### Step 20.2 — PythonCode `pc-web-search-extract` (class 22)

> Pure logic: slices the top-N results from a JSON search response.
> No I/O, no imports, no network.

```
name:        "pc-web-search-extract"
description: "Pure-logic helper: extracts the top results from a web search JSON response.
             Input: results (list of dicts with title/url/body keys), limit (integer).
             Output: extracted list of {title, url, snippet} dicts."
content: |
  # No I/O, no imports. IBS bakes in 'results' and 'limit' before execution.
  results = {{vars.slot0}}
  limit = {{vars.slot1}}
  extracted = []
  if isinstance(results, list):
      for item in results[:limit]:
          extracted.append({
              "title":   item.get("title", ""),
              "url":     item.get("href", item.get("url", "")),
              "snippet": item.get("body", item.get("snippet", ""))
          })
  result = {"results": extracted, "count": len(extracted)}
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 20.3 — Leaf Skill `skill-web-search` (class 1)

```
name:        "skill-web-search"
description: "Use the http tool to perform a web search. When the user asks you to search
             the web, look something up online, or find current information, call
             ts-web-search with the user's query encoded in the URL. Parse the returned
             JSON with pc-web-search-extract to get the top results. Summarise the results
             and provide the most relevant URLs and snippets. For very large responses,
             use ts-http-save to write the response to a file before parsing."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 20.4 — Recipe `web-search` (class 21)

```
name:        "web-search"
description: "Search the web and extract top results."
llm_call_required: true
tier:        1
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "llm",
    "label":   "LLM formulates the search query from user intent"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-web-search>"],
    "label":   "Executor fetches search results from API"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-web-search-extract>"],
    "label":   "PythonCode extracts top results from JSON response"
  }
]
intent_examples: [
  {"input": "search the web for this topic",           "class": 2},
  {"input": "what is the latest news about X",         "class": 2},
  {"input": "look up this term online",                "class": 2},
  {"input": "find recent articles about this",         "class": 3},
  {"input": "web search",                              "class": 1}
]
source: "system"
validation_status: "validated"
```


---

## Step 21 — Session Memory (Composition Pattern)

> Session memory bridges the gap between conversation history (not agent-queryable in Kohai)
> and persistent memory. This step authors the PythonCode helpers, leaf skill, domain skill,
> and Recipe `session-summarize` that together extract and persist session facts at session end.

---

### Step 21.1 — PythonCode `pc-session-facts-parse` (class 22)

> Pure logic: parses a bullet-point facts list from an LLM-extracted text.
> No I/O, no imports, no network.

```
name:        "pc-session-facts-parse"
description: "Pure-logic helper: parses a bullet-point list of facts from an LLM-extracted
             text block. Input: raw_text (string). Output: facts (list of strings, one per
             non-empty bullet line)."
content: |
  # No I/O, no imports. IBS bakes in 'raw_text' before execution.
  raw_text = "{{vars.slot0}}"
  facts = []
  for line in raw_text.split("\n"):
      stripped = line.strip()
      if stripped.startswith(("-", "*", "•", "·")):
          fact = stripped.lstrip("-*•· ").strip()
          if fact:
              facts.append(fact)
      elif stripped and not stripped.startswith("#"):
          facts.append(stripped)
  result = {"facts": facts, "count": len(facts)}
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 21.2 — PythonCode `pc-session-summary-render` (class 22)

> Pure logic: renders a session summary for memory_write. Timestamp supplied as a param
> (never via datetime — no I/O, no imports).

```
name:        "pc-session-summary-render"
description: "Pure-logic helper: renders a formatted session summary string for appending
             to memory. Input: facts (list of strings), session_topic (string), timestamp_str
             (string — caller supplies pre-fetched timestamp). Output: summary (string)."
content: |
  # No I/O, no imports, no datetime. IBS bakes in all values before execution.
  facts = {{vars.slot0}}
  session_topic = "{{vars.slot1}}"
  timestamp_str = "{{vars.slot2}}"
  lines = [f"## Session: {session_topic} [{timestamp_str}]", ""]
  if isinstance(facts, list) and facts:
      lines.append("**Key facts:**")
      for fact in facts:
          lines.append(f"- {fact}")
  else:
      lines.append("No key facts recorded.")
  summary = "\n".join(lines) + "\n"
  result = {"summary": summary}
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 21.3 — Leaf Skill `skill-session-write` (class 1)

```
name:        "skill-session-write"
description: "Write a session summary to persistent memory. After extracting session facts
             (typically via an LLM step), call ts-time-now to get the current timestamp,
             call pc-session-summary-render to format the summary, and then call
             ts-memory-write with target='memory' and append=true to persist it. This
             pattern ensures all session knowledge is captured before the conversation ends."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 21.4 — Domain Skill `skill-session-memory` (class 2)

```
name:        "skill-session-memory"
description: "The session memory domain bridges the gap between the current conversation
             and persistent memory. Kohai stores conversation turns forensically but the
             agent cannot query them directly. Use these components to extract and persist
             session facts before a session ends:

             — skill-session-write: Write a formatted session summary to memory.
             — pc-session-facts-parse: Parse bullet-point facts from an LLM response.
             — pc-session-summary-render: Render a formatted summary with timestamp.

             Pattern for session sync:
             1. Ask the LLM to extract key facts from the session as a bullet list.
             2. Parse the facts with pc-session-facts-parse.
             3. Get the current time with ts-time-now.
             4. Render the summary with pc-session-summary-render.
             5. Write to memory with ts-memory-write (target='memory', append=true).

             This is also the pattern used by the session-sync Action (class 16)."
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 21.5 — Recipe `session-summarize` (class 21)

```
name:        "session-summarize"
description: "Extract key facts from this session and save them to persistent memory."
llm_call_required: true
tier:        1
step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "llm",
    "label":   "LLM extracts key facts and decisions from the session as a bullet list"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-session-facts-parse>"],
    "label":   "PythonCode parses bullet list into structured facts"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-time-now>"],
    "label":   "Executor gets current timestamp"
  },
  {
    "step_id": "step-4",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-session-summary-render>"],
    "label":   "PythonCode renders formatted session summary"
  },
  {
    "step_id": "step-5",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-memory-write>"],
    "label":   "Executor writes summary to MEMORY.md"
  }
]
intent_examples: [
  {"input": "save this session to memory",              "class": 2},
  {"input": "summarise what we did and save it",        "class": 3},
  {"input": "write session summary to memory",          "class": 2},
  {"input": "remember what happened in this session",   "class": 2},
  {"input": "end of session memory sync",               "class": 1}
]
source: "system"
validation_status: "validated"
```

---

## Step 22 — Action `session-sync` (class 16)

> Deterministic no-LLM procedure. Uses `execute_action_procedure` path — not a Recipe.
> Steps are fixed; no LLM turn is required.

```
name:        "session-sync"
class:       16
description: "Deterministic end-of-session procedure: reads MEMORY.md for context, gets the
             current timestamp, and appends a session checkpoint marker to MEMORY.md and
             daily_log. No LLM call required — fully deterministic. Triggered automatically
             at session end or manually."
procedure_steps: [
  {
    "step_id": "step-1",
    "action":  "call_tool",
    "tool_skill": "ts-memory-read",
    "params":  {"path": "MEMORY.md"},
    "label":   "Read current MEMORY.md for context"
  },
  {
    "step_id": "step-2",
    "action":  "call_tool",
    "tool_skill": "ts-time-now",
    "params":  {},
    "label":   "Get current timestamp"
  },
  {
    "step_id": "step-3",
    "action":  "call_tool",
    "tool_skill": "ts-memory-write",
    "params":  {"target": "daily_log", "append": true,
                "content": "## Session checkpoint [{{step-2.timestamp}}]\n\nSession synced."},
    "label":   "Write checkpoint to daily_log"
  }
]
llm_call_required: false
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```


---

## Steps 23–27 — Five ExtensionCatalogues (class 23)

> One catalogue per cognitive domain. Each `overview_doc` describes the domain's purpose
> and when to use its tools. `child_component_ids` lists the UUIDs of all components in
> the domain. `task_groups` clusters them by sub-purpose.

---

### Step 23 — ExtensionCatalogue `builtin-filesystem`

```
name:        "builtin-filesystem"
class:       23
overview_doc: |
  # Filesystem Domain

  The filesystem domain provides six scoped tools for reading, writing, searching, and
  modifying files within the active workspace mount. All paths are scoped — operations
  outside the mount boundary are rejected before execution.

  **Tools in this domain:**
  - `read_file` — read a file's full or partial content (paginated by line range)
  - `write_file` — create or fully replace a file
  - `apply_patch` — targeted search-replace edit within a file (preferred over write_file for changes)
  - `list_dir` — list directory contents (supports recursive listing)
  - `glob` — find files by name pattern (**/*.rs, src/**/*.ts, etc.)
  - `grep` — search file contents by regex (content, files-with-matches, or count mode)

  **Decision guide:**
  - Exploring structure → list_dir or glob
  - Searching content → grep
  - Reading a specific file → read_file
  - Targeted change → apply_patch
  - Full rewrite or new file → write_file
task_groups: [
  {
    "group_name": "Reading",
    "child_component_ids": ["<uuid:ts-read-file>", "<uuid:skill-read-file>",
                            "<uuid:file-read>", "<uuid:ts-list-dir>",
                            "<uuid:skill-list-dir>", "<uuid:file-list>"]
  },
  {
    "group_name": "Finding",
    "child_component_ids": ["<uuid:ts-glob>", "<uuid:skill-glob>", "<uuid:file-glob>",
                            "<uuid:ts-grep>", "<uuid:skill-grep>", "<uuid:file-grep>"]
  },
  {
    "group_name": "Writing",
    "child_component_ids": ["<uuid:ts-write-file>", "<uuid:skill-write-file>", "<uuid:file-write>",
                            "<uuid:ts-apply-patch>", "<uuid:skill-apply-patch>", "<uuid:file-patch>"]
  },
  {
    "group_name": "Domain",
    "child_component_ids": ["<uuid:skill-filesystem>"]
  }
]
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 24 — ExtensionCatalogue `builtin-network`

```
name:        "builtin-network"
class:       23
overview_doc: |
  # Network Domain

  The network domain provides HTTP capabilities for making outbound requests and
  performing web searches.

  **Tools in this domain:**
  - `http` — make an HTTP/HTTPS request and receive the response inline (≤256 KiB)
  - `http.save` — make an HTTP/HTTPS request and save the response to a file (≤10 MiB)

  **Composed patterns:**
  - web search — issues a GET to a search API and extracts the top results

  **Decision guide:**
  - Small API response needed inline → http (skill-http-fetch)
  - Large download or must save to disk → http.save (skill-http-save)
  - Web search query → web-search recipe (ts-web-search + pc-web-search-extract)
  - Non-2xx status codes are NOT tool errors; inspect the status field in the output.
task_groups: [
  {
    "group_name": "HTTP",
    "child_component_ids": ["<uuid:ts-http-fetch>", "<uuid:skill-http-fetch>",
                            "<uuid:http-get>", "<uuid:http-post>",
                            "<uuid:ts-http-save>", "<uuid:skill-http-save>", "<uuid:http-save>"]
  },
  {
    "group_name": "Web Search",
    "child_component_ids": ["<uuid:ts-web-search>", "<uuid:pc-web-search-extract>",
                            "<uuid:skill-web-search>", "<uuid:web-search>"]
  },
  {
    "group_name": "Helpers",
    "child_component_ids": ["<uuid:pc-http-status-check>", "<uuid:pc-json-extract-field>"]
  },
  {
    "group_name": "Domain",
    "child_component_ids": ["<uuid:skill-http>"]
  }
]
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 25 — ExtensionCatalogue `builtin-memory`

```
name:        "builtin-memory"
class:       23
overview_doc: |
  # Memory Domain

  The memory domain provides four tools for working with the agent's persistent memory
  store — a filesystem-backed document repository that persists across sessions.

  **Tools in this domain:**
  - `memory_search` — semantic search across all memory documents
  - `memory_read` — read a specific document by exact path
  - `memory_write` — write or append to a document (targets: memory, daily_log, heartbeat, bootstrap, or any path)
  - `memory_tree` — browse the directory structure of the memory store

  **Session memory pattern:**
  At session end, use the `session-summarize` recipe to extract key facts from the
  current conversation and persist them to MEMORY.md. The `session-sync` Action
  provides a deterministic no-LLM checkpoint procedure.

  **Decision guide:**
  - Recall by topic → memory_search
  - Read known file → memory_read (MEMORY.md, HEARTBEAT.md, etc.)
  - Log progress → memory_write (daily_log, default)
  - Update main context → memory_write (target='memory')
  - Discover structure → memory_tree
task_groups: [
  {
    "group_name": "Reading",
    "child_component_ids": ["<uuid:ts-memory-search>", "<uuid:skill-memory-search>", "<uuid:memory-search>",
                            "<uuid:ts-memory-read>", "<uuid:skill-memory-read>", "<uuid:memory-read>",
                            "<uuid:ts-memory-tree>", "<uuid:skill-memory-tree>", "<uuid:memory-tree>"]
  },
  {
    "group_name": "Writing",
    "child_component_ids": ["<uuid:ts-memory-write>", "<uuid:skill-memory-write>", "<uuid:memory-write>"]
  },
  {
    "group_name": "Session Memory",
    "child_component_ids": ["<uuid:pc-session-facts-parse>", "<uuid:pc-session-summary-render>",
                            "<uuid:skill-session-write>", "<uuid:skill-session-memory>",
                            "<uuid:session-summarize>", "<uuid:session-sync>"]
  },
  {
    "group_name": "Helpers",
    "child_component_ids": ["<uuid:pc-memory-extract-section>", "<uuid:pc-memory-format-entry>"]
  },
  {
    "group_name": "Domain",
    "child_component_ids": ["<uuid:skill-memory>"]
  }
]
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 26 — ExtensionCatalogue `builtin-process`

```
name:        "builtin-process"
class:       23
overview_doc: |
  # Process Domain

  The process domain provides two capabilities for spawning subprocesses and child agents.

  **Tools in this domain:**
  - `shell` — execute a shell command in the sandboxed process executor (§shell-guard)
  - `spawn_subagent` — spawn an isolated child agent run with a self-contained task

  **§shell-guard:** All recipes using `shell` must be `llm_call_required: true` (Tier 1 minimum).
  The LLM must always choose the exact shell command — deterministic dispatch of arbitrary
  shell commands is not safe.

  **§spawn_subagent-guard:** All recipes using `spawn_subagent` must be `llm_call_required: true`.
  The LLM must always frame the subagent task.

  **Decision guide:**
  - Run a specific command, script, or build step → shell
  - Delegate a clearly bounded sub-task to an isolated agent → spawn_subagent
  - Do NOT use shell for trivial file operations — use the filesystem domain instead
  - Choose subagent flavor: general (read/search), researcher (+web), coder (read/write/shell), explorer (analysis)
task_groups: [
  {
    "group_name": "Shell",
    "child_component_ids": ["<uuid:ts-shell-run>", "<uuid:skill-shell-run>", "<uuid:skill-shell>",
                            "<uuid:shell-run>", "<uuid:shell-script>"]
  },
  {
    "group_name": "Subagent",
    "child_component_ids": ["<uuid:ts-spawn-subagent>", "<uuid:skill-spawn-subagent>",
                            "<uuid:skill-subagent>", "<uuid:subagent-spawn>"]
  }
]
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```

---

### Step 27 — ExtensionCatalogue `builtin-management`

```
name:        "builtin-management"
class:       23
overview_doc: |
  # Management Domain

  The management domain provides capabilities for managing the agent's operational state:
  skills, triggers, time, data transformation, and diagnostics.

  **Tools in this domain:**
  - `skill_list / skill_install / skill_remove` — manage installed filesystem skills
  - `trigger_create / trigger_list / trigger_remove` — manage scheduled cron triggers
  - `time` — get current time, parse, convert, and format timestamps
  - `json` — parse, stringify, query, and validate JSON data
  - `echo` — diagnostic message echo (testing only)

  **Trigger note:** Trigger prompts run in a fresh session. They must be self-contained.
  Minimum cron interval is 1 minute.

  **Time note:** IANA timezone names required for timezone operations.
  Use time-now before session memory writes to get a consistent timestamp.

  **JSON note:** Use json.query for simple path extraction, or pc-json-extract-field
  PythonCode for multi-field traversal.
task_groups: [
  {
    "group_name": "Skills",
    "child_component_ids": ["<uuid:ts-skill-list>", "<uuid:ts-skill-install>", "<uuid:ts-skill-remove>",
                            "<uuid:skill-skill-list>", "<uuid:skill-skill-install>", "<uuid:skill-skill-remove>",
                            "<uuid:skill-skills>", "<uuid:skill-list>", "<uuid:skill-install>", "<uuid:skill-remove>"]
  },
  {
    "group_name": "Triggers",
    "child_component_ids": ["<uuid:ts-trigger-create>", "<uuid:ts-trigger-list>", "<uuid:ts-trigger-remove>",
                            "<uuid:skill-trigger-create>", "<uuid:skill-trigger-list>", "<uuid:skill-trigger-remove>",
                            "<uuid:skill-triggers>", "<uuid:trigger-list>", "<uuid:trigger-create>", "<uuid:trigger-remove>"]
  },
  {
    "group_name": "Time",
    "child_component_ids": ["<uuid:ts-time-now>", "<uuid:ts-time-parse>", "<uuid:ts-time-convert>",
                            "<uuid:skill-time-now>", "<uuid:skill-time-parse>", "<uuid:skill-time-convert>",
                            "<uuid:time-now>", "<uuid:time-parse>", "<uuid:time-convert>"]
  },
  {
    "group_name": "Data",
    "child_component_ids": ["<uuid:ts-json-query>", "<uuid:ts-json-stringify>",
                            "<uuid:skill-json-query>", "<uuid:skill-json-stringify>",
                            "<uuid:json-query>", "<uuid:json-stringify>"]
  },
  {
    "group_name": "Diagnostics",
    "child_component_ids": ["<uuid:ts-echo>"]
  }
]
consumer_tags: ["02:orchestrator", "05:validator"]
source:        "system"
validation_status: "validated"
```


---

## Final — Component Summary Table + Seeding Order

---

### Component Count

| Class | Type | Count | Component Names |
|-------|------|-------|-----------------|
| 0 | Tool | 23 | shell, read_file, write_file, list_dir, glob, grep, apply_patch, http, http.save, memory_search, memory_write, memory_read, memory_tree, time, json, skill_list, skill_install, skill_remove, trigger_create, trigger_list, trigger_remove, spawn_subagent, echo |
| 13 | ToolSkill | 25 | ts-shell-run, ts-read-file, ts-write-file, ts-list-dir, ts-glob, ts-grep, ts-apply-patch, ts-http-fetch, ts-http-save, ts-web-search, ts-memory-search, ts-memory-write, ts-memory-read, ts-memory-tree, ts-time-now, ts-time-parse, ts-time-convert, ts-json-query, ts-json-stringify, ts-skill-list, ts-skill-install, ts-skill-remove, ts-trigger-create, ts-trigger-list, ts-trigger-remove, ts-spawn-subagent, ts-echo |
| 1 | Leaf Skill | 21 | skill-shell-run, skill-read-file, skill-write-file, skill-list-dir, skill-glob, skill-grep, skill-apply-patch, skill-http-fetch, skill-http-save, skill-web-search, skill-memory-search, skill-memory-write, skill-memory-read, skill-memory-tree, skill-time-now, skill-time-parse, skill-time-convert, skill-json-query, skill-json-stringify, skill-skill-list, skill-skill-install, skill-skill-remove, skill-trigger-create, skill-trigger-list, skill-trigger-remove, skill-spawn-subagent, skill-session-write |
| 2 | Domain Skill | 9 | skill-shell, skill-filesystem, skill-http, skill-memory, skill-skills, skill-triggers, skill-subagent, skill-session-memory, (time/json are leaf-only — no domain skill) |
| 21 | Recipe | 22 | shell-run, shell-script, file-read, file-write, file-list, file-glob, file-grep, file-patch, http-get, http-post, http-save, web-search, memory-search, memory-write, memory-read, memory-tree, time-now, time-parse, time-convert, json-query, json-stringify, skill-list, skill-install, skill-remove, trigger-list, trigger-create, trigger-remove, subagent-spawn, session-summarize |
| 16 | Action | 1 | session-sync |
| 22 | PythonCode | 8 | pc-http-status-check, pc-json-extract-field, pc-web-search-extract, pc-memory-extract-section, pc-memory-format-entry, pc-session-facts-parse, pc-session-summary-render |
| 23 | ExtensionCatalogue | 5 | builtin-filesystem, builtin-network, builtin-memory, builtin-process, builtin-management |

**Total: ~114 components** (23 Tools + 27 ToolSkills + 27 Leaf Skills + 9 Domain Skills + 29 Recipes + 1 Action + 7 PythonCode + 5 ExtensionCatalogues = **128** — exact count depends on final seeder deduplication)

---

### Seeding Order for `builtin_bootstrap.rs`

The seeder must insert in dependency order (referenced UUIDs must exist before referencing rows):

```
Phase 1 — Tools (class 0, no dependencies)
  Insert all 23 Tool rows with source='system', validation_status='pending'.
  Q1 runs inline. On Q1 pass: update to validation_status='validated'.

Phase 2 — ToolSkills (class 13, depend on Tool rows)
  Insert all 27 ToolSkill rows, referencing Tool name in tool_name field.
  Q1 runs inline.

Phase 3 — PythonCode (class 22, no dependencies)
  Insert all 7 PythonCode rows.
  Q1 runs inline: shell-injection scan, token budget check.

Phase 4 — Leaf Skills (class 1, depend on ToolSkill and PythonCode by reference)
  Insert all 27 Leaf Skill rows.
  Q1 runs inline.

Phase 5 — Domain Skills (class 2, reference Leaf Skills by name only)
  Insert all 9 Domain Skill rows.
  Q1 runs inline.

Phase 6 — Recipes (class 21, depend on ToolSkill UUIDs and PythonCode UUIDs)
  Insert all 29 Recipe rows with step_descriptions JSONB.
  Q1 runs inline: IBS build_instruction pre-flight check (§shell-guard, §spawn_subagent-guard,
  §tier0-orchestrator-channel Rule 1/2).
  Debug builds: IbsError → panic! (seeder content is a build-time bug).
  Release builds: IbsError → error!-log, skip, continue.

Phase 7 — Action (class 16)
  Insert session-sync.
  Q1 runs inline.

Phase 8 — ExtensionCatalogues (class 23, depend on all prior UUIDs in child_component_ids)
  Insert all 5 ExtensionCatalogue rows.
  Q1 runs inline: non-empty overview_doc, ≥1 task_group, valid UUID syntax.

Phase 9 — Intent examples (via seed_intent_input)
  For each Recipe and Skill with intent_examples: seed reborn_intent_inputs rows.
  This happens at Q2 graduation time (Phase P.0), NOT at Phase L seeder time.
  The Phase L seeder does NOT call seed_intent_input directly.

Phase 10 — Automated-auditable Q2 (Phase P.0)
  The seeder/automation records itself as the Q2 actor for all system-seeded components.
  Components transition from validation_status='validated' (Q1 passed) to confirmed via
  the Phase P.0 automated Q2 graduation path.
  Nothing ever bypasses Q1+Q2 — the seeder is the recorded Q2 actor, not a bypass.
```

> **Idempotency guard:** At boot, check `SELECT COUNT(*) FROM reborn_tools WHERE source = 'system'`.
> If ≥ 1 row exists, skip the seeder entirely. A full re-seed requires manual deletion.

---

### Cross-References to Plan Phases

| This Document | Plan Phase | What it gates |
|---------------|-----------|--------------|
| Tool rows | Phase L (builtin_bootstrap.rs) | Seeded at boot time |
| ToolSkill rows | Phase L | Seeded at boot time |
| PythonCode rows | Phase L (needs V052 from Phase B) | reborn_python_code table must exist |
| ExtensionCatalogues | Phase L (needs V053 from Phase C) | reborn_extension_catalogues table must exist |
| Recipes step_descriptions | Phase A (V050 adds column), Phase L seeds | step_descriptions JSONB column must exist |
| Intent examples | Phase P.0 (Q2 graduation), not Phase L | seed_intent_input called at graduation |
| Q1 validation | Phase I (component_validator.rs rules) | class-22/23 Q1 arms must exist |
| Q2 automation | Phase P.0 | q2_actor column (V061) must exist |

---

> **End of builtin_stuff_v3.md** — all built-in v3 artifacts fully specified.
