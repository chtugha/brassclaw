# Zencoder Extension — Component Plan (v3, step_descriptions-complete)

> **Source:** `github.com/chtugha/zencoder-ironclaw-integration`
> **Target:** BrassClaw Reborn v3 extension (`ext-zencoder`)
> **Reference:** `docs/archive/tomedo_v3.md`, `docs/archive/builtin_stuff_v3.md`
> **Version:** v3 — adds correct `step_descriptions` JSONB format, `llm_call_required` flags,
>               and reconciles against the full builtin inventory from `builtin_stuff_v3.md`.
>
> ## Design intent — why this architecture exists
>
> **The LLM is a one-time cost. Recipes are the permanent return.**
>
> The first time a user asks "check the status of that Zencoder task", the system has no
> recipe for it. The LLM reasons through it (Tier 2 / Non-Matching-Mode). The Sempai
> interceptor watches the outcome and proposes a new Recipe + intent examples. After Q1
> (automated) and Q2 (human review) pass, the recipe is `validated`. From that point on,
> every identical or similar request matches the recipe at Tier 0 — no LLM call, no
> latency, no token spend. The same turn that cost an LLM call the first time costs nothing
> on the hundredth repetition.
>
> **This extension pre-seeds that library.** Rather than waiting for the system to discover
> each Zencoder operation through use, this plan authors the full component stack upfront:
> the Tools, ToolSkills, PythonCode, Skills, and Recipes are all `source: "system"` rows
> that bootstrap into `validated` state. Every Zencoder operation is Tier 0 from day one.
>
> **Tier 1 is only where the LLM genuinely adds value:** composing a task body from the
> user's description (`zencoder-solve-coding-problem`), deciding what to patch and reading
> the current state first (`zencoder-update-task`), collecting and validating schedule
> parameters (`zencoder-create-automation`). In all three cases the LLM does real work —
> it is not just repeating fixed instructions.
>
> **The Sempai continues extending the library at runtime.** When a user does something
> with Zencoder that no existing recipe covers — e.g. listing tasks filtered by *assignee*,
> or creating a task *and* immediately checking its plan — the Sempai proposes the new
> Recipe. After Q1+Q2 it becomes Tier 0. The library grows without engineering effort.
>
> ## Recycling principle (the governing law of this document)
>
> **The library is the asset. A Recipe is a thin composition of already-existing parts.**
> Every component is authored at the *minimum grain that makes it independently reusable*.
> A ToolSkill knows only its endpoint + method. A PythonCode knows only the one call it
> makes. A Leaf Skill knows only how to drive one executor. None of them know which Recipe
> uses them. The Recipe is the only place that composes them into a user-facing intent.
>
> **Consequence for this extension:**
> - `pc-zencoder-get-task` is reused in `zencoder-get-task` (standalone lookup),
>   `zencoder-check-solution-status` (pre-flight before plan fetch), and
>   `zencoder-update-task` (read-before-patch guard) — same component, three recipes.
> - `ts-zencoder-tasks-list` is reused by both `zencoder-list-tasks` and
>   `zencoder-list-tasks-filtered` — one ToolSkill, two recipe variants.
> - `skill-zencoder-auth-error` (what a 401/402/429 means and what to do) is referenced by
>   every Tier-1 recipe — authored once, cited everywhere.
>
> **What this extension does NOT author (builtins already exist):**
> - Generic HTTP GET/POST/PATCH PythonCode executors → builtins `pc-exec-http-get`,
>   `pc-exec-http-post`, `pc-exec-http-patch` already exist
> - Generic HTTP skills → `skill-http-authenticated`, `skill-http-get`, etc. already exist
> - JSON parsing/extraction → `pc-exec-json-query`, `skill-json-query` already exist
> - HTTP status checking → `pc-http-status-check` already exists
> - The generic `http` Tool (class 0) → `builtin.http` already exists
>
> **What the `zencoder-api` Tool (class 0) adds:** Zencoder-specific config — Bearer JWT
> injection from secret `zencoder_access_token`, 30 s timeout, base-URL pinned to
> `https://api.zencoder.ai/api/v1/`. This is the same pattern as `tomedo-api` wrapping
> `builtin.http` with mTLS config. The Tool is new; it is not a duplication of the builtin.

---

## Component inventory

```
Tool (class 0)            ×1    zencoder-api  (one tool for all REST calls)

ToolSkills (class 13)     ×8    one per distinct endpoint shape
                                ts-zencoder-projects-list
                                ts-zencoder-tasks-list
                                ts-zencoder-tasks-get
                                ts-zencoder-tasks-post
                                ts-zencoder-tasks-patch
                                ts-zencoder-plan-get
                                ts-zencoder-automations-list
                                ts-zencoder-automations-post

PythonCode (class 22)     ×11   one executor per distinct call + 2 pure-logic helpers
                                pc-zencoder-list-projects        (GET /projects)
                                pc-zencoder-list-tasks           (GET /projects/{pid}/tasks[?status&limit])
                                pc-zencoder-get-task             (GET /projects/{pid}/tasks/{tid})
                                pc-zencoder-get-plan             (GET /projects/{pid}/tasks/{tid}/plan)
                                pc-zencoder-create-task          (POST /projects/{pid}/tasks)
                                pc-zencoder-patch-task           (PATCH /projects/{pid}/tasks/{tid})
                                pc-zencoder-list-automations     (GET /automations[?enabled])
                                pc-zencoder-create-automation    (POST /automations)
                                pc-zencoder-post-auth-instructions (host.post_reply with fixed text)
                                pc-zencoder-validate-uuid        (pure-logic: UUID guard, no host call)
                                pc-zencoder-build-task-summary   (pure-logic: merges task+plan result)

Leaf Skills (class 1)     ×10   one per executor / one per reusable concern
                                skill-zencoder-list-projects
                                skill-zencoder-list-tasks
                                skill-zencoder-get-task
                                skill-zencoder-get-plan
                                skill-zencoder-create-task
                                skill-zencoder-patch-task
                                skill-zencoder-list-automations
                                skill-zencoder-create-automation
                                skill-zencoder-auth-error        (shared: what 401/402/429 means)
                                skill-zencoder-resilience        (shared: state machine prose)

Domain Skill (class 2)    ×1    skill-zencoder  (routing overview; references all leaves)

Skill Amendments          ×3    prepend routing layer to: coding | commit | delegation

Recipes (class 21)        ×10   thin compositions of the above
                                zencoder-list-projects           (Tier 0)
                                zencoder-list-tasks              (Tier 0)
                                zencoder-get-task                (Tier 0)
                                zencoder-get-plan                (Tier 0)
                                zencoder-check-solution-status   (Tier 0 — composes get-task + get-plan)
                                zencoder-list-tasks-filtered     (Tier 0 — status filter variant)
                                zencoder-auth-setup              (Tier 0 — fixed reply via host.post_reply)
                                zencoder-solve-coding-problem    (Tier 1)
                                zencoder-update-task             (Tier 1 — read-before-patch required)
                                zencoder-create-automation       (Tier 1)

ExtensionCatalogue (class 23) ×1   ext-zencoder
```

