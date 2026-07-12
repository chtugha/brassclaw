# Intelligent Token Budget — Implementation Plan

## 0. Codebase Audit: Every Budget-Enforcement Location

Every file and line where `__check_budget__`, `tokens_remaining`,
`time_remaining_ms`, `usd_remaining`, or any immediate-stop logic lives:

| File | Lines | What it does |
|------|-------|-------------|
| `crates/brassclaw_engine/orchestrator/default.py` | 17, 789–798 | **THE primary target.** Calls `__check_budget__()` each iteration and immediately stops the turn if any value is ≤ 0. |
| `crates/brassclaw_engine/src/executor/orchestrator.rs` | 18, 623–624, 2449–2481 | Rust host-function handler. Reads `thread.config.max_tokens_total`, `max_duration`, `max_budget_usd`, returns three values as a JSON dict. |

No other production files implement equivalent mid-turn stop logic. All other
budget-related code in `brassclaw_agent_loop` governs *context selection* and
*compaction thresholds*, never turn-stopping.

---

## 1. Relationship to `token-budget-next-step.md` (current state)

`token-budget-next-step.md` was externally updated since this plan was first
written. The table below reflects the **current confirmed state** of the codebase
(verified by reading the modified files):

| Step | Status | Evidence in codebase |
|------|--------|---------------------|
| Step 1 — Wire `max_output` | ✅ Done | `LlmProviderModelGateway.max_output_tokens`, `DefaultPlannedRuntimeConfig.max_output_tokens` |
| Step 2 — Hot-swap budget gap | ✅ Done | `LlmReloadTrigger::on_provider_changed`, `RebornLlmReloadAdapter.with_on_provider_changed`, wired in `webui.rs` |
| Step 3 — `DefaultContextStrategy` semantics | ✅ Done | `context.rs` uses `TurnContextBudget` + `ObservedMessageAverage` EMA; `notify_model_usage` hook exists |
| Step 4 — Feed `context_window_tokens` | ✅ Done | `families/mod.rs` passes `context_window_tokens` to `DefaultContextStrategy` and `DefaultCompactionStrategy` |
| **NEW** — Wire `inline_control_tokens` | ✅ Done (just landed) | `default_with_full_config` now takes 5th param `inline_control_tokens: Option<usize>` and writes it to `strategy.inline_control_tokens` |
| Step 5 — Remove `[tokens]` fields from config schema | ⬜ Pending | `TokensSection` still has number fields |
| Step 6 — Phase 8 missing tests | ⬜ Pending | `ProviderDefinition` round-trip + live-setter regression tests not yet written |
| Steps 7–10 — Prompt caching | ⬜ Pending | `cache_retention` DB/UI, `LoopModelUsage` cache fields, vLLM tool-sort, `ProviderDefinition.cache_retention` |

### Specific changes landed in the externally modified files

**`crates/brassclaw_agent_loop/src/families/mod.rs`** (now at line 69–121):
- `default_with_full_config` signature changed from 4 to **5 parameters** — new
  trailing param: `inline_control_tokens: Option<usize>`.
- After the context strategy is constructed, line 97 sets:
  `strategy.inline_control_tokens = inline_control_tokens;`
- The `let mut strategy = ...` pattern (was `let strategy = ...`) enables this.

**`crates/brassclaw_agent_loop/src/strategies/context.rs`** (tests section):
- `plan_context_request_clamps_zero_to_one` test updated: no longer uses an
  inline struct literal with `token_budget: Some(LiveTokenBudget::new(...))`.
  Now uses `DefaultContextStrategy::with_token_budget(0, DEFAULT_MAX_CONTEXT_TOKENS)`.
  This is a test-only change; the struct itself is unchanged.

**`token-budget-next-step.md`**:
- The status table now confirms Steps 1–4 are done.
- Steps 5–6 remain pending (same content as before).
- New Steps 7–10 added (prompt caching infrastructure). These are orthogonal
  to this plan and must not be duplicated.

### Interaction between `inline_control_tokens` and this plan

`inline_control_tokens` is now wired through `default_with_full_config`. Any
future call sites that invoke `default_with_full_config` must pass 5 arguments.
The plan's section §3 (host function additions) does not call
`default_with_full_config` and is unaffected. However, the "Summary of All
Changed Files" table in §8 must list `app_loop_family.rs` as a downstream
caller that already needs updating to pass the new argument (see §3.6 note below).

---

## 2. Architecture Overview

### Current (to remove)

```
run_loop() — each iteration:
  ① __check_budget__()          ← reads tokens/time/usd
  ② if tokens_remaining ≤ 0:    ← HARD STOP — turn never reaches LLM
       transition "completed"
       return "Token budget exhausted"
  ③ if time_remaining_ms ≤ 0:   ← HARD STOP
  ④ if usd_remaining ≤ 0:       ← HARD STOP
  ⑤ inject prior knowledge + skills (step 0)
  ⑥ compact_if_needed()
  ⑦ __llm_complete__()
```

### New (this plan)

```
run_loop() — each iteration:
  ① __check_budget__()          ← reads budget; LOGS ONLY via __log_budget_warning__()
  ② inject prior knowledge + skills (step 0) — no early exit
  ③ measure total token count of assembled working_messages
  ④ if total tokens > limit:    ← SINGLE POST-ASSEMBLY CHECK
       apply reduce_prompt(working_messages, limit, rules)
  ⑤ compact_if_needed()
  ⑥ __llm_complete__()          ← guaranteed to happen
```

The Prompt-Segment Reduction logic lives in a new Python module:
`crates/brassclaw_engine/orchestrator/segment_reduction.py`

Two new host functions are added:
- `__log_budget_warning__(field, value, message)` — telemetry only, no branch.
- `__get_reduction_rules__()` — returns rule configs from the Store (cached in
  `thread.metadata`; no DB read per step).

---

## 3. Changes — Precise File-Level Deltas

### 3.1  `crates/brassclaw_engine/orchestrator/default.py`

#### 3.1.1 — Remove hard-stop block (lines 788–798), replace with telemetry

**DELETE** lines 788–798 entirely:

```python
        # 2. Check budget
        budget = __check_budget__()
        if budget.get("tokens_remaining", 1) <= 0:
            __transition_to__("completed", "token budget exhausted")
            return complete_result(state, "completed", "Token budget exhausted.")
        if budget.get("time_remaining_ms", 1) <= 0:
            __transition_to__("completed", "time budget exhausted")
            return complete_result(state, "completed", "Time budget exhausted.")
        if budget.get("usd_remaining") is not None and budget["usd_remaining"] <= 0:
            __transition_to__("completed", "cost budget exhausted")
            return complete_result(state, "completed", "Cost budget exhausted.")
```

**REPLACE** with:

