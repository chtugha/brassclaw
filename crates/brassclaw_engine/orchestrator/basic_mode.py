# v3 basic-mode orchestrator (orchestrator:main, class 10, protected).
#
# The compiled-in Phase-1 harness. Monty (the Python Orchestrator) is the sole
# execution authority: it resolves the turn's intent, composes the matched
# recipe into a concrete program, runs each step's executable code, and posts
# the reply. Rust is the host (muscle) — it serves `host.*` calls and runs
# `host.run_program` code; it does NOT sequence the turn.
#
# Flow:
#   1. Extract the last User message from `context` as `user_input`.
#   2. `host.resolve_intent(user_input=)` → dispatch on `status`.
#      - "match"        → `host.compose_orchestrator(component_id, step_link,
#                         user_input)` → iterate `program.steplist` running each
#                         step's `executable_code` via `host.run_program`. The
#                         skills array (`program.skills`) is carried for
#                         consultation; per-step code is concrete (variable
#                         substitution is server-side in compose_orchestrator).
#      - "disambiguation" / "no_match" / "error"
#                       → resolve the `host-non-match-llm-answer` recipe
#                         (compose + run); ultimate fallback → direct
#                         `host.kohai_complete` (Monty assembles the prompt and
#                         hands it to Kohai, which swaps the prefix placeholder
#                         for the provider prefix and calls the provider LLM).
#   3. `host.post_reply(text=answer)`.
#   4. Resolve the `host-save-history` recipe (compose + run) — best-effort.
#   5. `FINAL({outcome, response, state})`.
#
# `host.check_signals()` is consulted between phases; "stop" short-circuits to
# `{outcome: "stopped"}`.
#
# Monty 0.0.16 subset: dicts/lists/strs, `for`/`if`/`try`, `.get()`/`.append()`,
# `isinstance`, `len`, `str`, `range`, `is None`, host.* calls, FINAL. NO
# f-strings, NO str.format, NO exec/eval/compile, NO `re`, NO imports.


def _last_user_input(context):
    """Return the content of the last User message in context, or ''."""
    user_input = ""
    for msg in context:
        if msg.get("role") == "User":
            user_input = msg.get("content", "")
    return user_input


def _chat_history(context):
    """Build the chat_history list for a kohai_complete prompt from context."""
    history = []
    for msg in context:
        role = msg.get("role", "")
        content = msg.get("content", "")
        history.append({"role": role, "content": content})
    return history


def _stringify(value):
    """Coerce a run_program return_value to a reply string."""
    if value is None:
        return ""
    if isinstance(value, str):
        return value
    return str(value)


def _run_steplist(program):
    """Iterate program.steplist, running each step's executable_code via
    host.run_program. Returns {ok, answer}: ok=False on the first failed step
    (answer = last good step's text); ok=True with the last step's text."""
    steplist = program.get("steplist", [])
    # program.skills is carried for consultation (exact tool usage). Per-step
    # executable_code is already concrete (compose_orchestrator baked in the
    # {{vars}} substitution + tool calls), so v0 runs it as-is.
    skills = program.get("skills", [])
    last_answer = ""
    for step in steplist:
        code = step.get("executable_code", "")
        result = host.run_program(code)
        if not result.get("ok"):
            return {"ok": False, "answer": last_answer}
        rv = result.get("return_value")
        if rv is None:
            rv = result.get("stdout", "")
        step_text = _stringify(rv)
        if step_text != "":
            last_answer = step_text
    return {"ok": True, "answer": last_answer}


def _compose_and_run(component_id, step_link, user_input):
    """compose_orchestrator + run_steplist. Returns the answer string, or None
    when composition or any step fails (caller falls back)."""
    composed = host.compose_orchestrator(component_id, step_link, user_input)
    if not composed.get("ok"):
        return None
    program = composed.get("program")
    if program is None:
        return None
    ran = _run_steplist(program)
    if ran.get("ok"):
        return ran.get("answer", "")
    return None


def _non_match_answer(context, user_input):
    """Non-Matching-Mode (Tier 2). Try the host-non-match-llm-answer recipe
    first; on any failure, fall back to a direct host.kohai_complete call
    (Monty assembles the prompt; Kohai swaps the prefix placeholder + calls the
    provider LLM)."""
    recipe = host.resolve_component_by_name("host-non-match-llm-answer", 21)
    if recipe is not None:
        recipe_id = recipe.get("id", "")
        if recipe_id != "":
            answer = _compose_and_run(recipe_id, "default", user_input)
            if answer is not None and answer != "":
                return answer
    # Ultimate fallback: direct Kohai-mediated LLM call.
    prompt = {
        "chat_history": _chat_history(context),
        "user_query": user_input,
        "prefix_placeholder": "{{prefix}}",
    }
    kohai = host.kohai_complete(prompt=prompt)
    if kohai.get("ok"):
        return kohai.get("answer", "")
    return ""


def _save_history(user_input, answer):
    """Resolve + compose + run the host-save-history recipe (best-effort:
    silently skip when the recipe is absent or composition fails)."""
    recipe = host.resolve_component_by_name("host-save-history", 21)
    if recipe is None:
        return
    recipe_id = recipe.get("id", "")
    if recipe_id == "":
        return
    composed = host.compose_orchestrator(recipe_id, "default", user_input)
    if not composed.get("ok"):
        return
    program = composed.get("program")
    if program is None:
        return
    _run_steplist(program)


def main(context, goal, actions, state, config):
    """Basic-mode turn entry point. Returns the orchestrator outcome dict."""
    if not isinstance(state, dict):
        state = {}
    state.setdefault("history", [])

    # 1. Turn-start signal check.
    signal = host.check_signals()
    if signal == "stop":
        return {"outcome": "stopped", "state": state}

    # 2. Extract the user's request (last User message).
    user_input = _last_user_input(context)
    if user_input == "":
        return {"outcome": "completed", "response": "", "state": state}

    # 3. Resolve intent + dispatch.
    intent = host.resolve_intent(user_input=user_input)
    status = intent.get("status", "no_match")

    answer = ""
    if status == "match":
        component_id = intent.get("component_id", "")
        step_link = intent.get("step_link", "")
        match_answer = _compose_and_run(component_id, step_link, user_input)
        if match_answer is not None:
            answer = match_answer
        else:
            answer = _non_match_answer(context, user_input)
    else:
        # disambiguation / no_match / error → Non-Matching-Mode.
        answer = _non_match_answer(context, user_input)

    # 4. Pre-reply signal check.
    signal2 = host.check_signals()
    if signal2 == "stop":
        return {"outcome": "stopped", "state": state}

    # 5. Post the reply + save history.
    if answer != "":
        host.post_reply(text=answer)
    _save_history(user_input, answer)

    return {"outcome": "completed", "response": answer, "state": state}


# Entry point: call main with the injected bootstrap variables.
result = main(context, goal, actions, state, config)
FINAL(result)