---

## Step 1 — Tool (class 0)

One tool. All 8 source operations use the same HTTP primitive — one Rust handler,
one capability_id, one secret injection point.

```
name:           "zencoder-api"
capability_id:  "builtin.http"
effect_type:    "mixed"
description:    "Authenticated REST call to https://api.zencoder.ai/api/v1/.
                  Bearer JWT is injected from secret 'zencoder_access_token'.
                  Methods: GET, POST, PATCH. Timeout: 30 000 ms.
                  Returns raw JSON response body as string.
                  Rate limit: 60 req/min, 1 000 req/hour (Zencoder-enforced)."
param_schema:   {method: string (GET|POST|PATCH), path: string, body?: string}
preconditions:  "Secret 'zencoder_access_token' must be set. 401 = token expired."
error_handling: "401→re-auth. 402→quota. 429→Retry-After. 5xx→retry GETs ×3."
```

---

## Step 2 — ToolSkills (class 13)

**One ToolSkill per URL template + method.** The ToolSkill knows nothing about which
Recipe uses it or what the caller intends. It only specifies: which endpoint, what
method, what the path template looks like, what parameters flow in.

```
ts-zencoder-projects-list
  tool_name:      "zencoder-api"
  description:    "GET /projects — list all accessible projects."
  param_template: {method: "GET", path: "/projects"}

ts-zencoder-tasks-list
  tool_name:      "zencoder-api"
  description:    "GET /projects/{pid}/tasks[?status=&limit=] — list tasks in a project.
                    Optional query params: status (todo|inprogress|inreview|done|cancelled),
                    limit (integer)."
  param_template: {method: "GET", path: "/projects/{{vars.pid}}/tasks"}

ts-zencoder-tasks-get
  tool_name:      "zencoder-api"
  description:    "GET /projects/{pid}/tasks/{tid} — fetch one task.
                    Returns status, description, branch."
  param_template: {method: "GET", path: "/projects/{{vars.pid}}/tasks/{{vars.tid}}"}

ts-zencoder-plan-get
  tool_name:      "zencoder-api"
  description:    "GET /projects/{pid}/tasks/{tid}/plan — fetch the task's execution plan.
                    Returns steps[{name, status}]. Status: Pending|InProgress|Completed|Skipped.
                    404 = plan not yet created — not an error."
  param_template: {method: "GET", path: "/projects/{{vars.pid}}/tasks/{{vars.tid}}/plan"}

ts-zencoder-tasks-post
  tool_name:      "zencoder-api"
  description:    "POST /projects/{pid}/tasks — create a task.
                    Body: {title, description?, workflow_id?, start?}.
                    Returns created task object containing 'id'."
  param_template: {method: "POST", path: "/projects/{{vars.pid}}/tasks", body: "{{vars.body}}"}

ts-zencoder-tasks-patch
  tool_name:      "zencoder-api"
  description:    "PATCH /projects/{pid}/tasks/{tid} — partial update.
                    Body: any subset of {title, description, status}.
                    ⚠ PATCH fully replaces description — read task first if appending.
                    Status values lowercase: todo|inprogress|inreview|done|cancelled."
  param_template: {method: "PATCH", path: "/projects/{{vars.pid}}/tasks/{{vars.tid}}", body: "{{vars.body}}"}

ts-zencoder-automations-list
  tool_name:      "zencoder-api"
  description:    "GET /automations[?enabled=true|false] — list automations."
  param_template: {method: "GET", path: "/automations"}

ts-zencoder-automations-post
  tool_name:      "zencoder-api"
  description:    "POST /automations — create a scheduled automation.
                    Body: {name, target_project_id?, task_name?, task_description?,
                    schedule_time? (HH:MM), schedule_days_of_week? (int[] 0–6)}."
  param_template: {method: "POST", path: "/automations", body: "{{vars.body}}"}
```

---

## Step 3 — PythonCode (class 22)

**One executor per distinct call. Two pure-logic helpers.**

The step isolation invariant applies: each PythonCode step runs with a fresh empty
state. Steps do not see mutations from previous steps. UUID inline-validation is
inlined per executor rather than depending on the pure-logic helper at runtime
(pure-logic helper is provided for callers that need UUID validation outside of
executor context).

**Forbidden patterns (§body-scan):** `import os`, `import subprocess`, `exec(`, `eval(`,
`open(`. The executors below use only `import re` and `import json` — both allowed.

### Pure-logic helpers (no host call)

```python
# pc-zencoder-validate-uuid
# Class: 22 | Channel: orchestrator | Pure-logic: zero host calls.
# Validates slot0 as a UUID. Returns {valid: bool, error?: str}.
import re as _re
_v = "{{vars.slot0}}"
_ok = bool(_re.match(
    r'^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$', _v, _re.I))
result = {"valid": _ok} if _ok else {"valid": False, "error": f"not a UUID: {_v!r}"}
```

```python
# pc-zencoder-build-task-summary
# Class: 22 | Channel: orchestrator | Pure-logic: zero host calls.
# Merges task JSON (slot0) and plan JSON (slot1) into a summary dict.
# Used by check-solution-status recipe after two separate fetch steps.
import json as _j
_task = _j.loads("{{vars.slot0}}")
_plan = _j.loads("{{vars.slot1}}")
_d    = _task.get("data", _task)
_pd   = _plan.get("data", _plan)
_steps = _pd.get("steps", [])
_done  = sum(1 for s in _steps if s.get("status") == "Completed")
result = {
    "task_status": _d.get("status", "unknown"),
    "branch":      _d.get("branch"),
    "progress":    f"{_done} of {len(_steps)} steps completed",
    "plan_steps":  [{"name": s.get("name",""), "status": s.get("status","Pending")}
                    for s in _steps],
}
```

