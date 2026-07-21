# Engine v2 Orchestrator (default, v0)
#
# This is the self-modifiable execution loop. It replaces the Rust
# ExecutionLoop::run() with Python that can be patched at runtime
# by the self-improvement Mission.
#
# Host functions (provided by Rust via Monty suspension):
#   __llm_complete__(messages, actions, config)  -> response dict
#   __execute_code_step__(code, state)           -> result dict
#   __execute_action__(name, params)             -> result dict
#   __execute_actions_parallel__(calls)          -> list of result dicts (parallel execution)
#   __check_signals__()                          -> None | "stop" | {"inject": msg}
#   __emit_event__(kind, **data)                 -> None
#   __save_checkpoint__(state, counters)         -> None
#   __transition_to__(state, reason)             -> None
#   __retrieve_docs__(goal, max_docs)            -> list of doc dicts
#   __check_budget__()                           -> budget dict
#   __get_actions__()                            -> list of action dicts
#   __list_skills__()                            -> list of skill dicts
#   __record_skill_usage__(doc_id, success)      -> None
#   __regex_match__(pattern, text)               -> bool
#   __validate_component__(title, content, doc_type, metadata)
#                                               -> {queued, candidate_id, ...}
#
# Context variables (injected by Rust before execution):
#   context  - list of prior messages [{role, content}]
#   goal     - thread goal string
#   actions  - list of available action defs
#   state    - persisted state dict from prior steps
#   config   - thread config dict


# ── Helper functions (self-modifiable glue) ──────────────────
# Defined before run_loop so they are in scope when called.


def extract_final(text):
    """Extract FINAL() content from text. Returns None if not found."""
    idx = text.find("FINAL(")
    if idx < 0:
        return None
    after = text[idx + 6:]
    # Handle triple-quoted strings
    for q in ['"""', "'''"]:
        if after.startswith(q):
            end = after.find(q, len(q))
            if end >= 0:
                return after[len(q):end]
    # Handle single/double quoted strings
    if after and after[0] in ('"', "'"):
        quote = after[0]
        end = after.find(quote, 1)
        if end >= 0:
            return after[1:end]
    # Handle balanced parens
    depth = 1
    for i, ch in enumerate(after):
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
            if depth == 0:
                return after[:i]
    return None


def strip_quoted_strings(line):
    """Remove double-quoted string literals from a line."""
    result = []
    in_quote = False
    prev = ""
    for ch in line:
        if ch == '"' and prev != "\\":
            in_quote = not in_quote
            prev = ch
            continue
        if not in_quote:
            result.append(ch)
        prev = ch
    return "".join(result)


def strip_code_blocks(text):
    """Strip fenced code blocks, indented code lines, and double-quoted strings."""
    result = []
    in_fence = False
    for line in text.split("\n"):
        trimmed = line.lstrip()
        if trimmed.startswith("```"):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        if line.startswith("    ") or line.startswith("\t"):
            continue
        result.append(strip_quoted_strings(line))
    return "\n".join(result)



def format_output(result, max_chars=8000):
    """Format code execution result for the next LLM context message."""
    parts = []

    stdout = result.get("stdout", "")
    if stdout:
        parts.append("[stdout]\n" + stdout)

    for r in result.get("action_results", []):
        name = r.get("action_name", "?")
        output = str(r.get("output", ""))
        if r.get("is_error"):
            parts.append("[" + name + " ERROR] " + output)
        else:
            if len(output) > 500:
                preview = output[:500] + "..."
                parts.append(
                    "[" + name + "] " + preview +
                    "\n(full result stored in state['" + name + "']; "
                    "do NOT retype the data — reference the variable in your next call.)"
                )
            else:
                parts.append("[" + name + "] " + output)

    ret = result.get("return_value")
    if ret is not None:
        parts.append("[return] " + str(ret))

    text = "\n\n".join(parts)

    # Truncate from the front (keep the tail with most recent results)
    if len(text) > max_chars:
        text = "... (truncated) ...\n" + text[-max_chars:]

    if not text:
        text = "[code executed, no output]"

    return text


def ensure_working_messages(state, context):
    """Initialize the mutable orchestrator transcript."""
    existing = state.get("working_messages")
    if isinstance(existing, list):
        return existing
    if isinstance(context, list):
        state["working_messages"] = list(context)
    else:
        state["working_messages"] = []
    return state["working_messages"]


def append_message(messages, role, content, action_name=None, action_call_id=None, action_calls=None):
    """Append a normalized message to the working transcript."""
    msg = {"role": role, "content": content}
    if action_name is not None:
        msg["action_name"] = action_name
    if action_call_id is not None:
        msg["action_call_id"] = action_call_id
    if action_calls is not None:
        msg["action_calls"] = action_calls
    messages.append(msg)


# Conservative fallback heuristic matching the old Rust-side estimator.
# These MUST be defined before `estimate_context_tokens` (and therefore
# before the `FINAL(result)` entry-point call below). Moving them after the
# entry point is a latent NameError every time `compact_if_needed` runs.
CHARS_PER_TOKEN = 4
MESSAGE_OVERHEAD_CHARS = 4


def estimate_context_tokens(messages):
    """Estimate token count for a transcript using a rough chars/token heuristic."""
    total_chars = 0
    for msg in messages:
        total_chars += len(msg.get("content", ""))
        total_chars += len(msg.get("action_name", "") or "")
        total_chars += MESSAGE_OVERHEAD_CHARS
    return (total_chars + CHARS_PER_TOKEN - 1) // CHARS_PER_TOKEN


# ---------------------------------------------------------------------------
# Segment reduction helpers (Monty-safe subset).
#
# These five rule factories + `_reduce_prompt` implement the soft-budget
# reduction pipeline that runs after the orchestrator has assembled a message
# list but before the LLM call. They are intentionally pure Python: no
# imports, no `with`, no comprehensions with `if`, no `match`, no classes, no
# closures captured by reference. They are reachable from the Monty VM.
#
# Monty-safe conventions used throughout:
#   - no `import` statements
#   - no classes (only functions and dicts)
#   - no f-strings (use `"...".format(...)`)
#   - no list comprehensions with `if` (use explicit for-loops)
#   - no `match`, `del`, `yield`, `with`
#   - no `any` / `all` builtins (use explicit loops)
# ---------------------------------------------------------------------------