```python
        # 2. Budget telemetry — soft warnings only, never stops the turn.
        # Hard enforcement is performed post-assembly in step 3.5a below.
        budget = __check_budget__()
        tokens_remaining = budget.get("tokens_remaining", -1)
        time_remaining_ms = budget.get("time_remaining_ms", -1)
        usd_remaining = budget.get("usd_remaining")
        if tokens_remaining >= 0 and tokens_remaining < 500:
            __log_budget_warning__(
                "tokens_remaining", tokens_remaining,
                "low token budget — prompt assembly will continue; reduction may apply"
            )
        if time_remaining_ms >= 0 and time_remaining_ms < 5000:
            __log_budget_warning__(
                "time_remaining_ms", time_remaining_ms,
                "low time budget — continuing turn; time is a soft warning only"
            )
        if usd_remaining is not None and usd_remaining < 0.001:
            __log_budget_warning__(
                "usd_remaining", usd_remaining,
                "low cost budget — continuing turn; cost is a soft warning only"
            )
```

#### 3.1.2 — Add post-assembly check (after step 3, before the existing step 3.5 compaction)

After the existing `# 3. Inject prior knowledge and activate skills on first step`
block (ends around line 851 in the unmodified file), insert:

```python
        # 3.5a Post-assembly enforcement: measure total assembled prompt tokens.
        # This is the ONLY hard token cutoff. Fires every iteration, not just step 0,
        # because tool results injected in later steps can also push over budget.
        token_limit = budget.get("tokens_remaining", -1)
        if token_limit > 0:
            current_total = estimate_context_tokens(working_messages)
            if current_total > token_limit:
                __emit_event__(
                    "prompt_over_budget",
                    total_tokens=current_total,
                    limit=token_limit,
                )
                rules = __get_reduction_rules__()
                from segment_reduction import reduce_prompt
                working_messages, reduction_log = reduce_prompt(
                    working_messages, token_limit, rules
                )
                state["last_reduction_log"] = reduction_log
                __save_checkpoint__(state, {
                    "nudge_count": consecutive_nudges,
                    "consecutive_errors": consecutive_errors,
                    "consecutive_action_errors": consecutive_action_errors,
                    "compaction_count": state.get("compaction_count", 0),
                    "obligation_nudge_count": state.get("_obligation_nudge_count", 0),
                })
```

`from segment_reduction import reduce_prompt` uses Monty's module system.
`segment_reduction.py` is co-located in `orchestrator/` and registered by the
Rust loader alongside `default.py`. Monty caches the module after first import;
it does not re-parse on every loop iteration.

---

### 3.2  New file: `crates/brassclaw_engine/orchestrator/segment_reduction.py`

**Monty constraint:** Monty v0.0.16 does not support `class Foo:`. All five
rules are implemented as **function-dict factories** — the standard callable
interface is expressed through dicts with callable values, which Monty does
support.

Full file content:

```python
# crates/brassclaw_engine/orchestrator/segment_reduction.py
#
# Prompt-Segment Reduction pipeline — Monty-compatible (no class statements).
#
# Rule dict interface — every rule dict MUST contain:
#
#   {
#     "name": str,                           # Unique snake_case identifier
#     "description": str,                    # Human-readable purpose
#     "enabled": bool,                       # Togglable via UI
#     "params": dict,                        # User-editable parameters
#     "apply": callable(segment_text: str, segment_metadata: dict)
#                -> (reduced_text: str, log_entry: dict),
#     "apply_to_messages": callable(messages: list) -> (messages: list, logs: list)
#                          OR None for per-segment rules,
#   }
#
# log_entry must contain at minimum:
#   {"rule": name, "original_tokens": int, "reduced_tokens": int, "change_description": str}
#
# JSON/TOML serialization contract:
#   {"name": "<rule name>", "enabled": bool, "params": {...}}
#   "name" selects the factory in BUILTIN_RULE_REGISTRY.
#   User-defined rules: Python source stored as DocType::Skill tag
#   "reduction_rule_source"; config dict stored with tag "reduction_rule_config".
#
# LLM-authored rule authoring prompt (enforced in Rust §3.4):
#   The LLM must return a factory function named "make_rule" that returns a
#   rule dict with the interface above. The returned source is validated
#   (syntax check + interface check) before storage.
#
# Validation before a new rule is accepted:
#   - Source must parse without SyntaxError.
#   - Source must define a callable named "make_rule".
#   - "make_rule()" must return a dict with keys: name, description, enabled,
#     params, apply. (apply_to_messages is optional.)
#   - apply must be callable.


CHARS_PER_TOKEN = 4


def _estimate_tokens(text):
    if not text:
        return 0
    return max(0, (len(text) + CHARS_PER_TOKEN - 1) // CHARS_PER_TOKEN)


def _message_tokens(msg):
    content = msg.get("content") or ""
    action_name = msg.get("action_name") or ""
    return _estimate_tokens(content) + _estimate_tokens(action_name) + 1


def _total_tokens(messages):
    total = 0
    for m in messages:
        total += _message_tokens(m)
    return total


# ── Rule 1: compress_whitespace_formatting ────────────────────────────────────

def _compress_whitespace_apply(segment_text, segment_metadata, params):
    max_nl = params.get("max_consecutive_newlines", 2)
    original_tokens = _estimate_tokens(segment_text)
    threshold = "\n" * (max_nl + 1)
    replacement = "\n" * max_nl
    result = segment_text
    while threshold in result:
        result = result.replace(threshold, replacement)
    lines = result.split("\n")
    result = "\n".join(line.rstrip(" ") for line in lines)
    reduced_tokens = _estimate_tokens(result)
    return result, {
        "rule": "compress_whitespace_formatting",
        "original_tokens": original_tokens,
        "reduced_tokens": reduced_tokens,
        "change_description": "whitespace compressed: {} -> {} tokens".format(
            original_tokens, reduced_tokens
        ),
    }


def make_compress_whitespace_formatting(enabled=True, max_consecutive_newlines=2):
    params = {"max_consecutive_newlines": max_consecutive_newlines}
    def apply(segment_text, segment_metadata):
        return _compress_whitespace_apply(segment_text, segment_metadata, params)
    return {
        "name": "compress_whitespace_formatting",
        "description": "Collapse excessive blank lines and trailing spaces to reduce token count.",
        "enabled": enabled,
        "params": params,
        "apply": apply,
        "apply_to_messages": None,
    }


# ── Rule 2: drop_duplicate_examples ─────────────────────────────────────────

def _drop_duplicates_apply_to_messages(messages, params):
    max_allowed = params.get("max_allowed", 1)
    roles = set(params.get("roles_to_deduplicate", ["Assistant"]))
    seen_counts = {}
    kept = []
    dropped = 0
    for msg in messages:
        if msg.get("role") in roles:
            key = (msg.get("role"), msg.get("content", ""))
            count = seen_counts.get(key, 0)
            if count < max_allowed:
                kept.append(msg)
                seen_counts[key] = count + 1
            else:
                dropped += 1
        else:
            kept.append(msg)
    log = []
    if dropped > 0:
        log.append({
            "rule": "drop_duplicate_examples",
            "original_tokens": 0,
            "reduced_tokens": 0,
            "change_description": "dropped {} duplicate message(s)".format(dropped),
        })
    return kept, log


def make_drop_duplicate_examples(enabled=True, max_allowed=1, roles_to_deduplicate=None):
    if roles_to_deduplicate is None:
        roles_to_deduplicate = ["Assistant"]
    params = {"max_allowed": max_allowed, "roles_to_deduplicate": roles_to_deduplicate}
    def apply_to_messages(messages):
        return _drop_duplicates_apply_to_messages(messages, params)
    def apply(segment_text, segment_metadata):
        t = _estimate_tokens(segment_text)
        return segment_text, {"rule": "drop_duplicate_examples", "original_tokens": t, "reduced_tokens": t, "change_description": "no-op on single segment"}
    return {
        "name": "drop_duplicate_examples",
        "description": "Drop exact-duplicate assistant messages beyond a configured count.",
        "enabled": enabled,
        "params": params,
        "apply": apply,
        "apply_to_messages": apply_to_messages,
    }


# ── Rule 3: summarize_repeated_tool_output ────────────────────────────────────

def _summarize_tool_apply_to_messages(messages, params):
    min_repeats = params.get("min_repeats", 3)
    template = params.get("summary_template", "[{tool}: {count} prior results collapsed]")
    tool_indices = {}
    for i in range(len(messages)):
        msg = messages[i]
        if msg.get("role") == "ActionResult":
            tool_name = msg.get("action_name", "unknown")
            if tool_name not in tool_indices:
                tool_indices[tool_name] = []
            tool_indices[tool_name].append(i)

    indices_to_replace = {}
    log = []
    for tool_name in tool_indices:
        idxs = tool_indices[tool_name]
        if len(idxs) < min_repeats:
            continue
        to_collapse = idxs[:-1]
        summary = template.replace("{tool}", tool_name).replace("{count}", str(len(to_collapse)))
        first_idx = to_collapse[0]
        new_msg = {}
        for k in messages[first_idx]:
            new_msg[k] = messages[first_idx][k]
        new_msg["content"] = summary
        indices_to_replace[first_idx] = new_msg
        for i in to_collapse[1:]:
            indices_to_replace[i] = None
        log.append({
            "rule": "summarize_repeated_tool_output",
            "original_tokens": 0,
            "reduced_tokens": 0,
            "change_description": "collapsed {} prior {} result(s)".format(len(to_collapse), tool_name),
        })

    new_messages = []
    for i in range(len(messages)):
        if i not in indices_to_replace:
            new_messages.append(messages[i])
        elif indices_to_replace[i] is not None:
            new_messages.append(indices_to_replace[i])
    return new_messages, log


def make_summarize_repeated_tool_output(enabled=True, min_repeats=3, summary_template=None):
    if summary_template is None:
        summary_template = "[{tool}: {count} prior results collapsed]"
    params = {"min_repeats": min_repeats, "summary_template": summary_template}
    def apply_to_messages(messages):
        return _summarize_tool_apply_to_messages(messages, params)
    def apply(segment_text, segment_metadata):
        t = _estimate_tokens(segment_text)
        return segment_text, {"rule": "summarize_repeated_tool_output", "original_tokens": t, "reduced_tokens": t, "change_description": "no-op on single segment"}
    return {
        "name": "summarize_repeated_tool_output",
        "description": "Collapse repeated ActionResult messages for the same tool into a summary.",
        "enabled": enabled,
        "params": params,
        "apply": apply,
        "apply_to_messages": apply_to_messages,
    }


# ── Rule 4: remove_low_priority_context_blocks ───────────────────────────────

def _remove_blocks_apply(segment_text, segment_metadata, params):
    headings = params.get("headings_to_truncate", [
        "## Prior Knowledge", "## Active Skills", "## Activatable Integrations"
    ])
    replacement = params.get("replacement", "[content removed by budget reduction]")
    original_tokens = _estimate_tokens(segment_text)
    result = segment_text
    changes = []
    for heading in headings:
        idx = result.find(heading)
        if idx < 0:
            continue
        block_start = result.find("\n", idx)
        if block_start < 0:
            continue
        block_start += 1
        next_heading = -1
        for marker in ["\n## ", "\n# "]:
            pos = result.find(marker, block_start)
            if pos >= 0 and (next_heading < 0 or pos < next_heading):
                next_heading = pos
        if next_heading < 0:
            next_heading = len(result)
        body = result[block_start:next_heading]
        if len(body) > len(replacement) + 50:
            result = result[:block_start] + replacement + "\n" + result[next_heading:]
            changes.append(heading)
    reduced_tokens = _estimate_tokens(result)
    desc = "removed bodies of: " + ", ".join(changes) if changes else "no matching headings found"
    return result, {
        "rule": "remove_low_priority_context_blocks",
        "original_tokens": original_tokens,
        "reduced_tokens": reduced_tokens,
        "change_description": desc,
    }


def make_remove_low_priority_context_blocks(enabled=True, headings_to_truncate=None, replacement=None):
    if headings_to_truncate is None:
        headings_to_truncate = ["## Prior Knowledge", "## Active Skills", "## Activatable Integrations"]
    if replacement is None:
        replacement = "[content removed by budget reduction]"
    params = {"headings_to_truncate": headings_to_truncate, "replacement": replacement}
    def apply(segment_text, segment_metadata):
        return _remove_blocks_apply(segment_text, segment_metadata, params)
    return {
        "name": "remove_low_priority_context_blocks",
        "description": "Remove body text of low-priority context blocks when budget is tight.",
        "enabled": enabled,
        "params": params,
        "apply": apply,
        "apply_to_messages": None,
    }


# ── Rule 5: truncate_trailing_content ─────────────────────────────────────────

def _truncate_apply(segment_text, segment_metadata, params):
    max_chars = params.get("max_chars", 4000)
    marker = params.get("truncation_marker", "... [truncated by budget]")
    original_tokens = _estimate_tokens(segment_text)
    if len(segment_text) <= max_chars:
        return segment_text, {
            "rule": "truncate_trailing_content",
            "original_tokens": original_tokens,
            "reduced_tokens": original_tokens,
            "change_description": "no change (within limit)",
        }
    reduced = segment_text[:max_chars] + marker
    reduced_tokens = _estimate_tokens(reduced)
    return reduced, {
        "rule": "truncate_trailing_content",
        "original_tokens": original_tokens,
        "reduced_tokens": reduced_tokens,
        "change_description": "truncated from {} to {} chars".format(len(segment_text), len(reduced)),
    }


def make_truncate_trailing_content(enabled=True, max_chars=4000, truncation_marker=None):
    if truncation_marker is None:
        truncation_marker = "... [truncated by budget]"
    params = {"max_chars": max_chars, "truncation_marker": truncation_marker}
    def apply(segment_text, segment_metadata):
        return _truncate_apply(segment_text, segment_metadata, params)
    return {
        "name": "truncate_trailing_content",
        "description": "Truncate the trailing content of oversized messages, keeping the head.",
        "enabled": enabled,
        "params": params,
        "apply": apply,
        "apply_to_messages": None,
    }


# ── Registry ─────────────────────────────────────────────────────────────────

BUILTIN_RULE_REGISTRY = {
    "compress_whitespace_formatting": make_compress_whitespace_formatting,
    "drop_duplicate_examples": make_drop_duplicate_examples,
    "summarize_repeated_tool_output": make_summarize_repeated_tool_output,
    "remove_low_priority_context_blocks": make_remove_low_priority_context_blocks,
    "truncate_trailing_content": make_truncate_trailing_content,
}

DEFAULT_RULE_ORDER = [
    "compress_whitespace_formatting",
    "drop_duplicate_examples",
    "summarize_repeated_tool_output",
    "remove_low_priority_context_blocks",
    "truncate_trailing_content",
]


def build_rules(rule_configs):
    """Instantiate rule dicts from a list of config dicts (from __get_reduction_rules__()).

    Each config dict: {"name": str, "enabled": bool, "params": dict}
    Returns rules in DEFAULT_RULE_ORDER, then any extras in config order.
    """
    config_by_name = {}
    for cfg in rule_configs:
        name = cfg.get("name")
        if name:
            config_by_name[name] = cfg

    ordered_names = [n for n in DEFAULT_RULE_ORDER if n in config_by_name]
    extras = [cfg.get("name") for cfg in rule_configs
              if cfg.get("name") not in DEFAULT_RULE_ORDER]

    rules = []
    for name in ordered_names + extras:
        cfg = config_by_name.get(name, {})
        if not cfg.get("enabled", True):
            continue
        factory = BUILTIN_RULE_REGISTRY.get(name)
        if factory is None:
            continue
        params = cfg.get("params", {})
        # Build rule with params spread from config
        # (factories accept keyword args matching params keys)
        rule = factory(enabled=True, **params)
        rules.append(rule)
    return rules


def reduce_prompt(messages, token_limit, rule_configs):
    """Apply reduction rules to messages until total tokens fit within token_limit.

    Args:
        messages:     list of working_messages dicts.
        token_limit:  int — maximum total tokens allowed.
        rule_configs: list of rule config dicts from __get_reduction_rules__().

    Returns:
        (reduced_messages: list, reduction_log: list)
    """
    log = []
    current_total = _total_tokens(messages)
    if current_total <= token_limit:
        return messages, log

    rules = build_rules(rule_configs)

    for rule in rules:
        if current_total <= token_limit:
            break
        apply_msgs = rule.get("apply_to_messages")
        if apply_msgs is not None:
            messages, rule_log = apply_msgs(messages)
            log.extend(rule_log)
        else:
            apply = rule.get("apply")
            if apply is None:
                continue
            for i in range(len(messages)):
                msg = messages[i]
                if msg.get("role") == "System":
                    continue  # Never reduce the system prompt
                content = msg.get("content") or ""
                metadata = {"role": msg.get("role"), "index": i}
                reduced_content, entry = apply(content, metadata)
                if reduced_content != content:
                    new_msg = {}
                    for k in msg:
                        new_msg[k] = msg[k]
                    new_msg["content"] = reduced_content
                    messages[i] = new_msg
                    log.append(entry)
        current_total = _total_tokens(messages)

    return messages, log
```