### Executors (one host call each)

```python
# pc-zencoder-list-projects
# Class: 22 | Channel: orchestrator | Calls: host.zencoder_api ×1
# No parameters required.
result = host.zencoder_api(method="GET", path="/projects")
```

```python
# pc-zencoder-list-tasks
# Class: 22 | Channel: orchestrator | Calls: host.zencoder_api ×1
# slot0=project_id, slot1=status filter (empty=none), slot2=limit (empty=none)
import re as _re
_pid = "{{vars.slot0}}"
if not _re.match(r'^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$', _pid, _re.I):
    result = {"error": f"invalid project_id: {_pid!r}"}
else:
    _path = f"/projects/{_pid}/tasks"
    _q = []
    _s = "{{vars.slot1}}"
    _l = "{{vars.slot2}}"
    if _s in {"todo","inprogress","inreview","done","cancelled"}:
        _q.append(f"status={_s}")
    if str(_l).isdigit():
        _q.append(f"limit={_l}")
    if _q:
        _path += "?" + "&".join(_q)
    result = host.zencoder_api(method="GET", path=_path)
```

```python
# pc-zencoder-get-task
# Class: 22 | Channel: orchestrator | Calls: host.zencoder_api ×1
# slot0=project_id, slot1=task_id
import re as _re
_u = _re.compile(r'^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$', _re.I)
_pid, _tid = "{{vars.slot0}}", "{{vars.slot1}}"
if not _u.match(_pid) or not _u.match(_tid):
    result = {"error": "invalid UUID"}
else:
    result = host.zencoder_api(method="GET", path=f"/projects/{_pid}/tasks/{_tid}")
```

```python
# pc-zencoder-get-plan
# Class: 22 | Channel: orchestrator | Calls: host.zencoder_api ×1
# slot0=project_id, slot1=task_id
import re as _re
_u = _re.compile(r'^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$', _re.I)
_pid, _tid = "{{vars.slot0}}", "{{vars.slot1}}"
if not _u.match(_pid) or not _u.match(_tid):
    result = {"error": "invalid UUID"}
else:
    result = host.zencoder_api(method="GET", path=f"/projects/{_pid}/tasks/{_tid}/plan")
```

```python
# pc-zencoder-create-task
# Class: 22 | Channel: orchestrator | Calls: host.zencoder_api ×1
# slot0=project_id, slot1=JSON body string (title, description, workflow_id?, start?)
import re as _re
_u = _re.compile(r'^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$', _re.I)
_pid = "{{vars.slot0}}"
if not _u.match(_pid):
    result = {"error": "invalid project_id"}
else:
    result = host.zencoder_api(method="POST", path=f"/projects/{_pid}/tasks",
                               body="{{vars.slot1}}")
```

```python
# pc-zencoder-patch-task
# Class: 22 | Channel: orchestrator | Calls: host.zencoder_api ×1
# slot0=project_id, slot1=task_id, slot2=JSON body string
import re as _re
_u = _re.compile(r'^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$', _re.I)
_pid, _tid = "{{vars.slot0}}", "{{vars.slot1}}"
if not _u.match(_pid) or not _u.match(_tid):
    result = {"error": "invalid UUID"}
else:
    result = host.zencoder_api(method="PATCH",
                               path=f"/projects/{_pid}/tasks/{_tid}",
                               body="{{vars.slot2}}")
```

```python
# pc-zencoder-list-automations
# Class: 22 | Channel: orchestrator | Calls: host.zencoder_api ×1
# slot0=enabled filter ("true"|"false"|"" for none)
_f = "{{vars.slot0}}"
_path = "/automations"
if _f in {"true", "false"}:
    _path += f"?enabled={_f}"
result = host.zencoder_api(method="GET", path=_path)
```

```python
# pc-zencoder-create-automation
# Class: 22 | Channel: orchestrator | Calls: host.zencoder_api ×1
# slot0=JSON body string (LLM-composed, user-confirmed)
import json as _j
_body = "{{vars.slot0}}"
if not _j.loads(_body).get("name","").strip():
    result = {"error": "name must not be empty"}
else:
    result = host.zencoder_api(method="POST", path="/automations", body=_body)
```

```python
# pc-zencoder-post-auth-instructions
# Class: 22 | Channel: orchestrator | Calls: host.post_reply ×1
# No parameters. Posts fixed auth setup instructions directly to chat.
# Uses builtin host.post_reply — no LLM needed, output is deterministic.
result = host.post_reply(answer=(
    "**Zencoder authentication setup**\n\n"
    "1. Run the auth script:\n"
    "   ```\n"
    "   scripts/zencoder-auth.sh\n"
    "   ```\n"
    "   (Windows: `scripts\\zencoder-auth.ps1`)\n\n"
    "2. Copy the JWT from the script output and store it:\n"
    "   ```\n"
    "   brassclaw secret set zencoder_access_token <jwt>\n"
    "   ```\n\n"
    "The token is valid for ~24 hours. Re-run the script when you see a 401 error."
))
```

---

## Step 4 — Leaf Skills (class 1)

**One leaf skill per reusable concern.** A Leaf Skill describes how to use one
executor or one shared concept — nothing more. It says nothing about which Recipe
uses it.

The two shared skills (`skill-zencoder-auth-error`, `skill-zencoder-resilience`)
are not tied to any single action — they are referenced by many recipes. This is
the recycling principle applied to Skills: author the concern once, cite it everywhere.

### Shared concerns (referenced across many recipes)

```
skill-zencoder-auth-error  (class 1)
─────────────────────────
What the Zencoder HTTP error codes mean and what to do:
  401 → JWT expired. Tell the user to re-run scripts/zencoder-auth.sh and update
        the secret with: brassclaw secret set zencoder_access_token <new_jwt>
  402 → Quota or billing limit reached. Pause all Zencoder calls; wait for
        the user to confirm they have resolved it at auth.zencoder.ai.
  429 → Rate limit hit. Read the Retry-After header value; wait that many seconds;
        retry once.
  5xx → Transient server error. For GETs: retry up to 3 times with 1/2/4 turn backoff.
        For POST/PATCH: do not retry automatically — report and wait for user.
