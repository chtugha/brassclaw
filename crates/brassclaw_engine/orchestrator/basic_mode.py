# v3 basic-mode orchestrator (orchestrator:main, class 10, protected).
#
# The compiled-in Phase-1 harness. Monty (the Python Orchestrator) is the sole
# execution authority: it resolves the turn's intent, composes the matched
# recipe into a concrete program, runs each step's executable code, and posts
# the reply. Rust is the host (muscle) — it serves `host.*` calls and runs
# `host.run_program` code; it does NOT sequence the turn.
#
# Flow (resumable long-running loop — one iteration per turn; the VM persists
# across turns, parking at host.await_next_turn() instead of returning):
#   - Seed the in-VM `history` from the bootstrap `context` once (turn 1),
#     dropping the last User message (that message is the current turn's input,
#     delivered via host.await_next_turn() so the per-turn append stays uniform).
#   loop:
#   1. `host.check_signals()` → "stop" short-circuits to
#      `FINAL({outcome: "stopped", state})`.
#   2. `user_input = host.await_next_turn()` — the park point. The driver
#      resumes the parked VM with the next turn's input (turn 1's input arrives
#      via the prime-then-resume: a first `drive_to_yield(None)` parks here,
#      then `drive_to_yield(Some(turn1_input))` resumes). Empty →
#      `FINAL({outcome: "completed", response: ""})`.
#   3. Append the User message to `history`.
#   4. `host.resolve_intent(user_input=)` → dispatch on `status`.
#      - "match"        → `host.compose_orchestrator(component_id, step_link,
#                         user_input)` → iterate `program.steplist` running each
#                         step's `executable_code` via `host.run_program`. The
#                         skills array (`program.skills`) is carried for
#                         consultation (exact tool-usage narrative); per-step
#                         code is concrete (variable substitution is server-side
#                         in compose_orchestrator).
#      - "disambiguation" / "no_match" / "error"
#                       → resolve the `host-non-match-llm-answer` recipe
#                         (compose + run); ultimate fallback → direct
#                         `host.kohai_complete` (Monty assembles the prompt from
#                         `history` and hands it to Kohai, which swaps the prefix
#                         placeholder for the provider prefix and calls the
#                         provider LLM).
#   5. `host.post_reply(text=answer)`; resolve + run the `host-save-history`
#      recipe (best-effort); append the Assistant answer to `history`.
#   6. Loop back to step 1 (park at the next `host.await_next_turn()`).
#
# `FINAL(...)` is only reached on stop/empty-input termination; the happy path
# loops forever, parked between turns. The non-persistent `execute_orchestrator`
# caller maps an `AwaitNextTurn` park to an error (the persistent C.6 driver
# parks the session in a conversation-keyed registry instead).
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


def _seed_history(context):
    """Build the in-VM chat history from the bootstrap context, dropping the
    last User message. That message is the current turn's input, delivered via
    host.await_next_turn() — excluding it here lets every turn append the User
    message uniformly (turn 1 re-appends the dropped message; turn 2+ appends
    the new one)."""
    history = []
    last_user_idx = -1
    idx = 0
    for msg in context:
        if msg.get("role") == "User":
            last_user_idx = idx
        idx = idx + 1
    idx = 0
    for msg in context:
        if idx == last_user_idx:
            idx = idx + 1
            continue
        history.append({"role": msg.get("role", ""), "content": msg.get("content", "")})
        idx = idx + 1
    return history


def main(context, goal, actions, state, config):
    """Basic-mode orchestrator entry point. Runs as a resumable long-running
    loop: each iteration processes one turn, then parks on
    host.await_next_turn() until the driver feeds the next turn's input. The
    bootstrap context seeds the in-VM history once (turn 1); subsequent turns
    arrive via host.await_next_turn(). goal/actions/config are accepted to match
    the bootstrap contract but are not used by the v0 loop."""
    if not isinstance(state, dict):
        state = {}
    history = _seed_history(context)

    while True:
        # 1. Turn-start signal check.
        signal = host.check_signals()
        if signal == "stop":
            FINAL({"outcome": "stopped", "state": state})

        # 2. Park until the driver feeds this turn's user input.
        user_input = host.await_next_turn()
        if user_input == "":
            FINAL({"outcome": "completed", "response": "", "state": state})
        history.append({"role": "User", "content": user_input})

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
                answer = _non_match_answer(history, user_input)
        else:
            # disambiguation / no_match / error → Non-Matching-Mode.
            answer = _non_match_answer(history, user_input)

        # 4. Post the reply + save history (best-effort).
        if answer != "":
            host.post_reply(text=answer)
        _save_history(user_input, answer)
        history.append({"role": "Assistant", "content": answer})

        # 5. Loop back to the turn-start signal check; the next
        # host.await_next_turn() parks the VM until the following turn.


# Entry point: run the resumable loop with the injected bootstrap variables.
# main() never returns on the happy path — it parks at host.await_next_turn()
# each turn and only reaches FINAL(...) on stop/empty-input termination.
main(context, goal, actions, state, config)