---

### 3.3  `crates/brassclaw_engine/src/executor/orchestrator.rs`

#### 3.3.1 — Update `handle_check_budget` doc comment only (lines 2449–2450)

No implementation change. Replace the doc comment with:

```rust
/// Handle `__check_budget__()`.
///
/// Returns a dict with `tokens_remaining`, `time_remaining_ms`, and
/// `usd_remaining` for telemetry and the post-assembly reduction check.
/// The orchestrator must NOT use this result to stop the turn mid-step;
/// the only hard cutoff is the post-assembly total token check (step 3.5a).
fn handle_check_budget(thread: &Thread) -> ExtFunctionResult {
    // ... implementation unchanged ...
}
```

#### 3.3.2 — Add `__log_budget_warning__` host function and dispatch arm

Add after `handle_check_budget` (around line 2481):

```rust
/// Handle `__log_budget_warning__(field, value, message)`.
///
/// Emits a `BudgetWarning` event. Never stops the turn.
/// `field`: one of "tokens_remaining", "time_remaining_ms", "usd_remaining".
fn handle_log_budget_warning(
    args: &[MontyObject],
    thread: &Thread,
    event_tx: Option<&tokio::sync::broadcast::Sender<ThreadEvent>>,
) -> ExtFunctionResult {
    let field = args.first().map(monty_to_string).unwrap_or_default();
    let value_json = args.get(1).map(monty_to_json).unwrap_or_default();
    let message = args.get(2).map(monty_to_string).unwrap_or_default();

    debug!(
        thread_id = %thread.id,
        budget_field = %field,
        budget_value = ?value_json,
        warning = %message,
        "budget warning (soft)"
    );

    let event = ThreadEvent::new(
        thread.id,
        EventKind::BudgetWarning {
            field,
            value: value_json,
            message,
        },
    );
    if let Some(tx) = event_tx {
        let _ = tx.send(event);
    }

    ExtFunctionResult::Return(MontyObject::None)
}
```

Wire in the `RunProgress::FunctionCall` dispatch match, after the `"__check_budget__"` arm:

```rust
"__log_budget_warning__" => {
    handle_log_budget_warning(args, thread, event_tx)
}
```

#### 3.3.3 — Add `__get_reduction_rules__` host function and dispatch arm

Add after `handle_log_budget_warning`:

```rust
/// Handle `__get_reduction_rules__()`.
///
/// Returns active rule configs as a JSON array. Cached in `thread.metadata`
/// after first call — no DB read per turn.
async fn handle_get_reduction_rules(
    thread: &mut Thread,
    store: Option<&Arc<dyn Store>>,
) -> ExtFunctionResult {
    // Use cached snapshot when present.
    if let Some(cached) = thread.metadata.get("_reduction_rules") {
        return ExtFunctionResult::Return(json_to_monty(cached));
    }

    let rules = load_reduction_rules(store, thread.project_id).await;
    let rules_json = serde_json::to_value(&rules).unwrap_or(serde_json::json!([]));

    if let Some(meta) = thread.metadata.as_object_mut() {
        meta.insert("_reduction_rules".to_string(), rules_json.clone());
    }

    ExtFunctionResult::Return(json_to_monty(&rules_json))
}

/// Load rule configs from the Store, or return defaults.
async fn load_reduction_rules(
    store: Option<&Arc<dyn Store>>,
    project_id: ProjectId,
) -> Vec<serde_json::Value> {
    let Some(store) = store else {
        return default_reduction_rules();
    };
    match store.list_shared_memory_docs(project_id).await {
        Ok(docs) => {
            let stored: Vec<serde_json::Value> = docs
                .iter()
                .filter(|d| d.tags.contains(&"reduction_rule_config".to_string()))
                .filter_map(|d| serde_json::from_str(&d.content).ok())
                .collect();
            if stored.is_empty() { default_reduction_rules() } else { stored }
        }
        Err(_) => default_reduction_rules(),
    }
}

fn default_reduction_rules() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({"name":"compress_whitespace_formatting","enabled":true,"params":{"max_consecutive_newlines":2}}),
        serde_json::json!({"name":"drop_duplicate_examples","enabled":true,"params":{"max_allowed":1,"roles_to_deduplicate":["Assistant"]}}),
        serde_json::json!({"name":"summarize_repeated_tool_output","enabled":true,"params":{"min_repeats":3,"summary_template":"[{tool}: {count} prior results collapsed]"}}),
        serde_json::json!({"name":"remove_low_priority_context_blocks","enabled":true,"params":{"headings_to_truncate":["## Prior Knowledge","## Active Skills","## Activatable Integrations"],"replacement":"[content removed by budget reduction]"}}),
        serde_json::json!({"name":"truncate_trailing_content","enabled":true,"params":{"max_chars":4000,"truncation_marker":"... [truncated by budget]"}}),
    ]
}
```