REDUCE_TYPE_TRUNCATE = "truncate"
REDUCE_TYPE_SUMMARIZE = "summarize"
REDUCE_TYPE_DROP = "drop"
REDUCE_TYPE_PRIORITY = "priority"
REDUCE_TYPE_HISTORY_COMPACT = "history_compact"

VALID_REDUCE_TYPES = [
    REDUCE_TYPE_TRUNCATE,
    REDUCE_TYPE_SUMMARIZE,
    REDUCE_TYPE_DROP,
    REDUCE_TYPE_PRIORITY,
    REDUCE_TYPE_HISTORY_COMPACT,
]


def make_truncate_rule(field, max_chars):
    """Build a rule that truncates `field` in the last user message to `max_chars`."""
    return {
        "type": REDUCE_TYPE_TRUNCATE,
        "field": field,
        "max_chars": int(max_chars),
    }


def make_summarize_rule(field):
    """Build a rule that marks `field` for LLM-based summarization.

    The host runtime performs the actual summarization on the next turn.
    The Python pipeline just records the request so the post-assembly
    enforcement step knows to skip this field rather than drop it.
    """
    return {
        "type": REDUCE_TYPE_SUMMARIZE,
        "field": field,
    }


def make_drop_rule(field):
    """Build a rule that removes `field` from the last user message entirely."""
    return {
        "type": REDUCE_TYPE_DROP,
        "field": field,
    }


def make_priority_rule(fields_priority_list):
    """Build a rule that drops fields lowest-priority-first until under budget.

    `fields_priority_list` is a list of field names ordered from highest to
    lowest priority. The reduction pipeline walks the list in reverse and
    drops fields until the budget is satisfied or only the highest-priority
    field remains.
    """
    return {
        "type": REDUCE_TYPE_PRIORITY,
        "fields": list(fields_priority_list),
    }


def make_history_compact_rule(keep_recent_n):
    """Build a rule that keeps only the N most recent conversation messages."""
    return {
        "type": REDUCE_TYPE_HISTORY_COMPACT,
        "keep_recent_n": int(keep_recent_n),
    }


def _truncate_field_in_message(message, field, max_chars):
    """Truncate a top-level field on `message` to at most `max_chars` chars.

    A non-positive `max_chars` clears the field. Monty-safe: returns a
    new dict rather than slicing in place so the caller's reference
    remains valid for the next rule's read.
    """
    if not isinstance(message, dict):
        return message
    raw = message.get(field, "")
    if not isinstance(raw, str):
        return message
    if max_chars <= 0:
        new_message = dict(message)
        new_message[field] = ""
        return new_message
    if len(raw) <= max_chars:
        return message
    suffix = "..."
    keep = max_chars - len(suffix)
    if keep < 0:
        keep = 0
    truncated = raw[:keep] + suffix
    new_message = dict(message)
    new_message[field] = truncated
    return new_message


def _drop_field_in_message(message, field):
    """Remove `field` from `message`. Returns a new dict to keep the call site pure.

    Avoids the `del` statement per the Monty-safe subset — uses a
    manual copy through `dict.pop` is also off-limits for the same
    reason, so we rebuild with key iteration instead.
    """
    if not isinstance(message, dict):
        return message
    if field not in message:
        return message
    new_message = {}
    for key in message:
        if key != field:
            new_message[key] = message[key]
    return new_message


def _summarize_field_in_message(message, field):
    """Mark a field as targeted for summarization on the next turn.

    The Python pipeline never performs the summarization itself — Monty
    has no LLM handle. We leave the content untouched but flag it so the
    post-assembly pass can avoid retrying the same rule on the next turn.
    """
    if not isinstance(message, dict):
        return message
    new_message = dict(message)
    # Copy the nested flags dict so we never mutate the original message's
    # `_reduction_flags` (which `new_message` shares by reference until
    # we replace it). Without this copy, calling this helper twice on two
    # different messages from the same source list would corrupt the
    # first message.
    flags_source = new_message.get("_reduction_flags", {})
    if not isinstance(flags_source, dict):
        flags_source = {}
    flags = dict(flags_source)
    flags[field] = "summarize"
    new_message["_reduction_flags"] = flags
    return new_message


def _priority_drop_until_under_budget(messages, fields, budget_tokens):
    """Drop `fields` (lowest-priority tail-first) from the last user message until under budget.

    Walks `fields` in reverse so the lowest-priority field is dropped first.
    Returns the field name that finally brought the budget under, or None
    if even dropping the highest-priority field couldn't fit.
    """
    if len(messages) == 0 or len(fields) == 0:
        return None
    target_field = None
    for i in range(len(fields) - 1, -1, -1):
        candidate = fields[i]
        updated = _drop_field_in_message(messages[-1], candidate)
        messages[-1] = updated
        if estimate_context_tokens(messages) <= budget_tokens:
            target_field = candidate
            break
    return target_field


def _history_compact(messages, keep_recent_n):
    """Trim history to at most `keep_recent_n` non-system messages.

    System messages (the cache-stable prefix) are preserved at the head
    of the message list. Everything else is sliced to the most recent
    `keep_recent_n` entries. Returns the new list.

    Accepts both `"System"` (the canonical role produced by the
    orchestrator's `append_message` helper) and the lowercase `"system"` so
    that imported transcripts (e.g. CPython test fixtures, external
    gateways) are also classified correctly.
    """
    if keep_recent_n <= 0 or len(messages) <= keep_recent_n:
        return messages
    system_prefix = []
    body = []
    for msg in messages:
        if isinstance(msg, dict) and msg.get("role") in ("System", "system"):
            system_prefix.append(msg)
        else:
            body.append(msg)
    if len(body) <= keep_recent_n:
        return messages
    trimmed_body = body[-keep_recent_n:]
    return system_prefix + trimmed_body