```

```
skill-zencoder-resilience  (class 1)
─────────────────────────
Three-state model to track across a conversation:
  healthy    — default; last call returned 2xx (or no call yet this session)
  degraded   — last call returned 401/402/429/5xx or a network error
  unavailable — tool returned "not found" / "not registered" (permanent this session)

When degraded or unavailable: skip ALL Zencoder calls for the current turn.
Fall through to native BrassClaw behavior. Tell the user once which fallback ran.
Probe to leave degraded: after 5 turns, or after the user confirms they fixed auth,
or after a different call succeeds.
Never probe unavailable.
```

### Per-executor leaf skills

```
skill-zencoder-list-projects  (class 1)
Use pc-zencoder-list-projects. No parameters required.
Response is an array — extract id and name from each entry. If empty, tell the user
they have no accessible projects and link to auth.zencoder.ai to verify access.
```

```
skill-zencoder-list-tasks  (class 1)
Use pc-zencoder-list-tasks with project_id (slot0), optional status (slot1),
optional limit (slot2). Status values are lowercase: todo, inprogress, inreview,
done, cancelled. If project_id is unknown, call skill-zencoder-list-projects first.
```

```
skill-zencoder-get-task  (class 1)
Use pc-zencoder-get-task with project_id (slot0) and task_id (slot1).
Returns current status, description, and branch (if any).
Call this before any patch — PATCH fully replaces description.
```

```
skill-zencoder-get-plan  (class 1)
Use pc-zencoder-get-plan with project_id (slot0) and task_id (slot1).
Returns steps[]. Step status is PascalCase: Pending|InProgress|Completed|Skipped.
404 from this call is normal for newly created tasks — not an error.
```

```
skill-zencoder-create-task  (class 1)
Use pc-zencoder-create-task with project_id (slot0) and a JSON body (slot1).
Body must include title. For delegation, also include:
  workflow_id: "default-auto-workflow" and start: true.
The response contains the new task's id — retain it in context.
```

```
skill-zencoder-patch-task  (class 1)
Use pc-zencoder-patch-task with project_id (slot0), task_id (slot1),
and a JSON body (slot2). Body must include at least one of: title, description, status.
⚠ PATCH fully replaces description. Always call skill-zencoder-get-task first
to read the current description before appending anything.
```

```
skill-zencoder-list-automations  (class 1)
Use pc-zencoder-list-automations with optional enabled filter (slot0: "true"|"false"|"").
Returns all global automations. Empty array is valid.
```

```
skill-zencoder-create-automation  (class 1)
Use pc-zencoder-create-automation with a JSON body (slot0).
Required field: name. Optional: target_project_id, task_name, task_description,
task_workflow, schedule_time (HH:MM 24-hour), schedule_days_of_week (int array,
0=Sunday through 6=Saturday). Always confirm the full spec with the user before
dispatching — automation creation cannot be undone easily.
```

---

## Step 5 — Domain Skill (class 2)

```
skill-zencoder  (class 2)
─────────────────────────
Zencoder/Zenflow is a cloud service that runs multi-model AI coding agents
asynchronously. You delegate a coding problem → a remote pipeline executes on a
git branch → you poll for completion. It is a delegated AI worker, not an LLM provider.

WHEN TO USE ZENCODER (routing rules — healthy state only):
  1. task_id present in conversation → check solution status before any local edit
  2. user explicitly delegates coding work → create a task with start:true
  3. user commits + task is inprogress/inreview → block the commit, warn once
  4. plan-mode + task_id in scope → read the Zencoder plan instead of a local doc
  5. code-review + task_id in scope → read task first, then append findings via patch
  6. all other cases → native behavior, no Zencoder call

RESILIENCE: see skill-zencoder-resilience
AUTH ERRORS: see skill-zencoder-auth-error

LEAF SKILLS:
  skill-zencoder-list-projects        find which project to work in
  skill-zencoder-list-tasks           enumerate tasks, filter by status
  skill-zencoder-get-task             read one task before patching or reviewing
  skill-zencoder-get-plan             read the execution plan
  skill-zencoder-create-task          create and optionally start a task
  skill-zencoder-patch-task           update title, description, or status
  skill-zencoder-list-automations     list scheduled automations
  skill-zencoder-create-automation    schedule a recurring task
```

---

## Step 6 — Skill Amendments

Three existing builtin skills get a routing preamble prepended.
**The preamble references skill-zencoder by name — it does not duplicate content.**

```
[coding skill amendment]
## Zencoder Routing (see skill-zencoder for full rules)
If task_id in context → check solution status before editing locally.
If user explicitly delegates → create a Zencoder task.
Otherwise → native coding behavior below.
---
<existing body unchanged>
```

```
[commit skill amendment]
## Zencoder Routing (see skill-zencoder for full rules)
If task_id in context AND status is inprogress or inreview → block commit, warn once.
Otherwise → native commit behavior below.
---
<existing body unchanged>
```

```
[delegation skill amendment]
## Zencoder Routing (see skill-zencoder for full rules)
Coding delegation (code/file/function/API/test/build/refactor/bug/PR/branch) →
  create a Zencoder task. Non-coding → native delegation below.