Wire in dispatch:

```rust
"__get_reduction_rules__" => {
    handle_get_reduction_rules(thread, store).await
}
```

Update the module-level doc comment (lines 9–19) to list the two new host functions.

#### 3.3.4 — Add `author_reduction_rule` public function (LLM-authoring round-trip)

This is invoked via a dedicated HTTP endpoint (§3.6), not from within `execute_orchestrator`.

```rust
/// Author a new reduction rule from a natural-language description.
///
/// Makes a single LLM call with a strict prompt template that requests
/// a Monty-compatible Python function-dict factory (no `class` syntax).
/// Returns the validated Python source, or an error if the LLM call fails
/// or the output fails the interface check.
///
/// Round-trip format (matches `segment_reduction.py` factory contract):
///   - Source must define a callable `make_rule(*kwargs)` returning a dict
///     with keys: name, description, enabled, params, apply.
///   - `apply` must be a callable: (segment_text: str, metadata: dict)
///     -> (reduced_text: str, log_entry: dict).
///   - `log_entry` must contain: rule, original_tokens, reduced_tokens,
///     change_description.
pub async fn author_reduction_rule(
    description: &str,
    llm: &Arc<dyn LlmBackend>,
) -> Result<String, EngineError> {
    use crate::types::message::ThreadMessage;

    let prompt = build_rule_authoring_prompt(description);
    let config = LlmCallConfig {
        force_text: true,
        max_tokens: Some(2048),
        ..LlmCallConfig::default()
    };
    let messages = vec![ThreadMessage::user(prompt)];

    let output = llm
        .complete(&messages, &[], &config)
        .await
        .map_err(|e| EngineError::Effect {
            reason: format!("LLM call for rule authoring failed: {e}"),
        })?;

    let source = match output.response {
        crate::types::step::LlmResponse::Text(text) => extract_python_source(&text),
        _ => return Err(EngineError::Effect {
            reason: "Rule authoring LLM returned non-text response".into(),
        }),
    };

    validate_rule_source(&source)?;
    Ok(source)
}

fn build_rule_authoring_prompt(description: &str) -> String {
    format!(
        r#"You are generating a Monty-compatible Python factory function for a prompt \
reduction rule in BrassClaw.

DESCRIPTION: {description}

IMPORTANT CONSTRAINTS:
- Monty v0.0.16 does NOT support `class` statements. Use functions and dicts only.
- No imports. No `re` module. Use plain string operations only.
- Token estimate helper: (len(text) + 3) // 4

You must produce a single Python factory function with this EXACT signature and
return shape. No markdown, no explanation — Python source only.

REQUIRED STRUCTURE:
def make_rule(enabled=True, **params):
    # Your rule logic here using closures
    def apply(segment_text, segment_metadata):
        original_tokens = (len(segment_text) + 3) // 4
        # ... your reduction logic ...
        reduced_tokens = (len(segment_text) + 3) // 4
        return segment_text, {{
            "rule": "<your_rule_name>",
            "original_tokens": original_tokens,
            "reduced_tokens": reduced_tokens,
            "change_description": "...",
        }}
    return {{
        "name": "<your_rule_name>",
        "description": "<one sentence>",
        "enabled": enabled,
        "params": dict(params),
        "apply": apply,
        "apply_to_messages": None,
    }}

OUTPUT ONLY THE PYTHON FUNCTION. No markdown, no triple-backticks, no explanation.
"#
    )
}

fn extract_python_source(llm_output: &str) -> String {
    // Strip markdown code fences if the LLM wrapped the output.
    let stripped = llm_output.trim();
    if stripped.starts_with("```python") {
        let inner = stripped
            .trim_start_matches("```python")
            .trim_start_matches("```");
        if let Some(end) = inner.rfind("```") {
            return inner[..end].trim().to_string();
        }
        return inner.trim().to_string();
    }
    if stripped.starts_with("```") {
        let inner = stripped.trim_start_matches("```");
        if let Some(end) = inner.rfind("```") {
            return inner[..end].trim().to_string();
        }
        return inner.trim().to_string();
    }
    stripped.to_string()
}

fn validate_rule_source(source: &str) -> Result<(), EngineError> {
    // Syntax check: attempt to parse with Monty's parser.
    let _ = MontyRun::new(source.to_string(), "rule_validation.py", vec![])
        .map_err(|e| EngineError::Effect {
            reason: format!("Rule source syntax error: {e}"),
        })?;

    // Interface check: required names present as plain-text scan.
    for required in &["def make_rule", "def apply", '"apply"', "'apply'"] {
        if source.contains(required) {
            // At least one apply form found
            break;
        }
    }
    if !source.contains("def make_rule") {
        return Err(EngineError::Effect {
            reason: "Rule source must define a function named 'make_rule'".into(),
        });
    }
    if !source.contains('"name"') && !source.contains("'name'") {
        return Err(EngineError::Effect {
            reason: "Rule source must set a 'name' key in the returned dict".into(),
        });
    }
    if !source.contains('"apply"') && !source.contains("'apply'") {
        return Err(EngineError::Effect {
            reason: "Rule source must set an 'apply' key in the returned dict".into(),
        });
    }
    Ok(())
}
```

---

### 3.4  `crates/brassclaw_engine/src/types/event.rs`

Add two new variants to `EventKind`:

```rust
/// Emitted when a budget value is low but execution continues.
/// Never stops the turn — telemetry only.
BudgetWarning {
    field: String,       // "tokens_remaining" | "time_remaining_ms" | "usd_remaining"
    value: serde_json::Value,
    message: String,
},