def _apply_rule(messages, rule, budget_tokens):
    """Apply a single reduction rule. Returns the (possibly modified) message list."""
    rule_type = rule.get("type")
    field = rule.get("field")
    if rule_type == REDUCE_TYPE_TRUNCATE:
        max_chars = rule.get("max_chars", 0)
        if not field or len(messages) == 0:
            return messages
        updated = _truncate_field_in_message(messages[-1], field, max_chars)
        messages[-1] = updated
        return messages
    if rule_type == REDUCE_TYPE_SUMMARIZE:
        if not field or len(messages) == 0:
            return messages
        updated = _summarize_field_in_message(messages[-1], field)
        messages[-1] = updated
        return messages
    if rule_type == REDUCE_TYPE_DROP:
        if not field or len(messages) == 0:
            return messages
        updated = _drop_field_in_message(messages[-1], field)
        messages[-1] = updated
        return messages
    if rule_type == REDUCE_TYPE_PRIORITY:
        fields = rule.get("fields", [])
        _priority_drop_until_under_budget(messages, fields, budget_tokens)
        return messages
    if rule_type == REDUCE_TYPE_HISTORY_COMPACT:
        keep = rule.get("keep_recent_n", 0)
        return _history_compact(messages, keep)
    return messages


# NOTE: This function is named `_reduce_prompt` (underscore prefix) in this
# Monty-sandboxed production file. The CPython reference implementation in
# `segment_reduction.py` names it `reduce_prompt` (no prefix). The logic is
# identical; the underscore signals "internal to this module" in the Monty
# context where there is no real import system.
def _reduce_prompt(messages, rules, budget_tokens):
    """Apply reduction rules in order until the prompt fits the budget.

    Returns the message list (possibly reduced). Some rules — like
    `history_compact` — return a NEW list rather than mutating in
    place; we adopt that new list as the working list so subsequent
    rules operate on the reduced shape. Always returns the current
    working list even if all rules were applied and we are still
    over budget — caller is responsible for emitting `prompt_over_budget`
    and deciding whether to abort or proceed.
    """
    if estimate_context_tokens(messages) <= budget_tokens:
        return messages
    for rule in rules:
        if estimate_context_tokens(messages) <= budget_tokens:
            return messages
        messages = _apply_rule(messages, rule, budget_tokens)
    return messages


def compact_if_needed(state, config):
    """Compact thread context when the active message history grows too large.

    The orchestrator owns compaction policy. Rust only provides helpers for
    token estimation, explicit LLM calls, and replacing the active message
    scaffold after a summary has been produced.
    """
    if not config.get("enable_compaction", False):
        return False

    context_limit = config.get("model_context_limit", 128000)
    threshold_pct = config.get("compaction_threshold", 0.85)
    threshold = int(context_limit * threshold_pct)
    working_messages = state.get("working_messages")
    if not isinstance(working_messages, list) or not working_messages:
        return False

    current_tokens = estimate_context_tokens(working_messages)
    if current_tokens < threshold:
        return False

    snapshot = list(working_messages)

    history = state.get("history")
    if not isinstance(history, list):
        history = []
        state["history"] = history

    compaction_count = state.get("compaction_count", 0) + 1
    history.append({
        "kind": "compaction",
        "index": compaction_count,
        "tokens_before": current_tokens,
        "messages": snapshot,
    })

    summary_prompt = (
        "Summarize progress so far in a concise but complete way.\n"
        "Include:\n"
        "1. What has been accomplished\n"
        "2. Key intermediate results, facts, and variable values\n"
        "3. Tool results or findings worth preserving\n"
        "4. What still needs to be done\n"
        "5. Errors encountered and how they were handled\n\n"
        "Preserve all information needed to continue the task."
    )
    summary_messages = list(snapshot)
    summary_messages.append({"role": "User", "content": summary_prompt})
    summary_resp = __llm_complete__(summary_messages, None, {"force_text": True})

    summary_text = summary_resp.get("content", "")
    if not summary_text:
        summary_text = "[compaction produced no summary]"

    state["working_messages"] = []
    system_message = None
    for msg in snapshot:
        if msg.get("role") == "System":
            system_message = {"role": "System", "content": msg.get("content", "")}
            break
    if system_message is not None:
        state["working_messages"].append(system_message)
    append_message(state["working_messages"], "Assistant", summary_text)
    append_message(
        state["working_messages"],
        "User",
        "Your conversation has been compacted. The summary above captures prior progress. "
        "Older details remain available through state['history'] and project retrieval. Continue working on the task.",
    )
    state["compaction_count"] = compaction_count
    return True


# ── Skill selection and injection (self-modifiable) ────────

def _skill_token_cost(skill, activation):
    """Estimate token cost for a skill, mirroring Rust `skill_token_cost`.

    If the declared `max_context_tokens` is implausibly low (the actual
    prompt content is more than 2x the declared value), use the actual
    estimate instead. This prevents a skill from declaring
    `max_context_tokens: 1` to bypass the budget.
    """
    declared = max(activation.get("max_context_tokens", 2000), 1)
    content = skill.get("content", "")
    approx = int(len(content) * 0.25) if content else 0
    if approx > declared * 2:
        return max(approx, 1)
    return declared


def select_skills(skills, goal, max_candidates=3, max_tokens=6000):
    """Select skills to report as active for the current turn.

    Phase 1.5 — intent-system-driven path:
    Score-based selection (score_skill) and slash-command extraction
    (extract_explicit_skills) have been removed; the intent system now
    routes queries to the correct component.  This function returns the
    first `max_candidates` skills that fit within `max_tokens`, relying on
    Rust (handle_list_skills) to pre-filter and order candidates by
    relevance before they reach the orchestrator.
    """
    if not skills or not goal:
        return []

    # Build name -> skill lookup for chain-loading companion resolution.
    by_name = {}
    for sk in skills:
        meta = sk.get("metadata", {})
        name = meta.get("name")
        if name:
            by_name[str(name)] = sk

    selected = []
    selected_names = set()
    budget = max_tokens

    for parent in skills:
        if len(selected) >= max_candidates:
            break
        parent_meta = parent.get("metadata", {})
        parent_name = parent_meta.get("name")
        if parent_name is None or str(parent_name) in selected_names:
            continue
        parent_activation = parent_meta.get("activation", {})
        parent_cost = _skill_token_cost(parent, parent_activation)
        if parent_cost > budget:
            continue
        selected.append(parent)
        selected_names.add(str(parent_name))
        budget -= parent_cost

        # Chain-load companions (depth 1, non-transitive).
        requires = parent_meta.get("requires", {})
        companion_names = requires.get("skills", [])
        for companion_name in companion_names:
            cname = str(companion_name)
            if len(selected) >= max_candidates:
                break
            if cname in selected_names:
                continue
            companion = by_name.get(cname)
            if companion is None:
                # Listed but not loaded — ignore silently, persona
                # bundles often list optional companions.
                continue
            comp_meta = companion.get("metadata", {})
            comp_activation = comp_meta.get("activation", {})
            comp_cost = _skill_token_cost(companion, comp_activation)
            if comp_cost > budget:
                # Budget exhausted for companions. Parent is still
                # selected; the remaining companions are skipped.
                continue
            selected.append(companion)
            selected_names.add(cname)
            budget -= comp_cost

    return selected



