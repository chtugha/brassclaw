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
#      - "no_match"     → resolve and execute the Tier-2 Instruction/Recipe.
#      - "disambiguation" → explicit selection required; no LLM replay.
#      - "error" / malformed status → fail this task.
#      Composition/execution failures never switch paths or call the LLM directly.
#   5. `host.post_reply(text=answer)`; resolve + run the `host-save-history`
#      recipe; persistence failure remains visible; append the Assistant answer
#      to `history` after successful persistence.
#   6. Loop back to step 1 (park at the next `host.await_next_turn()`).
#
# Task-contract exceptions propagate to the driver and remain failures. Global
# hosting must isolate these inside task continuations during the Phase 3a cutover;
# this legacy per-chat loop is not proof of global failure isolation.
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


def _run_steplist(program, recipe_state):
    """Iterate program.steplist, running each step's executable_code via
    host.run_program. Returns {ok, answer}: ok=False on the first failed step
    (answer = last good step's text); ok=True with the last step's text."""
    # Validate the complete executable plan before the first effect. A missing
    # or malformed later step must not leave a partially executed recipe.
    if not isinstance(program, dict):
        return {"ok": False, "answer": ""}
    steplist = program.get("steplist")
    if not isinstance(steplist, list) or len(steplist) == 0:
        return {"ok": False, "answer": ""}
    for step in steplist:
        if not isinstance(step, dict):
            return {"ok": False, "answer": ""}
        code = step.get("executable_code")
        if not isinstance(code, str) or code.strip() == "":
            return {"ok": False, "answer": ""}
    # program.skills is carried for consultation (exact tool usage). Per-step
    # executable_code is already concrete (compose_orchestrator baked in the
    # {{vars}} substitution + tool calls), so v0 runs it as-is.
    skills = program.get("skills", [])
    last_answer = ""
    for step in steplist:
        code = step.get("executable_code", "")
        # Monty owns the handoff. Runtime values travel as data, never as
        # substituted Python source. Unrelated Recipe executions get their
        # own state; a child runner receives only this Recipe's input/results.
        result = host.run_program(code, recipe_state)
        if not isinstance(result, dict) or result.get("ok") is not True:
            return {"ok": False, "answer": last_answer}
        rv = result.get("return_value")
        recipe_state["previous_result"] = rv
        if rv is None:
            rv = result.get("stdout", "")
        step_text = _stringify(rv)
        if step_text != "":
            last_answer = step_text
    return {"ok": True, "answer": last_answer}


def _compose_and_run(component_id, step_link, user_input, recipe_state=None):
    """Execute the selected Recipe once. A failed step never restarts as Tier 2."""
    if component_id == "" or step_link == "":
        raise RuntimeError("recipe_composition_failed")
    composed = host.compose_orchestrator(component_id, step_link, user_input)
    if not composed.get("ok"):
        raise RuntimeError("recipe_composition_failed")
    program = composed.get("program")
    if program is None:
        raise RuntimeError("recipe_composition_failed")
    if recipe_state is None:
        recipe_state = {"inputs": {"user_input": user_input}, "previous_result": None}
    ran = _run_steplist(program, recipe_state)
    if not ran.get("ok"):
        raise RuntimeError("recipe_execution_failed")
    return ran.get("answer", "")


def _non_match_answer(context, user_input):
    """Only genuine No-Match executes the Tier-2 Instruction/Recipe.
    Missing or failed instructions are errors, never direct LLM fallbacks."""
    recipe = host.resolve_component_by_name("host-non-match-llm-answer", 21)
    if recipe is None or recipe.get("id", "") == "":
        raise RuntimeError("non_match_instruction_unavailable")
    answer = _compose_and_run(recipe.get("id"), "0:1-0:E", user_input, {
        "inputs": {"user_input": user_input, "history": context[:-1]},
        "previous_result": None
    })
    if answer == "":
        raise RuntimeError("non_match_instruction_failed")
    return answer


def _save_history(user_input, answer):
    """Persist through the history Recipe; failures remain visible.
    The caller has already posted its reply: never replay completed effects."""
    recipe = host.resolve_component_by_name("host-save-history", 21)
    if recipe is None or recipe.get("id", "") == "":
        raise RuntimeError("history_persistence_failed")
    _compose_and_run(recipe.get("id"), "0:1-0:E", user_input, {
        "inputs": {"user_input": user_input, "answer": answer},
        "previous_result": None
    })


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
            continue

        # 2. Park until the driver feeds this turn's user input.
        user_input = host.await_next_turn()
        if user_input == "":
            FINAL({"outcome": "completed", "response": "", "state": state})
            continue
        history.append({"role": "User", "content": user_input})

        # 3. Resolve intent + dispatch.
        intent = host.resolve_intent(user_input=user_input)
        status = intent.get("status", "error")

        answer = ""
        if status == "match":
            component_id = intent.get("component_id", "")
            step_link = intent.get("step_link", "")
            answer = _compose_and_run(component_id, step_link, user_input)
        elif status == "no_match":
            answer = _non_match_answer(history, user_input)
        elif status == "disambiguation":
            raise RuntimeError("intent_disambiguation_required")
        else:
            raise RuntimeError("intent_resolution_failed")

        # 4. Post the reply + save history. Never replay if persistence fails.
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