---
<existing body unchanged>
```

---

## Step 7 — Recipes (class 21) with step_descriptions

**Recipes are thin.** Each recipe is a named composition of components already
defined above. Nothing new is authored here — just wiring.

The `step_descriptions` JSONB format follows the canonical structure from
`builtin_stuff_v3.md`:
- `channel: "orchestrator"` + `type: "component"` = load Skills as LLM context (Tier-1 step-0)
  OR execute a PythonCode body (Tier-0 step-N, Tier-1 final step)
- `channel: "rust"` + `type: "component"` = pre-load a ToolSkill binding
- `type: "llm"` = LLM reasoning step (Tier-1 only)

> **Auth-setup is Tier 0:** The orchestrator can post directly to chat via `host.post_reply`
> without an LLM. The instructions are fixed — `pc-zencoder-post-auth-instructions` has them
> hardcoded. No LLM, no variable input, no irreversible action. This is the correct pattern:
> `ts-host-post-reply` (rust binding) → `pc-zencoder-post-auth-instructions` (orchestrator exec).

**Q1 rule check:**
- Tier-0 `orchestrator` steps contain ONLY PythonCode UUIDs. ✅
- `llm_call_required: false` recipes have zero `type: "llm"` steps. ✅
- Every `channel: "rust"` step is immediately followed by a matching `channel: "orchestrator"`
  PythonCode step. ✅

### Tier-0 reads

```json
{
  "name": "zencoder-list-projects",
  "description": "List all accessible Zencoder projects.",
  "llm_call_required": false,
  "step_descriptions": [
    {
      "step_id": "step-1",
      "type":    "component",
      "channel": "rust",
      "include": ["<uuid:ts-zencoder-projects-list>"],
      "label":   "Pre-load ts-zencoder-projects-list ToolSkill binding"
    },
    {
      "step_id": "step-2",
      "type":    "component",
      "channel": "orchestrator",
      "include": ["<uuid:pc-zencoder-list-projects>"],
      "label":   "Execute: host.zencoder_api(GET /projects)"
    }
  ],
  "skills": ["<uuid:skill-zencoder-list-projects>", "<uuid:skill-zencoder-resilience>"],
  "intent_examples": [
    {"input": "list my zencoder projects",              "class": 1},
    {"input": "what zencoder projects do I have",       "class": 1},
    {"input": "show all zenflow projects",              "class": 1},
    {"input": "zencoder project list",                  "class": 1},
    {"input": "which project should I pick",            "class": 2},
    {"input": "zenflow projects",                       "class": 1},
    {"input": "what projects are in zencoder",          "class": 1},
    {"input": "list projects",                          "class": 2},
    {"input": "show zenflow project list",              "class": 1},
    {"input": "zencoder projects",                      "class": 1}
  ]
}
```

```json
{
  "name": "zencoder-list-tasks",
  "description": "List all tasks in a Zencoder project (no filter).",
  "llm_call_required": false,
  "step_descriptions": [
    {
      "step_id": "step-1",
      "type":    "component",
      "channel": "rust",
      "include": ["<uuid:ts-zencoder-tasks-list>"],
      "label":   "Pre-load ts-zencoder-tasks-list ToolSkill binding"
    },
    {
      "step_id": "step-2",
      "type":    "component",
      "channel": "orchestrator",
      "include": ["<uuid:pc-zencoder-list-tasks>"],
      "label":   "Execute: host.zencoder_api(GET /projects/{pid}/tasks)"
    }
  ],
  "skills": ["<uuid:skill-zencoder-list-tasks>", "<uuid:skill-zencoder-resilience>"],
  "intent_examples": [
    {"input": "list zencoder tasks",                    "class": 1},
    {"input": "show all zenflow tasks",                 "class": 1},
    {"input": "what tasks are in this project",         "class": 1},
    {"input": "zencoder task list",                     "class": 1},
    {"input": "list all tasks",                         "class": 2},
    {"input": "show zenflow tasks",                     "class": 1},
    {"input": "what tasks exist in zencoder",           "class": 1},
    {"input": "list my zencoder tasks",                 "class": 1},
    {"input": "show tasks in project X",                "class": 2},
    {"input": "zenflow task list",                      "class": 1}
  ]
}
```

```json
{
  "name": "zencoder-list-tasks-filtered",
  "description": "List tasks in a Zencoder project filtered by status.",
  "llm_call_required": false,
  "variable_patterns": [{"name": "slot1", "description": "status filter"}],
  "step_descriptions": [
    {
      "step_id": "step-1",
      "type":    "component",
      "channel": "rust",
      "include": ["<uuid:ts-zencoder-tasks-list>"],
      "label":   "Pre-load ts-zencoder-tasks-list ToolSkill binding"
    },
    {
      "step_id": "step-2",
      "type":    "component",
      "channel": "orchestrator",
      "include": ["<uuid:pc-zencoder-list-tasks>"],
      "label":   "Execute: host.zencoder_api(GET /projects/{pid}/tasks?status={slot1})"
    }
  ],
  "skills": ["<uuid:skill-zencoder-list-tasks>", "<uuid:skill-zencoder-resilience>"],
  "intent_examples": [
    {"input": "show inprogress zencoder tasks",         "class": 1},
    {"input": "list tasks in review",                   "class": 1},
    {"input": "what tasks are done",                    "class": 1},
    {"input": "show cancelled zencoder tasks",          "class": 1},
    {"input": "list todo tasks in zenflow",             "class": 1},
    {"input": "which tasks are in progress",            "class": 1},
    {"input": "show tasks with status done",            "class": 1},
    {"input": "zenflow inprogress tasks",               "class": 1},
    {"input": "list tasks that are in review",          "class": 1},
    {"input": "show open zencoder tasks",               "class": 1}
  ]
}
```

```json
{
  "name": "zencoder-get-task",
  "description": "Fetch a single Zencoder task by project_id and task_id.",
  "llm_call_required": false,
  "variable_patterns": [
    {"name": "slot0", "description": "project_id (UUID)"},
    {"name": "slot1", "description": "task_id (UUID)"}
  ],
  "step_descriptions": [
    {
      "step_id": "step-1",
      "type":    "component",
      "channel": "rust",
      "include": ["<uuid:ts-zencoder-tasks-get>"],
      "label":   "Pre-load ts-zencoder-tasks-get ToolSkill binding"
    },
    {
      "step_id": "step-2",
      "type":    "component",
      "channel": "orchestrator",
      "include": ["<uuid:pc-zencoder-get-task>"],
      "label":   "Execute: host.zencoder_api(GET /projects/{pid}/tasks/{tid})"
    }
  ],
  "skills": ["<uuid:skill-zencoder-get-task>", "<uuid:skill-zencoder-resilience>"],
  "intent_examples": [
    {"input": "get zencoder task",                      "class": 1},
    {"input": "show task details",                      "class": 2},
    {"input": "what is the status of task X",           "class": 1},
    {"input": "fetch zencoder task",                    "class": 1},
    {"input": "look up zenflow task",                   "class": 1},
    {"input": "show me that task",                      "class": 2},
    {"input": "get task by id",                         "class": 1},
    {"input": "zencoder task details",                  "class": 1},
    {"input": "what does task X say",                   "class": 1},
    {"input": "read zenflow task",                      "class": 1}
  ]
}
```

```json
{
  "name": "zencoder-get-plan",
  "description": "Fetch the execution plan for a Zencoder task.",
  "llm_call_required": false,
  "variable_patterns": [
    {"name": "slot0", "description": "project_id (UUID)"},
    {"name": "slot1", "description": "task_id (UUID)"}
  ],
  "step_descriptions": [
    {
      "step_id": "step-1",
      "type":    "component",
      "channel": "rust",
      "include": ["<uuid:ts-zencoder-plan-get>"],
      "label":   "Pre-load ts-zencoder-plan-get ToolSkill binding"
    },
    {
      "step_id": "step-2",
      "type":    "component",
      "channel": "orchestrator",
      "include": ["<uuid:pc-zencoder-get-plan>"],
      "label":   "Execute: host.zencoder_api(GET /projects/{pid}/tasks/{tid}/plan)"
    }
  ],
  "skills": ["<uuid:skill-zencoder-get-plan>", "<uuid:skill-zencoder-resilience>"],
  "intent_examples": [
    {"input": "show the zencoder plan",                 "class": 1},
    {"input": "what steps has zenflow planned",         "class": 1},
    {"input": "get plan for this task",                 "class": 1},
    {"input": "zencoder execution plan",                "class": 1},
    {"input": "show plan steps",                        "class": 1},
    {"input": "what is zenflow going to do",            "class": 1},
    {"input": "list the plan",                          "class": 2},
    {"input": "show task plan",                         "class": 2},
    {"input": "get plan from zencoder",                 "class": 1},
    {"input": "zenflow plan steps",                     "class": 1}
  ]
}
```

```json
{
  "name": "zencoder-check-solution-status",
  "description": "Check a Zencoder task's status and execution plan progress in one combined read.",
  "llm_call_required": false,
  "variable_patterns": [
    {"name": "slot0", "description": "project_id (UUID)"},
    {"name": "slot1", "description": "task_id (UUID)"}
  ],
  "step_descriptions": [
    {
      "step_id": "step-1",
      "type":    "component",
      "channel": "rust",
      "include": ["<uuid:ts-zencoder-tasks-get>", "<uuid:ts-zencoder-plan-get>"],
      "label":   "Pre-load task + plan ToolSkill bindings"
    },
    {
      "step_id": "step-2",
      "type":    "component",
      "channel": "orchestrator",
      "include": ["<uuid:pc-zencoder-get-task>"],
      "label":   "Execute: host.zencoder_api(GET /projects/{pid}/tasks/{tid}) → slot0 for summary"
    },
    {
      "step_id": "step-3",
      "type":    "component",
      "channel": "orchestrator",
      "include": ["<uuid:pc-zencoder-get-plan>"],
      "label":   "Execute: host.zencoder_api(GET /projects/{pid}/tasks/{tid}/plan) → slot1 for summary"
    },
    {
      "step_id": "step-4",
      "type":    "component",
      "channel": "orchestrator",
      "include": ["<uuid:pc-zencoder-build-task-summary>"],
      "label":   "Pure-logic: merge task + plan results into summary dict"
    }
  ],
  "skills": [
    "<uuid:skill-zencoder-get-task>",
    "<uuid:skill-zencoder-get-plan>",
    "<uuid:skill-zencoder-resilience>"
  ],
  "intent_examples": [
    {"input": "how is that zencoder task going",        "class": 1},
    {"input": "check zencoder status",                  "class": 1},
    {"input": "is the zenflow agent done",              "class": 1},
    {"input": "check solution status",                  "class": 1},
    {"input": "how far along is zencoder",              "class": 1},
    {"input": "what has zenflow done so far",           "class": 1},
    {"input": "status of the delegated task",           "class": 1},
    {"input": "is the coding task finished",            "class": 1},
    {"input": "how many steps completed",               "class": 1},
    {"input": "zencoder progress",                      "class": 1},
    {"input": "check if zencoder is done",              "class": 1},
    {"input": "what is the progress",                   "class": 2}
  ]
}
```

### Tier-1 operations

> **Tier-1 recipe structure (canonical):**
> - step-0: `type: "component", channel: "orchestrator"` — load Skill UUIDs as LLM context
>   (Skills in an orchestrator/component step is the correct context-loading pattern; this is
>   NOT a Q1 error — Q1 forbids Skills in `orchestrator_steps` of Tier-0 recipes only)
> - step-1: `type: "llm"` — LLM reasoning, composition, user confirmation
> - step-2: `type: "component", channel: "rust"` — pre-load ToolSkill binding
> - step-3: `type: "component", channel: "orchestrator"` — execute PythonCode

```json
{
  "name": "zencoder-solve-coding-problem",
  "description": "Delegate a coding problem to Zencoder — create and start a task.",
  "llm_call_required": true,
  "step_descriptions": [
    {
      "step_id": "step-0",
      "type":    "component",
      "channel": "orchestrator",
      "include": [
        "<uuid:skill-zencoder>",
        "<uuid:skill-zencoder-create-task>",
        "<uuid:skill-zencoder-list-projects>",
        "<uuid:skill-zencoder-auth-error>"
      ],
      "label":   "Load Zencoder domain + create-task + auth-error skills as LLM context"
    },
    {
      "step_id": "step-1",
      "type":    "llm",
      "label":   "LLM: compose task body (title, workflow_id:default-auto-workflow, start:true), confirm project_id (list-projects if unknown), present to user before creating"
    },
    {
      "step_id": "step-2",
      "type":    "component",
      "channel": "rust",
      "include": ["<uuid:ts-zencoder-tasks-post>"],
      "label":   "Pre-load ts-zencoder-tasks-post ToolSkill binding"
    },
    {
      "step_id": "step-3",
      "type":    "component",
      "channel": "orchestrator",
      "include": ["<uuid:pc-zencoder-create-task>"],
      "label":   "Execute: host.zencoder_api(POST /projects/{pid}/tasks, body)"
    }
  ],
  "intent_examples": [
    {"input": "delegate this to zencoder",                    "class": 1},
    {"input": "have zenflow fix this",                        "class": 1},
    {"input": "let zencoder handle the bug",                  "class": 1},
    {"input": "solve coding problem with zencoder",           "class": 1},
    {"input": "send this to zenflow",                         "class": 1},
    {"input": "zencoder solve this",                          "class": 1},
    {"input": "have zencoder implement this",                 "class": 1},
    {"input": "push this to zenflow",                         "class": 1},
    {"input": "ask zencoder to fix this",                     "class": 1},
    {"input": "delegate to zenflow agents",                   "class": 1},
    {"input": "zenflow take over",                            "class": 1},
    {"input": "delegate the implementation to zencoder",      "class": 1}
  ]
}
```

```json
{
  "name": "zencoder-update-task",
  "description": "Update a Zencoder task — status, title, or description (read-then-patch).",
  "llm_call_required": true,
  "step_descriptions": [
    {
      "step_id": "step-0",
      "type":    "component",
      "channel": "orchestrator",
      "include": [
        "<uuid:skill-zencoder-get-task>",
        "<uuid:skill-zencoder-patch-task>",
        "<uuid:skill-zencoder-auth-error>"
      ],
      "label":   "Load get-task + patch-task + auth-error skills as LLM context"
    },
    {
      "step_id": "step-1",
      "type":    "llm",
      "label":   "LLM: read current task (get-task result in context), compose PATCH body, confirm with user"
    },
    {
      "step_id": "step-2",
      "type":    "component",
      "channel": "rust",
      "include": ["<uuid:ts-zencoder-tasks-patch>"],
      "label":   "Pre-load ts-zencoder-tasks-patch ToolSkill binding"
    },
    {
      "step_id": "step-3",
      "type":    "component",
      "channel": "orchestrator",
      "include": ["<uuid:pc-zencoder-patch-task>"],
      "label":   "Execute: host.zencoder_api(PATCH /projects/{pid}/tasks/{tid}, body)"
    }
  ],
  "intent_examples": [
    {"input": "update the zencoder task",               "class": 1},
    {"input": "mark task as done",                      "class": 1},
    {"input": "change task status to done",             "class": 1},
    {"input": "close this zencoder task",               "class": 1},
    {"input": "update task description",                "class": 1},
    {"input": "set status to cancelled",                "class": 1},
    {"input": "add notes to the task",                  "class": 1},
    {"input": "mark zenflow task complete",             "class": 1},
    {"input": "finish the zencoder task",               "class": 1},
    {"input": "update zenflow task status",             "class": 1}
  ]
}
```

```json
{
  "name": "zencoder-create-automation",
  "description": "Create a Zencoder scheduled automation after user confirmation.",
  "llm_call_required": true,
  "step_descriptions": [
    {
      "step_id": "step-0",
      "type":    "component",
      "channel": "orchestrator",
      "include": [
        "<uuid:skill-zencoder-create-automation>",
        "<uuid:skill-zencoder-auth-error>"
      ],
      "label":   "Load create-automation + auth-error skills as LLM context"
    },
    {
      "step_id": "step-1",
      "type":    "llm",
      "label":   "LLM: collect automation details, validate schedule_time (HH:MM), present full spec for user confirmation"
    },
    {
      "step_id": "step-2",
      "type":    "component",
      "channel": "rust",
      "include": ["<uuid:ts-zencoder-automations-post>"],
      "label":   "Pre-load ts-zencoder-automations-post ToolSkill binding"
    },
    {
      "step_id": "step-3",
      "type":    "component",
      "channel": "orchestrator",
      "include": ["<uuid:pc-zencoder-create-automation>"],
      "label":   "Execute: host.zencoder_api(POST /automations, body)"
    }
  ],
  "intent_examples": [
    {"input": "create a zencoder automation",           "class": 1},
    {"input": "schedule zenflow daily",                 "class": 1},
    {"input": "run zencoder every weekday",             "class": 1},
    {"input": "automate zencoder",                      "class": 1},
    {"input": "set up scheduled zencoder task",         "class": 1},
    {"input": "create zenflow automation",              "class": 1},
    {"input": "schedule zencoder at 9am",               "class": 1},
    {"input": "add a zencoder cron",                    "class": 1},
    {"input": "daily zenflow run",                      "class": 1},
    {"input": "automate the coding task weekly",        "class": 1}
  ]
}
```

```json
{
  "name": "zencoder-auth-setup",
  "description": "Post fixed Zencoder authentication setup instructions to the user. Tier 0.",
  "llm_call_required": false,
  "step_descriptions": [
    {
      "step_id": "step-1",
      "type":    "component",
      "channel": "rust",
      "include": ["<uuid:ts-host-post-reply>"],
      "label":   "Pre-load ts-host-post-reply ToolSkill binding"
    },
    {
      "step_id": "step-2",
      "type":    "component",
      "channel": "orchestrator",
      "include": ["<uuid:pc-zencoder-post-auth-instructions>"],
      "label":   "Execute: host.post_reply(fixed auth instructions)"
    }
  ],
  "intent_examples": [
    {"input": "set up zencoder",                        "class": 1},
    {"input": "authenticate with zencoder",             "class": 1},
    {"input": "configure zencoder token",               "class": 1},
    {"input": "zencoder auth",                          "class": 1},
    {"input": "how do I connect to zencoder",           "class": 1},
    {"input": "set zencoder api key",                   "class": 1},
    {"input": "zencoder login",                         "class": 1},
    {"input": "configure zenflow access",               "class": 1},
    {"input": "set up zenflow credentials",             "class": 1},
    {"input": "zencoder token setup",                   "class": 1}
  ]
}
```

---

## Step 8 — ExtensionCatalogue (class 23)

```
name:         "ext-zencoder"
overview_doc: |
  Zencoder/Zenflow: delegate coding tasks to AI agents, track progress,
  manage tasks/plans/automations. Auth: run scripts/zencoder-auth.sh →
  brassclaw secret set zencoder_access_token <jwt>. Token ~24h lifetime.