def complete_result(state, outcome, response=None, error=None, extra=None):
    """Return a standard orchestrator result with persisted state."""
    result = {"outcome": outcome, "state": state}
    if response is not None:
        result["response"] = response
    if error is not None:
        result["error"] = error
    if isinstance(extra, dict):
        for key in extra:
            result[key] = extra[key]
    return result


# ── Action execution mode (class_code 16) ────────────────────
#
# When the intent system routes a query to an Action (class_code 16),
# default.py executes it deterministically — no __llm_complete__ call.
# spec §3.11, §7 Q13: 13 step types.
#
# SEC-07: allowed_tools checked here AND in EffectExecutor (defence-in-depth).
# SEC-08: spawn_subprocess dispatched via host runtime script lane only.
# SEC-09: call_action depth bounded; total step budget = 1000.
# PERF-18: content/step/tool hard limits enforced by Rust validator at save time.

ACTION_MAX_DEPTH = 5          # SEC-09 — call_action nesting limit
ACTION_STEP_BUDGET = 1000     # SEC-09 — total step budget across nesting


def _action_eval_condition(condition, scope_vars):
    """Evaluate a simple condition dict against the current variable scope.

    Supported condition forms:
      {"var_eq": {"name": "x", "value": "foo"}}
      {"var_truthy": "x"}
      {"const": true/false}
    Returns bool.
    """
    if not isinstance(condition, dict):
        return bool(condition)
    if "const" in condition:
        return bool(condition["const"])
    if "var_eq" in condition:
        name = condition["var_eq"].get("name", "")
        value = condition["var_eq"].get("value")
        return scope_vars.get(name) == value
    if "var_truthy" in condition:
        return bool(scope_vars.get(condition["var_truthy"]))
    return False


def _execute_action_steps(action, scope_vars, depth, step_counter):
    """Recursively execute the ordered step list of an Action.

    Returns a tuple (result, step_counter) where result is the return value
    of the first `return` step encountered, or None if execution ends without
    an explicit return.

    depth        — current call_action nesting depth (SEC-09).
    step_counter — mutable list [int] tracking total steps across all levels.
    """
    if depth > ACTION_MAX_DEPTH:
        return {"error": "call_action depth limit exceeded (SEC-09)"}, step_counter

    steps = action.get("steps", [])
    if not isinstance(steps, list):
        return {"error": "action steps must be a list"}, step_counter

    allowed_tools = action.get("allowed_tools", [])
    if not isinstance(allowed_tools, list):
        allowed_tools = []

    for step_def in steps:
        if not isinstance(step_def, dict):
            continue

        step_counter[0] += 1
        if step_counter[0] > ACTION_STEP_BUDGET:
            return {"error": "action step budget exceeded (SEC-09)"}, step_counter

        kind = step_def.get("type", "")

        if kind == "tool_call":
            # SEC-07: check allowed_tools at default.py level.
            tool_name = step_def.get("tool", "")
            if tool_name not in allowed_tools:
                return {
                    "error": "tool_call blocked: '{}' not in allowed_tools (SEC-07)".format(tool_name)
                }, step_counter
            params = step_def.get("params", {})
            result = __execute_action__(tool_name, params)
            scope_vars["_last_result"] = result

        elif kind == "conditional":
            cond = step_def.get("condition", {"const": False})
            if _action_eval_condition(cond, scope_vars):
                branch = step_def.get("then_step")
            else:
                branch = step_def.get("else_step")
            if branch:
                sub_result, step_counter = _execute_action_steps(
                    {"steps": [branch], "allowed_tools": allowed_tools},
                    scope_vars, depth, step_counter
                )
                if sub_result is not None and "return" in str(sub_result):
                    return sub_result, step_counter

        elif kind == "set_var":
            var_name = step_def.get("name", "")
            var_value = step_def.get("value")
            if var_name:
                scope_vars[var_name] = var_value

        elif kind == "loop":
            loop_steps = step_def.get("steps", [])
            exit_cond = step_def.get("exit_condition", {"const": True})
            max_iters = step_def.get("max_iterations", 100)
            for _ in range(max_iters):
                if _action_eval_condition(exit_cond, scope_vars):
                    break
                sub_result, step_counter = _execute_action_steps(
                    {"steps": loop_steps, "allowed_tools": allowed_tools},
                    scope_vars, depth, step_counter
                )
                if sub_result is not None:
                    return sub_result, step_counter

        elif kind == "return":
            return_val = step_def.get("value")
            # Substitute scope variable if value is a variable reference.
            if isinstance(return_val, str) and return_val.startswith("$"):
                return_val = scope_vars.get(return_val[1:], return_val)
            return {"result": return_val}, step_counter

        elif kind == "evaluate":
            # Python-tunable evaluation step.
            expr = step_def.get("expression", "")
            store_as = step_def.get("store_as", "_eval_result")
            # eval is intentional here: Actions are validated by Rust before save.
            # Only Actions that pass Rust validation + LLM code audit reach this path.
            try:
                local_ctx = dict(scope_vars)
                local_ctx["__last__"] = scope_vars.get("_last_result")
                eval_result = eval(expr, {"__builtins__": {}}, local_ctx)  # noqa: S307
                scope_vars[store_as] = eval_result
            except Exception as exc:
                scope_vars[store_as] = None
                scope_vars["_eval_error"] = str(exc)

        elif kind == "call_skill":
            # Invoke a skill as a tool call (skills must be in allowed_tools).
            skill_name = step_def.get("skill", "")
            params = step_def.get("params", {})
            if skill_name not in allowed_tools:
                return {
                    "error": "call_skill blocked: '{}' not in allowed_tools (SEC-07)".format(skill_name)
                }, step_counter
            result = __execute_action__(skill_name, params)
            scope_vars["_last_result"] = result

        elif kind == "try_catch":
            try_steps = step_def.get("try", [])
            catch_steps = step_def.get("catch", [])
            sub_result, step_counter = _execute_action_steps(
                {"steps": try_steps, "allowed_tools": allowed_tools},
                scope_vars, depth, step_counter
            )
            if sub_result is not None and "error" in sub_result:
                scope_vars["_caught_error"] = sub_result["error"]
                sub_result, step_counter = _execute_action_steps(
                    {"steps": catch_steps, "allowed_tools": allowed_tools},
                    scope_vars, depth, step_counter
                )
            if sub_result is not None and "result" in sub_result:
                return sub_result, step_counter

        elif kind == "parallel":
            # Concurrent tool calls — dispatch all, collect results.
            parallel_calls = step_def.get("calls", [])
            calls_to_run = []
            for call in parallel_calls:
                tool_name = call.get("tool", "")
                if tool_name not in allowed_tools:
                    return {
                        "error": "parallel tool_call blocked: '{}' not in allowed_tools (SEC-07)".format(tool_name)
                    }, step_counter
                calls_to_run.append({"name": tool_name, "params": call.get("params", {})})
            results = __execute_actions_parallel__(calls_to_run)
            scope_vars["_parallel_results"] = results

        elif kind == "call_action":
            # Invoke a nested Action (SEC-09 depth bounded above).
            nested_name = step_def.get("action", "")
            nested_params = step_def.get("params", {})
            # Retrieve the nested action from the prior-knowledge docs.
            nested_docs = __retrieve_docs__(nested_name, 1)
            if not nested_docs:
                return {
                    "error": "call_action: Action '{}' not found".format(nested_name)
                }, step_counter
            nested_action = nested_docs[0]
            nested_scope = dict(nested_params)
            sub_result, step_counter = _execute_action_steps(
                nested_action, nested_scope, depth + 1, step_counter
            )
            if sub_result is not None:
                scope_vars["_last_result"] = sub_result
                if "result" in sub_result:
                    scope_vars["_last_result"] = sub_result["result"]

        elif kind == "spawn_subprocess":
            # SEC-08: dispatch ONLY through host runtime script lane.
            # Raw subprocess.Popen is NOT used here — the host runtime
            # enforces capability lease + approval gate + sandbox boundary.
            if "spawn_subprocess" not in allowed_tools:
                return {
                    "error": "spawn_subprocess blocked: 'spawn_subprocess' not in allowed_tools (SEC-08)"
                }, step_counter
            params = {
                "command": step_def.get("command", ""),
                "args": step_def.get("args", []),
                "cwd": step_def.get("cwd"),
                "timeout_secs": step_def.get("timeout_secs", action.get("timeout_secs", 60)),
            }
            result = __execute_action__("spawn_subprocess", params)
            scope_vars["_last_result"] = result

        elif kind == "wait":
            # Pause for a fixed duration or until a polling condition is met.
            duration_secs = step_def.get("duration_secs", 0)
            if duration_secs > 0:
                result = __execute_action__("wait", {"duration_secs": duration_secs})
                scope_vars["_last_result"] = result
            else:
                poll_cond = step_def.get("condition", {"const": True})
                max_poll = step_def.get("max_polls", 30)
                for _ in range(max_poll):
                    if _action_eval_condition(poll_cond, scope_vars):
                        break
                    __execute_action__("wait", {"duration_secs": 1})

        elif kind == "emit_event":
            # Emit a structured event to the event bus (brassclaw_events).
            event_kind = step_def.get("kind", "action_event")
            event_data = step_def.get("data", {})
            __emit_event__(event_kind, **event_data)

        # Unknown step kinds are silently skipped (forward-compatibility).

    return None, step_counter


