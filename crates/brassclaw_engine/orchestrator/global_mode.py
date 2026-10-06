# Global class-10 orchestrator source for the Phase 3a hosting cutover.
# Not activated by the legacy driver. The host must validate and pin each task's
# BuildInstruction and retain its scoped host ports across pending calls.
# No claim token, credential or shared conversation history belongs in this VM.
import asyncio


async def _execute_recipe(task_token, recipe_id, step_link, inputs):
    # Composition returns a task-owned program reference, not authority to run
    # arbitrary Python supplied by the caller. The host resolves each step from
    # the pinned manifest when run_program is called.
    composed = await host.compose_orchestrator(task_token, recipe_id, step_link, inputs)
    if not isinstance(composed, dict) or composed.get("ok") is not True:
        raise RuntimeError("recipe_composition_failed")
    program_ref = composed.get("program_ref")
    steps = composed.get("steps")
    if not isinstance(program_ref, str) or program_ref == "":
        raise RuntimeError("recipe_composition_failed")
    if not isinstance(steps, list) or len(steps) == 0:
        raise RuntimeError("recipe_composition_failed")
    # Validate the entire dispatch list before the first step can have effects.
    step_ids = []
    for step in steps:
        if not isinstance(step, dict):
            raise RuntimeError("recipe_composition_failed")
        step_id = step.get("step_id")
        if not isinstance(step_id, str) or step_id == "" or step_id in step_ids:
            raise RuntimeError("recipe_composition_failed")
        step_ids.append(step_id)
    recipe_state = {"inputs": inputs, "results": {}, "previous_result": None}
    for step_id in step_ids:
        result = await host.run_program(task_token, program_ref, step_id, recipe_state)
        if not isinstance(result, dict) or result.get("ok") is not True:
            raise RuntimeError("recipe_execution_failed")
        value = result.get("return_value")
        recipe_state["results"][step_id] = value
        recipe_state["previous_result"] = value
    return recipe_state["previous_result"]


async def _execute_task(task):
    # The routing token indexes the admitted Rust-owned task host. Payload IDs
    # are data; changing one in Python cannot switch host authority.
    task_token = task["task_token"]
    user_input = task["user_input"]
    inputs = {"user_input": user_input, "history": task["history"]}
    intent = await host.resolve_intent(task_token, user_input)
    if not isinstance(intent, dict):
        raise RuntimeError("intent_resolution_failed")
    status = intent.get("status")
    if status == "match":
        recipe_id = intent.get("component_id")
        step_link = intent.get("step_link")
    elif status == "no_match":
        recipe = await host.resolve_component_by_name(task_token, "host-non-match-llm-answer", 21)
        if not isinstance(recipe, dict):
            raise RuntimeError("non_match_instruction_unavailable")
        recipe_id = recipe.get("id")
        step_link = recipe.get("step_link")
    elif status == "disambiguation":
        raise RuntimeError("intent_disambiguation_required")
    else:
        raise RuntimeError("intent_resolution_failed")
    if not isinstance(recipe_id, str) or recipe_id == "":
        raise RuntimeError("recipe_composition_failed")
    if not isinstance(step_link, str) or step_link == "":
        raise RuntimeError("recipe_composition_failed")
    reply_ref = await _execute_recipe(task_token, recipe_id, step_link, inputs)
    # The Recipe posts through the transcript port and returns its message ref.
    # Never publish the last step again or stringify structured model/tool output.
    # The host must verify this ref against the task's real finalized message.
    if not isinstance(reply_ref, str) or not reply_ref.startswith("msg:"):
        raise RuntimeError("recipe_reply_invalid")
    history_recipe = await host.resolve_component_by_name(task_token, "host-save-history", 21)
    if not isinstance(history_recipe, dict):
        raise RuntimeError("history_persistence_failed")
    history_id = history_recipe.get("id")
    history_link = history_recipe.get("step_link")
    if not isinstance(history_id, str) or history_id == "":
        raise RuntimeError("history_persistence_failed")
    if not isinstance(history_link, str) or history_link == "":
        raise RuntimeError("history_persistence_failed")
    await _execute_recipe(task_token, history_id, history_link, {
        "user_input": user_input, "reply_ref": reply_ref
    })
    return reply_ref


async def _worker(worker_id):
    while True:
        task = await host.await_next_task(worker_id)
        # Only the trusted instance shutdown path supplies None. Empty chat
        # messages, task errors and cancellation never terminate this worker.
        if task is None:
            return
        # Admission validates the envelope before supplying it to this VM.
        # Malformed transport is fatal; it must not invent a task identity.
        if not isinstance(task, dict):
            raise RuntimeError("task_envelope_invalid")
        task_token = task.get("task_token")
        if not isinstance(task_token, str) or task_token == "":
            raise RuntimeError("task_envelope_invalid")
        outcome = "completed"
        reply_ref = None
        reason_kind = None
        try:
            reply_ref = await _execute_task(task)
        except Exception as error:
            # The service retains the actual host failure/cancellation reason.
            # Do not expose arbitrary exception text or replay as Tier 2.
            outcome = "failed"
            reason_kind = "task_execution_failed"
            safe_reason = str(error)
            if safe_reason in ["recipe_composition_failed", "recipe_execution_failed",
                              "recipe_reply_invalid", "history_persistence_failed",
                              "intent_resolution_failed", "intent_disambiguation_required",
                              "non_match_instruction_unavailable"]:
                reason_kind = safe_reason
            safe_reason = None
        await host.finish_task(task_token, {
            "status": outcome, "reply_ref": reply_ref, "reason_kind": reason_kind
        })
        # Release all task-local history and values before the next work wait.
        task = None
        task_token = None
        outcome = None
        reply_ref = None
        reason_kind = None


async def _global_main():
    # The hosting service supplies its validated admission capacity at boot.
    # A fixed number of coroutines provides bounded active task slots without
    # requiring Monty's unsupported asyncio.create_task or Queue APIs.
    if not isinstance(worker_count, int) or isinstance(worker_count, bool) or worker_count < 1:
        raise RuntimeError("orchestrator_capacity_invalid")
    workers = []
    for worker_id in range(worker_count):
        workers.append(_worker(worker_id))
    await asyncio.gather(*workers)


asyncio.run(_global_main())