/// Emitted when the fully assembled prompt exceeds the token limit
/// and the reduction pipeline is about to run.
PromptOverBudget {
    total_tokens: u64,
    limit: u64,
},
```

**Location:** After the existing `SkillActivated` variant.

---

### 3.5  Downstream call-site update: `default_with_full_config` now takes 5 arguments

`families/mod.rs` was externally updated to add `inline_control_tokens: Option<usize>`
as the 5th parameter. Every existing call site must be updated to pass this argument.

**File:** `crates/brassclaw_reborn/src/app_loop_family.rs`

Find the call to `families::default_with_full_config(...)` and add `None` (or the
resolved value from `LoopFamilyConfig`) as the 5th argument:

```rust
// Before (4 args — now a compile error):
families::default_with_full_config(
    config.conversation_token_budget,
    capability_focus,
    planning_context,
    config.context_window_tokens,
)

// After (5 args):
families::default_with_full_config(
    config.conversation_token_budget,
    capability_focus,
    planning_context,
    config.context_window_tokens,
    config.inline_control_tokens,   // new — None if not yet plumbed
)
```

Also add `inline_control_tokens: Option<usize>` to `LoopFamilyConfig` if it is
not already there, and thread it through `build_loop_family_registry_with_full_config`
and `DefaultPlannedRuntimeConfig` following the same pattern as
`context_window_tokens`.

**Note:** This is a compile-time break introduced by the external change. Any
`cargo build` will fail until this call site is updated. This must be the **first**
task in the execution checklist.

---

### 3.6  Backend: HTTP endpoints for rule management

**New routes** in `crates/brassclaw_webui_v2/src/router.rs`:

| Route ID | Method | Pattern | Purpose |
|---|---|---|---|
| `webui.v2.list_reduction_rules` | GET | `/api/webchat/v2/tokens/reduction-rules` | List active rule configs |
| `webui.v2.update_reduction_rules` | PUT | `/api/webchat/v2/tokens/reduction-rules` | Persist rule configs to Store |
| `webui.v2.author_reduction_rule` | POST | `/api/webchat/v2/tokens/reduction-rules/author` | LLM-author a rule from description |

Handlers call new facade methods on `RebornServicesApi`:

```rust
async fn list_reduction_rules(&self, caller: WebUiAuthenticatedCaller)
    -> Result<Vec<ReductionRuleConfig>, ServiceError>;

async fn update_reduction_rules(&self, caller: WebUiAuthenticatedCaller,
    rules: Vec<ReductionRuleConfig>)
    -> Result<(), ServiceError>;

async fn author_reduction_rule(&self, caller: WebUiAuthenticatedCaller,
    description: String)
    -> Result<AuthoredRuleResponse, ServiceError>;
```

Wire types (in `crates/brassclaw_product_workflow/`):

```rust
pub struct ReductionRuleConfig {
    pub name: String,
    pub enabled: bool,
    pub params: serde_json::Value,
    pub is_builtin: bool,
    pub python_source: Option<String>,  // user-authored rules only
}