def execute_action_procedure(action_doc, goal, state):
    """Execute an Action document (class_code 16) deterministically.

    Called from run_loop when the intent system returns class_code 16.
    Returns a complete_result dict — run_loop returns this directly without
    calling __llm_complete__.

    Spec §3.11 dispatch flow (steps 5–8):
      5. prior_knowledge_content is given to default.py.
      6. default.py recognises class_code 16 and stops further prompt creation.
      7. default.py performs the Action directly.
      8. The Action's return value becomes the turn result.
    """
    scope_vars = {}
    # Make goal available inside the Action's evaluate/conditional steps.
    scope_vars["goal"] = goal

    step_counter = [0]
    result, _counter = _execute_action_steps(action_doc, scope_vars, 0, step_counter)

    if result is None:
        # Action completed without an explicit `return` step.
        return complete_result(state, "completed", "Action completed.")
    if "error" in result:
        return complete_result(state, "error", None, error=result["error"])
    return complete_result(state, "completed", result.get("result", ""))


# ── Main execution loop ─────────────────────────────────────


def run_loop(context, goal, actions, state, config):
    """Main execution loop. Returns an outcome dict."""
    max_iterations = config.get("max_iterations", 30)
    # None means "no limit" — callers can disable the guard explicitly.
    max_consecutive_errors = config.get("max_consecutive_errors", 5)
    # None means "no limit" (matches Option::None semantics from Rust caller).
    # Use a sentinel larger than any realistic counter so comparisons stay well-typed.
    if max_consecutive_errors is None:
        max_consecutive_errors = 10**9
    obligation_enabled = config.get("require_action_attempt", False)
    max_obligation_nudges = config.get("max_action_requirement_nudges", 2)

    # consecutive_nudges is kept for checkpoint compatibility (resume across turns).
    # Tool-intent-nudge logic has been removed (replaced by intent system, Phase 1.5).
    consecutive_nudges = 0
    consecutive_errors = 0
    consecutive_action_errors = 0
    step_count = config.get("step_count", 0)
    if not isinstance(state, dict):
        state = {}
    state.setdefault("history", [])
    state.setdefault("compaction_count", 0)

    working_messages = ensure_working_messages(state, context)

    for step in range(step_count, max_iterations):
        # 1. Check signals
        signal = __check_signals__()
        if signal == "stop":
            __transition_to__("completed", "stopped by signal")
            return complete_result(state, "stopped")
        if signal and isinstance(signal, dict) and "inject" in signal:
            injected_text = signal["inject"]
            append_message(working_messages, "User", injected_text)

        # 2. Check budget
        # Token budget: SOFT TELEMETRY ONLY. The post-assembly reduction
        # pipeline (_reduce_prompt) progressively shrinks the prompt when
        # it is over budget, so we never abort on token exhaustion here.
        # Time + cost budgets remain hard-stops because they reflect real
        # resource limits (session deadline, accumulated spend) that no
        # reduction can alleviate.
        budget = __check_budget__()
        if budget.get("tokens_remaining", 1) <= 0:
            __log_budget_warning__("tokens", int(budget.get("tokens_remaining", 0)), "token budget low")
        if budget.get("time_remaining_ms", 1) <= 0:
            __transition_to__("completed", "time budget exhausted")
            return complete_result(state, "completed", "Time budget exhausted.")
        if budget.get("usd_remaining") is not None and budget["usd_remaining"] <= 0:
            __transition_to__("completed", "cost budget exhausted")
            return complete_result(state, "completed", "Cost budget exhausted.")

        # 3. Register active skills on first step.
        # Prior-knowledge docs are injected at position N-1 by the Rust
        # layer (build_step_context / InstructionBundleBuilder priority 6+).
        # Skills are assembled into the stable system-prompt prefix by Rust
        # (InstructionBundleBuilder priority 2). The orchestrator registers
        # which skills are active for tracking and event emission only.
        if step == 0:
            docs = __retrieve_docs__(goal, 5)

            # ── Action short-circuit (class_code 16, §3.11) ──────────────
            # If the retrieved docs include an Action, execute it directly
            # without calling __llm_complete__ and return immediately.
            # Actions bypass the LLM turn entirely — no prompt creation.
            if docs:
                for doc in docs:
                    metadata = doc.get("metadata", {}) if isinstance(doc, dict) else {}
                    if metadata.get("class_code") == 16:
                        __emit_event__("action_started", action_name=metadata.get("name", ""))
                        __transition_to__("running", "action execution")
                        action_result = execute_action_procedure(doc, goal, state)
                        __transition_to__("completed", "action completed")
                        return action_result
            # ─────────────────────────────────────────────────────────────

            # Register active skills for tracking / event emission.
            all_skills = __list_skills__()
            active_skills = select_skills(all_skills, goal, max_candidates=3, max_tokens=6000)
            if active_skills:
                __set_active_skills__([
                    {
                        "doc_id": s.get("doc_id", ""),
                        "name": s.get("metadata", {}).get("name", "?"),
                        "version": s.get("metadata", {}).get("version", 1),
                        "snippet_names": [
                            sn.get("name", "")
                            for sn in s.get("metadata", {}).get("code_snippets", [])
                            if sn.get("name")
                        ],
                        "force_activated": False,
                    }
                    for s in active_skills
                ])
                # Emit skill activation event for CLI/gateway display.
                skill_names = ",".join(s.get("metadata", {}).get("name", "?") for s in active_skills)
                __emit_event__("skill_activated", skill_names=skill_names)
                # Store active skill IDs in state for tracking.
                state["active_skill_ids"] = [s.get("doc_id", "") for s in active_skills]
                state["skill_snippet_names"] = []
                for s in active_skills:
                    for sn in s.get("metadata", {}).get("code_snippets", []):
                        state["skill_snippet_names"].append(sn.get("name", ""))

        # 3.4 Post-assembly reduction pipeline.
        # If the assembled prompt is over budget, fetch the per-user/user
        # reduction rules from the host store and progressively shrink
        # the message list. Only emits a telemetry event when even
        # reduction can't fit — never aborts the orchestrator.
        prompt_budget = 0
        if isinstance(config, dict):
            raw = config.get("prompt_budget_tokens")
            if isinstance(raw, int):
                prompt_budget = raw
            elif isinstance(raw, float):
                prompt_budget = int(raw)
        if prompt_budget > 0 and estimate_context_tokens(working_messages) > prompt_budget:
            rules = __get_reduction_rules__()
            rules_list = []
            if isinstance(rules, list):
                rules_list = rules
            # IMPORTANT: capture the return value. Rules that rebuild the
            # message list (e.g. `history_compact`) return a NEW list —
            # discarding the return loses the trimmed prefix.
            working_messages = _reduce_prompt(
                working_messages, rules_list, prompt_budget
            )
            # Mirror into state so `ensure_working_messages` picks up the
            # reduced list when it runs after the enforcement step.
            state["working_messages"] = working_messages
            # Compute once and reuse: the post-reduction check and the event
            # kwarg both read the same (now-reduced) message list.
            post_tokens = estimate_context_tokens(working_messages)
            if post_tokens > prompt_budget:
                __emit_event__(
                    "prompt_over_budget",
                    estimated_tokens=post_tokens,
                    budget_tokens=prompt_budget,
                )

        # 3.5 Compact context before the next model call when needed.
        compact_if_needed(state, config)
        working_messages = ensure_working_messages(state, context)

        # 4. Call LLM
        __emit_event__("step_started", step=step)
        response = __llm_complete__(working_messages, actions, None)
        __emit_event__("step_completed", step=step,
                       input_tokens=response.get("usage", {}).get("input_tokens", 0),
                       output_tokens=response.get("usage", {}).get("output_tokens", 0))

        # 5. Handle response based on type
        resp_type = response.get("type", "text")

        if resp_type == "text":
            text = response.get("content", "")
            append_message(working_messages, "Assistant", text)

            # Check for FINAL()
            final_answer = extract_final(text)
            if final_answer is not None:
                __transition_to__("completed", "FINAL() in text")
                return complete_result(state, "completed", final_answer)

            # Check execution obligation.
            available_actions = __get_actions__()
            if (obligation_enabled
                    and len(available_actions) > 0
                    and not state.get("_obligation_resolved", False)
                    and state.get("_obligation_nudge_count", 0) < max_obligation_nudges):
                state["_obligation_nudge_count"] = state.get("_obligation_nudge_count", 0) + 1
                append_message(
                    working_messages,
                    "User",
                    "You were asked to perform an action, but you responded with text only.\n"
                    "Do NOT describe or explain — call the appropriate tool now.\n"
                    "Use the tool_calls mechanism to invoke the tool.",
                )
                continue

            # Plain text response - done
            __transition_to__("completed", "text response")
            return complete_result(state, "completed", text)

        elif resp_type == "code":
            state["_obligation_resolved"] = True  # code attempt satisfies obligation
            code = response.get("code", "")
            append_message(working_messages, "Assistant", "```repl\n" + code + "\n```")

            # Execute code in nested Monty VM
            result = __execute_code_step__(code, state)

            # Update persisted state with results
            if result.get("return_value") is not None:
                state["step_" + str(step) + "_return"] = result["return_value"]
                state["last_return"] = result["return_value"]
            for r in result.get("action_results", []):
                state[r.get("action_name", "unknown")] = r.get("output")

            # Format output for next LLM context
            output = format_output(result)
            append_message(working_messages, "User", output)

            # Check for FINAL() in code output
            if result.get("final_answer") is not None:
                __transition_to__("completed", "FINAL() in code")
                return complete_result(state, "completed", result["final_answer"])

            # Check for unified gate pause (new path)
            gate = result.get("pending_gate")
            if gate is None:
                gate = result.get("need_approval")
            if gate is not None and isinstance(gate, dict) and gate.get("gate_paused"):
                __save_checkpoint__(state, {
                    "nudge_count": consecutive_nudges,
                    "consecutive_errors": consecutive_errors,
                    "consecutive_action_errors": consecutive_action_errors,
                    "compaction_count": state.get("compaction_count", 0),
                    "obligation_nudge_count": state.get("_obligation_nudge_count", 0),
                })
                __transition_to__("waiting", "gate paused: " + gate.get("gate_name", "unknown"))
                return {
                    "outcome": "gate_paused",
                    "state": state,
                    "gate_name": gate.get("gate_name", ""),
                    "action_name": gate.get("action_name", ""),
                    "call_id": gate.get("call_id", ""),
                    "parameters": gate.get("parameters", {}),
                    "resume_kind": gate.get("resume_kind", {}),
                }

            # Check for approval or authentication needed (legacy path)
            if result.get("need_approval") is not None:
                approval = result["need_approval"]
                __save_checkpoint__(state, {
                    "nudge_count": consecutive_nudges,
                    "consecutive_errors": consecutive_errors,
                    "consecutive_action_errors": consecutive_action_errors,
                    "compaction_count": state.get("compaction_count", 0),
                    "obligation_nudge_count": state.get("_obligation_nudge_count", 0),
                })
                if approval.get("need_authentication"):
                    __transition_to__("waiting", "authentication needed")
                    return {
                        "outcome": "need_authentication",
                        "state": state,
                        "credential_name": approval.get("credential_name", ""),
                        "action_name": approval.get("action_name", ""),
                        "call_id": approval.get("call_id", ""),
                        "parameters": approval.get("parameters", {}),
                    }
                __transition_to__("waiting", "approval needed")
                return {
                    "outcome": "need_approval",
                    "state": state,
                    "action_name": approval.get("action_name", ""),
                    "call_id": approval.get("call_id", ""),
                    "parameters": approval.get("parameters", {}),
                }

            # Track consecutive errors
            if result.get("had_error"):
                consecutive_errors += 1
                if max_consecutive_errors is not None and consecutive_errors >= max_consecutive_errors:
                    __transition_to__("failed", "too many consecutive errors")
                    return complete_result(
                        state,
                        "failed",
                        error=str(max_consecutive_errors) + " consecutive code errors",
                    )
            else:
                consecutive_errors = 0

            __save_checkpoint__(state, {
                "nudge_count": consecutive_nudges,
                "consecutive_errors": consecutive_errors,
                "consecutive_action_errors": consecutive_action_errors,
                "compaction_count": state.get("compaction_count", 0),
                "obligation_nudge_count": state.get("_obligation_nudge_count", 0),
            })

        elif resp_type == "actions":
            state["_obligation_resolved"] = True  # action attempt satisfies obligation
            # Tier 0: structured tool calls.
            # NOTE: consecutive_nudges is NOT reset here (V1 semantics).
            # Only non-intent text responses reset the counter.
            calls = response.get("calls", [])

            # Handle FINAL emitted as a structured tool call. FINAL is a
            # CodeAct sentinel for completion — when the LLM tries to call
            # it via tool_calls instead of inside a code block, the engine's
            # action executor has no lease for it and the call fails. If FINAL
            # is co-emitted with other calls, execute the non-FINAL calls first
            # so persistence side effects are not silently dropped.
            final_call = None
            duplicate_finals_dropped = 0
            executable_calls = []
            for c in calls:
                if c.get("name", "") == "FINAL":
                    # First FINAL wins; any extras are dropped (not appended
                    # to executable_calls) so they don't try to run as a
                    # normal action and fail with a lease error.
                    if final_call is None:
                        final_call = c
                    else:
                        duplicate_finals_dropped += 1
                    continue
                executable_calls.append(c)

            if duplicate_finals_dropped > 0:
                # Surface the drop so traces show why fewer FINALs were
                # executed than the LLM emitted.
                __emit_event__(
                    "duplicate_final_dropped",
                    count=duplicate_finals_dropped,
                )

            # Append the assistant message with only the executable calls.
            # FINAL is filtered out of `action_calls` so the message history
            # does not record a FINAL action with no matching ActionResult,
            # which would confuse context replay on resume.
            append_message(
                working_messages,
                "Assistant",
                response.get("content", "") or "",
                action_calls=executable_calls,
            )

            # Execute all tool calls in parallel via the batch host function.
            # Rust handles preflight (lease/policy), parallel execution via
            # JoinSet, and event emission in call order.
            results = __execute_actions_parallel__(executable_calls)
            # Every tool call in the assistant message MUST have a matching
            # ActionResult, otherwise the LLM API rejects the sequence with
            # "No tool output found for function call <id>". Iterate over
            # executable_calls (not results) so we cover calls that the Rust
            # batch handler skipped (e.g. RequireApproval early return).
            batch_error_count = 0
            batch_success_count = 0
            for idx in range(len(executable_calls)):
                call = executable_calls[idx]
                call_id = call.get("call_id", "")
                r = results[idx] if idx < len(results) else None
                if r is not None:
                    action_name = r.get("action_name", call.get("name", ""))
                    output = r.get("output")
                    output_str = str(output) if output is not None else "[no output]"
                    if r.get("is_error"):
                        output_str = "[ACTION FAILED] " + action_name + ": " + output_str
                        batch_error_count += 1
                    else:
                        batch_success_count += 1
                else:
                    action_name = call.get("name", "unknown")
                    output_str = "[execution skipped]"
                    batch_error_count += 1
                append_message(
                    working_messages,
                    "ActionResult",
                    output_str,
                    action_name=action_name,
                    action_call_id=call_id,
                )

            # TODO(#2325): track consecutive action errors here, mirroring the
            # code error tracking above (lines 623-634). Needs a unified
            # progress-tracking design across both execution paths.

            # Check results for auth/approval interrupts
            for r_idx, r in enumerate(results):
                if r is None:
                    continue

                if r.get("gate_paused"):
                    # Unified gate pause (replaces separate need_approval/need_authentication)
                    __save_checkpoint__(state, {
                        "nudge_count": consecutive_nudges,
                        "consecutive_errors": consecutive_errors,
                        "consecutive_action_errors": consecutive_action_errors,
                        "compaction_count": state.get("compaction_count", 0),
                        "obligation_nudge_count": state.get("_obligation_nudge_count", 0),
                    })
                    gate = r
                    # Get action info from the original call or the result
                    orig_call = executable_calls[r_idx] if r_idx < len(executable_calls) else {}
                    __transition_to__("waiting", "gate paused: " + gate.get("gate_name", "unknown"))
                    return {
                        "outcome": "gate_paused",
                        "state": state,
                        "gate_name": gate.get("gate_name", ""),
                        "action_name": gate.get("action_name", orig_call.get("name", "")),
                        "call_id": orig_call.get("call_id", ""),
                        "parameters": orig_call.get("params", {}),
                        "resume_kind": gate.get("resume_kind", {}),
                    }

                if r.get("need_authentication"):
                    __save_checkpoint__(state, {
                        "nudge_count": consecutive_nudges,
                        "consecutive_errors": consecutive_errors,
                        "consecutive_action_errors": consecutive_action_errors,
                        "compaction_count": state.get("compaction_count", 0),
                        "obligation_nudge_count": state.get("_obligation_nudge_count", 0),
                    })
                    __transition_to__("waiting", "authentication needed")
                    return {
                        "outcome": "need_authentication",
                        "state": state,
                        "credential_name": r.get("credential_name", ""),
                        "action_name": r.get("action_name", ""),
                        "call_id": r.get("call_id", ""),
                        "parameters": r.get("parameters", {}),
                    }

                if r.get("need_approval"):
                    __save_checkpoint__(state, {
                        "nudge_count": consecutive_nudges,
                        "consecutive_errors": consecutive_errors,
                        "consecutive_action_errors": consecutive_action_errors,
                        "compaction_count": state.get("compaction_count", 0),
                        "obligation_nudge_count": state.get("_obligation_nudge_count", 0),
                    })
                    __transition_to__("waiting", "approval needed")
                    return {
                        "outcome": "need_approval",
                        "state": state,
                        "action_name": r.get("action_name", ""),
                        "call_id": r.get("call_id", ""),
                        "parameters": r.get("parameters", {}),
                    }

            if final_call is not None:
                raw_params = final_call.get("params", {})
                # Some LLMs pass FINAL with the answer as a positional string
                # argument instead of a named param dict. Handle that case so
                # the answer is not silently dropped.
                if isinstance(raw_params, str):
                    answer = raw_params
                else:
                    params = raw_params or {}
                    answer = (
                        params.get("answer")
                        or params.get("result")
                        or params.get("value")
                        or params.get("content")
                        or params.get("text")
                    )
                    if not answer:
                        # Fall back to the assistant's content text. This may
                        # contain the model's full explanation rather than the
                        # intended terse answer — truncate aggressively so we
                        # don't ship thousands of tokens of reasoning as the
                        # final answer, and emit a trace event so the
                        # ambiguity is visible.
                        fallback_content = response.get("content", "") or ""
                        FINAL_FALLBACK_MAX_CHARS = 500
                        truncated = False
                        if len(fallback_content) > FINAL_FALLBACK_MAX_CHARS:
                            fallback_content = (
                                fallback_content[:FINAL_FALLBACK_MAX_CHARS]
                                + "… [truncated by orchestrator: FINAL was emitted with no recognizable answer param]"
                            )
                            truncated = True
                        answer = fallback_content
                        __emit_event__(
                            "final_fallback",
                            reason="no recognizable answer param on FINAL",
                            truncated=truncated,
                            original_length=len(response.get("content", "") or ""),
                        )
                __transition_to__("completed", "FINAL via tool_calls")
                return complete_result(state, "completed", str(answer))

            # Track consecutive action errors (separate from code errors).
            # Partial batch failures: increment only if ALL actions failed,
            # reset if ANY succeeded.
            if batch_success_count > 0:
                consecutive_action_errors = 0
            elif batch_error_count > 0:
                consecutive_action_errors += 1

            if max_consecutive_errors is not None and consecutive_action_errors > 0 and consecutive_action_errors >= max_consecutive_errors + 2:
                __transition_to__("failed", "too many consecutive action errors")
                return complete_result(
                    state,
                    "failed",
                    error=str(consecutive_action_errors) + " consecutive action errors — all recent tool calls failed",
                )
            elif max_consecutive_errors is not None and consecutive_action_errors > 0 and consecutive_action_errors >= max_consecutive_errors:
                append_message(
                    working_messages,
                    "User",
                    "[SYSTEM] Your last " + str(consecutive_action_errors) +
                    " action calls have all failed. You appear to be stuck in a loop. "
                    "Try a completely different approach: use different tools, different "
                    "parameters, or break the problem down differently. If you cannot "
                    "make progress, call FINAL() with an honest explanation of what failed.",
                )

            __save_checkpoint__(state, {
                "nudge_count": consecutive_nudges,
                "consecutive_errors": consecutive_errors,
                "consecutive_action_errors": consecutive_action_errors,
                "compaction_count": state.get("compaction_count", 0),
                "obligation_nudge_count": state.get("_obligation_nudge_count", 0),
            })

    # Max iterations reached
    __transition_to__("completed", "max iterations reached")
    return complete_result(state, "max_iterations")


# Entry point: call run_loop with injected context variables
result = run_loop(context, goal, actions, state, config)
FINAL(result)