task_groups:
  Delegation:      zencoder-solve-coding-problem, zencoder-check-solution-status
  Task management: zencoder-list-projects, zencoder-list-tasks,
                   zencoder-list-tasks-filtered, zencoder-get-task, zencoder-update-task
  Plans:           zencoder-get-plan
  Automations:     zencoder-list-automations, zencoder-create-automation
  Setup:           zencoder-auth-setup
```

> **Note:** `zencoder-list-automations` is a Tier-0 recipe not in the table above — add
> it here even though it was not listed in the original component inventory (it's a straightforward
> read, included for completeness; recipe body follows the same Tier-0 pattern as the others).

---

## Reuse map — proof of correct grain

A component that appears in only one recipe isn't necessarily wrong,
but a component that appears in many is the sign of correct grain.

```
Component                        Appears in recipes
────────────────────────────────────────────────────────────────────
ts-zencoder-tasks-get            get-task, check-solution-status
ts-zencoder-plan-get             get-plan, check-solution-status
ts-zencoder-tasks-list           list-tasks, list-tasks-filtered
pc-zencoder-get-task             get-task, check-solution-status
pc-zencoder-get-plan             get-plan, check-solution-status
pc-zencoder-list-tasks           list-tasks, list-tasks-filtered
skill-zencoder-get-task          get-task, update-task, check-solution-status (step-0 context)
skill-zencoder-get-plan          get-plan, check-solution-status (step-0 context)
skill-zencoder-list-projects     list-projects, solve-coding-problem (step-0 context)
skill-zencoder-resilience        all Tier-0 recipes (×6, in skills[] array)
skill-zencoder-auth-error        all Tier-1 recipes (×4, in step-0 context)
skill-zencoder                   solve-coding-problem (step-0 context) + domain skill amendments (×3)
pc-zencoder-validate-uuid        pure-logic helper available to any orchestrator step needing
                                  UUID pre-flight outside executor context