pub struct AuthoredRuleResponse {
    pub python_source: String,
    pub rule_config: ReductionRuleConfig,
}
```

**Implementation notes (all follow the rules stated in `AGENTS.md`):**
- `list_reduction_rules`: reads MemoryDocs tagged `reduction_rule_config`.
  Returns the 5 defaults if none stored.
- `update_reduction_rules`: upserts one MemoryDoc per rule config, tagged
  `reduction_rule_config`. Overwrites by name match.
- `author_reduction_rule`: calls `author_reduction_rule()` from `brassclaw_engine`;
  on success stores source as MemoryDoc tagged `reduction_rule_source`, and the
  config doc tagged `reduction_rule_config`.
- Handlers must not import `brassclaw_engine` directly — the composition layer
  wraps the engine call inside the `RebornServices` implementation.

---

### 3.7  Frontend: "Prompt creation" settings segment

**`settings-page.js`** — add "Prompt creation" as a top-level nav tab.

**New: `prompt-creation-tab.js`** — renders the Prompt-Segment Reduction Rules tab:
- Lists all 5 default rules (name + description).
- Enable/disable toggle per rule → calls `update_reduction_rules` PUT.
- Per-param editable fields (number input for int/float, text for string, tag
  input for list) → calls PUT on blur/save.
- Delete button → removes rule from list + calls PUT.
- "Add rule" button → opens modal with description textarea + "Generate rule"
  → POST to `author_reduction_rule` → shows generated Python in read-only
  code block for review → "Save" or "Discard".

**New: `reduction-rules-api.js`**:

```javascript
export async function listReductionRules(baseUrl, token) {
  const res = await fetch(`${baseUrl}/api/webchat/v2/tokens/reduction-rules`, {
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!res.ok) throw new Error(`listReductionRules: ${res.status}`);
  return res.json();
}

export async function updateReductionRules(baseUrl, token, rules) {
  const res = await fetch(`${baseUrl}/api/webchat/v2/tokens/reduction-rules`, {
    method: "PUT",
    headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
    body: JSON.stringify({ rules }),
  });
  if (!res.ok) throw new Error(`updateReductionRules: ${res.status}`);
  return res.json();
}

export async function authorReductionRule(baseUrl, token, description) {
  const res = await fetch(`${baseUrl}/api/webchat/v2/tokens/reduction-rules/author`, {
    method: "POST",
    headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
    body: JSON.stringify({ description }),
  });
  if (!res.ok) throw new Error(`authorReductionRule: ${res.status}`);
  return res.json();
}
```

**`en.js`** — add 10 i18n keys:

```javascript
"settings.prompt_creation.title": "Prompt creation",
"settings.prompt_creation.reduction_rules.title": "Prompt-Segment Reduction Rules",
"settings.prompt_creation.reduction_rules.add": "Add rule",
"settings.prompt_creation.reduction_rules.describe_placeholder": "Describe what your rule should do...",
"settings.prompt_creation.reduction_rules.generate": "Generate rule",
"settings.prompt_creation.reduction_rules.review_source": "Review generated Python source",
"settings.prompt_creation.reduction_rules.save": "Save rule",
"settings.prompt_creation.reduction_rules.discard": "Discard",
"settings.prompt_creation.reduction_rules.enabled": "Enabled",
"settings.prompt_creation.reduction_rules.delete": "Delete rule",
```

---

## 4. Test Changes

### 4.1  New Rust tests

**`crates/brassclaw_engine/src/executor/orchestrator.rs`** — test block:

```rust
#[test]
fn check_budget_returns_well_formed_dict_and_does_not_stop_turn() {
    use crate::types::thread::{Thread, ThreadConfig};
    let config = ThreadConfig {
        max_tokens_total: Some(100),
        max_duration: None,
        max_budget_usd: Some(1.0),
        ..Default::default()
    };
    let thread = Thread::new_for_test(config);
    let result = handle_check_budget(&thread);
    match result {
        ExtFunctionResult::Return(obj) => {
            let json = monty_to_json(&obj);
            assert!(json.get("tokens_remaining").is_some());
            assert!(json.get("time_remaining_ms").is_some());
            assert!(json.get("usd_remaining").is_some());
        }
        other => panic!("expected Return, got {:?}", other),
    }
}

#[test]
fn default_reduction_rules_returns_five_entries_in_default_order() {
    let rules = default_reduction_rules();
    assert_eq!(rules.len(), 5);
    let names: Vec<_> = rules.iter()
        .filter_map(|r| r.get("name").and_then(|v| v.as_str()))
        .collect();
    assert_eq!(names[0], "compress_whitespace_formatting");
    assert_eq!(names[4], "truncate_trailing_content");
}
```

### 4.2  New Python tests

**New file: `crates/brassclaw_engine/orchestrator/test_segment_reduction.py`**

```python
# test_segment_reduction.py — unit tests for segment_reduction module.
# Run with: python test_segment_reduction.py (CPython) or via the Monty test harness.

import sys
sys.path.insert(0, ".")

from segment_reduction import (
    make_truncate_trailing_content,
    make_remove_low_priority_context_blocks,
    make_compress_whitespace_formatting,
    make_drop_duplicate_examples,
    make_summarize_repeated_tool_output,
    reduce_prompt,
    _total_tokens,
    DEFAULT_RULE_ORDER,
)


def test_truncate_shortens_long_segment():
    rule = make_truncate_trailing_content(max_chars=10)
    text = "A" * 100
    reduced, log = rule["apply"](text, {})
    assert len(reduced) <= 10 + len("... [truncated by budget]")
    assert log["original_tokens"] > log["reduced_tokens"]


def test_truncate_noop_within_limit():
    rule = make_truncate_trailing_content(max_chars=1000)
    reduced, log = rule["apply"]("short", {})
    assert reduced == "short"
    assert "no change" in log["change_description"]


def test_compress_whitespace_collapses_blank_lines():
    rule = make_compress_whitespace_formatting(max_consecutive_newlines=2)
    text = "line1\n\n\n\n\nline2"
    reduced, log = rule["apply"](text, {})
    assert "\n\n\n" not in reduced
    assert "line1" in reduced and "line2" in reduced


def test_remove_blocks_strips_known_heading_body():
    rule = make_remove_low_priority_context_blocks()
    text = "## Prior Knowledge\nSome long content here.\n## Next Section\nKept."
    reduced, log = rule["apply"](text, {})
    assert "Some long content here" not in reduced
    assert "## Prior Knowledge" in reduced
    assert "Kept." in reduced


def test_drop_duplicates_removes_repeated_assistant():
    rule = make_drop_duplicate_examples(max_allowed=1)
    messages = [
        {"role": "User", "content": "hi"},
        {"role": "Assistant", "content": "hello"},
        {"role": "Assistant", "content": "hello"},
        {"role": "User", "content": "bye"},
    ]
    new_msgs, log = rule["apply_to_messages"](messages)
    asst = [m for m in new_msgs if m["role"] == "Assistant"]
    assert len(asst) == 1
    assert log[0]["change_description"].startswith("dropped 1")


def test_summarize_repeated_tool_collapses_actions():
    rule = make_summarize_repeated_tool_output(min_repeats=3)
    messages = [
        {"role": "ActionResult", "action_name": "search", "content": "r1"},
        {"role": "ActionResult", "action_name": "search", "content": "r2"},
        {"role": "ActionResult", "action_name": "search", "content": "r3"},
        {"role": "ActionResult", "action_name": "search", "content": "r4"},
    ]
    new_msgs, log = rule["apply_to_messages"](messages)
    search_msgs = [m for m in new_msgs if m.get("action_name") == "search"]
    assert len(search_msgs) < 4
    assert len(log) > 0


def test_reduce_prompt_noop_when_under_limit():
    rule_configs = [{"name": "truncate_trailing_content", "enabled": True, "params": {"max_chars": 4000}}]
    messages = [{"role": "System", "content": "system"}, {"role": "User", "content": "hi"}]
    reduced, log = reduce_prompt(messages, 100000, rule_configs)
    assert reduced == messages
    assert log == []


def test_reduce_prompt_applies_rules_when_over_limit():
    rule_configs = [{"name": "truncate_trailing_content", "enabled": True, "params": {"max_chars": 5, "truncation_marker": "..."}}]
    messages = [
        {"role": "System", "content": "system"},
        {"role": "User", "content": "A" * 5000},
    ]
    reduced, log = reduce_prompt(messages, 10, rule_configs)
    assert len(log) > 0
    user_msg = next(m for m in reduced if m["role"] == "User")
    assert len(user_msg["content"]) <= 5 + len("...")


def test_system_prompt_never_reduced():
    rule_configs = [{"name": "truncate_trailing_content", "enabled": True, "params": {"max_chars": 1}}]
    sys_content = "system " * 1000
    messages = [{"role": "System", "content": sys_content}]
    reduced, log = reduce_prompt(messages, 1, rule_configs)
    assert reduced[0]["content"] == sys_content


def test_default_rule_order_has_five_entries():
    assert len(DEFAULT_RULE_ORDER) == 5
    assert DEFAULT_RULE_ORDER[0] == "compress_whitespace_formatting"
    assert DEFAULT_RULE_ORDER[4] == "truncate_trailing_content"


def test_rule_config_round_trip():
    rule = make_truncate_trailing_content(enabled=False, max_chars=999)
    assert rule["name"] == "truncate_trailing_content"
    assert rule["enabled"] is False
    assert rule["params"]["max_chars"] == 999


if __name__ == "__main__":
    test_truncate_shortens_long_segment()
    test_truncate_noop_within_limit()
    test_compress_whitespace_collapses_blank_lines()
    test_remove_blocks_strips_known_heading_body()
    test_drop_duplicates_removes_repeated_assistant()
    test_summarize_repeated_tool_collapses_actions()
    test_reduce_prompt_noop_when_under_limit()
    test_reduce_prompt_applies_rules_when_over_limit()
    test_system_prompt_never_reduced()
    test_default_rule_order_has_five_entries()
    test_rule_config_round_trip()
    print("All segment_reduction tests passed.")
```

### 4.3  Existing tests that need updating

| Test | Failure cause | Fix |
|------|-------------|-----|
| `cargo build` on `app_loop_family.rs` | `default_with_full_config` now takes 5 args | Add 5th arg (see §3.5). This is a **compile error**, not a test failure. |
| `webui_v2_descriptors_contract` | Descriptor count +3 | Update the asserted count; add 3 new descriptors to the expected table. |
| `webui_v2_handlers_contract` | New handlers need stubs | Add stub implementations for the 3 new `RebornServicesApi` methods. |
| Any exhaustive `EventKind` match | Two new variants | Add `EventKind::BudgetWarning { .. } => ...` and `EventKind::PromptOverBudget { .. } => ...` match arms. |
| Any test asserting `"Token budget exhausted."` string | Stop logic removed | No such tests exist in the engine test block (confirmed by audit). If any E2E test asserts this string, change it to assert on `BudgetWarning` event emission. |

---

## 5. Execution Checklist

- [ ] **A0** — Fix compile break: update `app_loop_family.rs` to pass 5 args to `default_with_full_config` (§3.5). Run `cargo build -p brassclaw_reborn` to confirm.
- [ ] **A1** — Audit complete: all impacted locations confirmed (§0).
- [ ] **A2** — Confirm `token-budget-next-step.md` Steps 1–4 + `inline_control_tokens` all done (§1).
- [ ] **B1** — Create `orchestrator/segment_reduction.py` (§3.2).
- [ ] **B2** — Modify `orchestrator/default.py`: remove hard-stop block; add telemetry block; add post-assembly check (§3.1).
- [ ] **B3** — Add `EventKind::BudgetWarning` and `EventKind::PromptOverBudget` to `types/event.rs` (§3.4).
- [ ] **B4** — Update `handle_check_budget` doc comment (§3.3.1).
- [ ] **B5** — Add `handle_log_budget_warning` + dispatch arm (§3.3.2).
- [ ] **B6** — Add `handle_get_reduction_rules`, `load_reduction_rules`, `default_reduction_rules` + dispatch arm (§3.3.3).
- [ ] **B7** — Add `author_reduction_rule`, `build_rule_authoring_prompt`, `extract_python_source`, `validate_rule_source` (§3.3.4).
- [ ] **B8** — Update orchestrator.rs module-level doc comment for new host functions.
- [ ] **C1** — Add 3 new route descriptors and handlers in `brassclaw_webui_v2` (§3.6).
- [ ] **C2** — Add `list_reduction_rules`, `update_reduction_rules`, `author_reduction_rule` to `RebornServicesApi` + `RebornServices` (§3.6).
- [ ] **C3** — Add wire types `ReductionRuleConfig`, `AuthoredRuleResponse` to `brassclaw_product_workflow` (§3.6).
- [ ] **C4** — Add `reduction-rules-api.js` (§3.7).
- [ ] **C5** — Add `prompt-creation-tab.js` (§3.7).
- [ ] **C6** — Update `settings-page.js` navigation (§3.7).
- [ ] **C7** — Add 10 i18n keys to `en.js` (§3.7).
- [ ] **D1** — Write `test_segment_reduction.py` (§4.2).
- [ ] **D2** — Add `check_budget_returns_well_formed_dict` and `default_reduction_rules_returns_five_entries` Rust tests (§4.1).
- [ ] **D3** — Update `webui_v2_descriptors_contract` route count + expected descriptors (§4.3).
- [ ] **D4** — Update `webui_v2_handlers_contract` stubs (§4.3).
- [ ] **D5** — Add exhaustive match arms for new `EventKind` variants wherever they are matched (§4.3).
- [ ] **E1** — `cargo build -p brassclaw_reborn` (must be clean before anything else).
- [ ] **E2** — `cargo test -p brassclaw_engine`.
- [ ] **E3** — `cargo test -p brassclaw_webui_v2`.
- [ ] **E4** — `cargo test -p brassclaw_architecture reborn_crate_dependency_boundaries_hold`.
- [ ] **E5** — `cargo test` (full suite).
- [ ] **E6** — `cargo clippy --all --benches --tests --examples --all-features -- -D warnings`.

---

## 6. Design Invariants Preserved

- **No `.unwrap()` or `.expect()` in production paths.** All new Rust code uses `map_err`, `?`, `unwrap_or`, or explicit match.
- **No `info!` or `warn!` in hot-path code.** Budget telemetry uses `debug!` only.
- **`live_context_budget` remains the single write point.** The reduction pipeline reads messages from the orchestrator's working transcript; it never touches `LiveTokenBudget` or `DefaultContextStrategy`.
- **No new fields on `LoopExecutionState`.** The reduction log lives in `state["last_reduction_log"]` inside the Python orchestrator's opaque state dict.
- **No DB reads during a running turn.** `handle_get_reduction_rules` caches in `thread.metadata["_reduction_rules"]` after the first call. Only one Store read per thread lifetime, not per step.
- **`segment_reduction.py` uses only Monty-supported constructs.** No `class`, no `with`, no `match`, no `del`, no `yield`. Uses `import` for the module import in `default.py` (which Monty supports) and no stdlib imports inside `segment_reduction.py` itself.
- **Steps 7–10 from `token-budget-next-step.md` are independent.** This plan adds no `cache_retention` fields, no `LoopModelUsage` cache fields, and no tool-definition sorting. Those belong in separate PRs per the execution order table in `token-budget-next-step.md`.

---

## 7. Summary of All Changed Files

| File | Change type | Description |
|------|-------------|-------------|
| `crates/brassclaw_engine/orchestrator/default.py` | Modify | Remove 9-line hard-stop block; add telemetry block; add post-assembly reduction trigger |
| `crates/brassclaw_engine/orchestrator/segment_reduction.py` | **New** | 5 reduction rule factories + `reduce_prompt` function (Monty-compatible, class-free) |
| `crates/brassclaw_engine/orchestrator/test_segment_reduction.py` | **New** | 11 Python unit tests for `segment_reduction.py` |
| `crates/brassclaw_engine/src/executor/orchestrator.rs` | Modify | Updated doc; `handle_log_budget_warning`; `handle_get_reduction_rules`; `load_reduction_rules`; `default_reduction_rules`; `author_reduction_rule` + helpers; dispatch arms; module doc |
| `crates/brassclaw_engine/src/types/event.rs` | Modify | Add `BudgetWarning` and `PromptOverBudget` variants |
| `crates/brassclaw_reborn/src/app_loop_family.rs` | **Compile fix** | Add 5th arg `inline_control_tokens` to `default_with_full_config` call; add field to `LoopFamilyConfig` |
| `crates/brassclaw_webui_v2/src/router.rs` | Modify | 3 new route descriptors |
| `crates/brassclaw_webui_v2/src/handlers.rs` (+ handlers/) | Modify | 3 new handler implementations |
| `crates/brassclaw_product_workflow/src/reborn_services.rs` | Modify | 3 new facade methods + `ReductionRuleConfig` / `AuthoredRuleResponse` wire types |
| `crates/brassclaw_webui_v2_static/static/js/pages/settings/settings-page.js` | Modify | Add "Prompt creation" nav entry |
| `crates/brassclaw_webui_v2_static/static/js/pages/settings/components/prompt-creation-tab.js` | **New** | Full settings tab UI component |
| `crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/reduction-rules-api.js` | **New** | 3 API helper functions |
| `crates/brassclaw_webui_v2_static/static/js/i18n/en.js` | Modify | 10 new i18n keys |
| `crates/brassclaw_webui_v2/tests/webui_v2_descriptors_contract.rs` | Modify | Update route count; add 3 expected descriptors |
| `crates/brassclaw_webui_v2/tests/webui_v2_handlers_contract.rs` | Modify | Add stub methods for 3 new endpoints |