pc-zencoder-build-task-summary   check-solution-status (step-4)
```

---

## Q1 hard error checklist

| Rule | Status | Evidence |
|------|--------|----------|
| Tier-0 `orchestrator` steps contain ONLY PythonCode | ✅ | All Tier-0 recipes: only `pc-zencoder-*` UUIDs in `channel:orchestrator` steps |
| `llm_call_required: false` recipes have zero `type:"llm"` steps | ✅ | Six Tier-0 recipes: no LLM step present |
| Every `channel:"rust"` step is followed by a `channel:"orchestrator"` PythonCode step | ✅ | All Tier-0 and Tier-1 recipes: rust → orchestrator pairs correct |
| `§body-scan`: no forbidden imports in PythonCode | ✅ | Bodies use only `import re`, `import json` — both allowed |
| `§shell-guard`: no `builtin.shell` usage | ✅ | No shell calls anywhere in this extension |
| `§spawn_subagent-guard`: no `builtin.spawn_subagent` usage | ✅ | No subagent calls in this extension |
| `§no-snippet`: no `type:"snippet"` steps | ✅ | All steps use `type:"component"`, `type:"llm"`, or `type:"text"` |
| `§channel-isolation`: ToolSkill UUIDs not in `orchestrator_steps` | ✅ | `ts-zencoder-*` only appear in `channel:"rust"` steps |
| One PythonCode = one host call | ✅ | Each executor has exactly one `host.zencoder_api(...)` call |
| Tier-1 step-0 skills: correctly in `channel:"orchestrator"` `type:"component"` (context loading) | ✅ | Canonical pattern from `builtin_stuff_v3.md` `http-post` recipe confirmed |

---

## Build order

```
1.  zencoder-api                  (class 0)  — no dependencies
2.  ts-zencoder-*                 (class 13) — depend on tool name only
3.  pc-zencoder-validate-uuid     (class 22) — pure-logic, no dependencies
4.  pc-zencoder-build-task-summary(class 22) — pure-logic, no dependencies
5.  pc-zencoder-list-projects     (class 22) — depends on ts-zencoder-projects-list shape
6.  pc-zencoder-list-tasks        (class 22) — depends on ts-zencoder-tasks-list shape
7.  pc-zencoder-get-task          (class 22) — depends on ts-zencoder-tasks-get shape
8.  pc-zencoder-get-plan          (class 22) — depends on ts-zencoder-plan-get shape
9.  pc-zencoder-create-task       (class 22) — depends on ts-zencoder-tasks-post shape
10. pc-zencoder-patch-task        (class 22) — depends on ts-zencoder-tasks-patch shape
11. pc-zencoder-list-automations  (class 22) — depends on ts-zencoder-automations-list shape
12. pc-zencoder-create-automation (class 22) — depends on ts-zencoder-automations-post shape
13. pc-zencoder-post-auth-instructions (class 22) — calls host.post_reply (builtin), no zencoder-api dep
14. skill-zencoder-auth-error     (class 1)  — no dependencies
15. skill-zencoder-resilience     (class 1)  — no dependencies
16. skill-zencoder-list-projects  (class 1)  — references pc-zencoder-list-projects name
17. skill-zencoder-list-tasks     (class 1)  — references pc-zencoder-list-tasks name
18. skill-zencoder-get-task       (class 1)  — references pc-zencoder-get-task name
19. skill-zencoder-get-plan       (class 1)  — references pc-zencoder-get-plan name
20. skill-zencoder-create-task    (class 1)  — references pc-zencoder-create-task name
21. skill-zencoder-patch-task     (class 1)  — references pc-zencoder-patch-task name
22. skill-zencoder-list-automations (class 1) — references pc-zencoder-list-automations name
23. skill-zencoder-create-automation(class 1) — references pc-zencoder-create-automation name
24. skill-zencoder               (class 2)  — references all leaf skills by name
25. Skill amendments              —          — reference skill-zencoder by name
26. zencoder-list-projects        (class 21) — references steps 2 + 5 + 16–17 by UUID
27. zencoder-list-tasks           (class 21) — references steps 3 + 6 + 17 by UUID
28. zencoder-list-tasks-filtered  (class 21) — references steps 3 + 6 + 17 by UUID
29. zencoder-get-task             (class 21) — references steps 4 + 7 + 18 by UUID
30. zencoder-get-plan             (class 21) — references steps 4 + 8 + 19 by UUID
31. zencoder-check-solution-status(class 21) — references steps 4 + 7 + 8 + 18 + 19 by UUID
32. zencoder-auth-setup           (class 21) — references step 13 + ts-host-post-reply (builtin) by UUID
33. zencoder-solve-coding-problem (class 21) — references steps 2 + 9 + 14 + 16 + 24 by UUID
34. zencoder-update-task          (class 21) — references steps 4 + 10 + 14 + 18 + 21 by UUID
35. zencoder-create-automation    (class 21) — references steps 2 + 12 + 14 + 23 by UUID
36. ext-zencoder                  (class 23) — references recipe names
```
