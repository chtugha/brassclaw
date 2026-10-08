# Global class-10 orchestrator source for the Phase 3a hosting cutover.
# Not activated by the legacy driver. The host must validate and pin each task's
# BuildInstruction and retain its scoped host ports across pending calls.
# No claim token, credential or shared conversation history belongs in this VM.
import asyncio


def _exact_fields(value, names):
    if not isinstance(value, dict) or len(value) != len(names):
        raise RuntimeError("recipe_composition_failed")
    for name in names:
        if name not in value:
            raise RuntimeError("recipe_composition_failed")


def _input_name(name):
    if not isinstance(name, str) or name == "":
        return False
    alphabet = "abcdefghijklmnopqrstuvwxyz"
    if name[0] not in alphabet:
        return False
    for char in name:
        if char not in alphabet + "0123456789_":
            return False
    return True


def _validate_ref(ref, available, input_names, items):
    if not isinstance(ref, dict):
        raise RuntimeError("recipe_composition_failed")
    kind = ref.get("kind")
    if kind == "input":
        _exact_fields(ref, ["kind", "name"])
        if ref["name"] not in input_names:
            raise RuntimeError("recipe_composition_failed")
    elif kind == "constant":
        _exact_fields(ref, ["kind", "value"])
    elif kind == "result" or kind == "item":
        field = "step_id" if kind == "result" else "loop_id"
        _exact_fields(ref, ["kind", field, "path"])
        allowed = available if kind == "result" else items
        if ref[field] not in allowed:
            raise RuntimeError("recipe_composition_failed")
        path = ref["path"]
        if not isinstance(path, list) or len(path) > 16:
            raise RuntimeError("recipe_composition_failed")
        for part in path:
            if not isinstance(part, str) or part == "":
                raise RuntimeError("recipe_composition_failed")
    else:
        raise RuntimeError("recipe_composition_failed")


def _flow_bound(value, maximum):
    if not isinstance(value, int) or isinstance(value, bool) or value < 1 or value > maximum:
        raise RuntimeError("recipe_composition_failed")
    return value


def _validate_flow_nodes(nodes, step_ids, input_names, available, items, seen, depth):
    # A structured tree has no arbitrary back edges. Only repeat/foreach nodes
    # repeat work, with explicit technical bounds and unique occurrence IDs.
    if depth > 16 or not isinstance(nodes, list) or len(nodes) == 0:
        raise RuntimeError("recipe_composition_failed")
    available = list(available)
    maximum = 0
    returned = False
    for node in nodes:
        if returned or not isinstance(node, dict):
            raise RuntimeError("recipe_composition_failed")
        node_id = node.get("node_id")
        if not _input_name(node_id) or node_id in seen or len(seen) >= 512:
            raise RuntimeError("recipe_composition_failed")
        seen.append(node_id)
        kind = node.get("kind")
        if kind == "step":
            _exact_fields(node, ["kind", "node_id", "step_id", "inputs"])
            step_id = node["step_id"]
            bindings = node["inputs"]
            if step_id not in step_ids or not isinstance(bindings, dict):
                raise RuntimeError("recipe_composition_failed")
            for name in bindings:
                if not _input_name(name):
                    raise RuntimeError("recipe_composition_failed")
                _validate_ref(bindings[name], available, input_names, items)
            # A selected source step has one declaration. Repetition instantiates
            # that declaration; duplicate declarations cannot change bindings.
            if step_id in seen:
                raise RuntimeError("recipe_composition_failed")
            seen.append(step_id)
            available.append(step_id)
            maximum = maximum + 1
        elif kind == "branch":
            _exact_fields(node, ["kind", "node_id", "source", "cases"])
            _validate_ref(node["source"], available, input_names, items)
            cases = node["cases"]
            if not isinstance(cases, dict) or len(cases) == 0:
                raise RuntimeError("recipe_composition_failed")
            common = None
            branch_maximum = 0
            all_returned = True
            for tag in cases:
                if not _input_name(tag):
                    raise RuntimeError("recipe_composition_failed")
                checked = _validate_flow_nodes(cases[tag], step_ids, input_names, available, items, seen, depth + 1)
                branch_maximum = max(branch_maximum, checked[1])
                all_returned = all_returned and checked[2]
                if not checked[2]:
                    if common is None:
                        common = checked[0]
                    else:
                        common = [name for name in common if name in checked[0]]
            if common is not None:
                available = common
            maximum = maximum + branch_maximum
            returned = all_returned
        elif kind == "foreach":
            _exact_fields(node, ["kind", "node_id", "source", "max_items", "body", "result"])
            _validate_ref(node["source"], available, input_names, items)
            count = _flow_bound(node["max_items"], 256)
            checked = _validate_flow_nodes(node["body"], step_ids, input_names, available, items + [node_id], seen, depth + 1)
            _validate_ref(node["result"], checked[0], input_names, items + [node_id])
            # Only the declared collected result escapes, including [] for no items.
            available.append(node_id)
            maximum = maximum + count * checked[1]
        elif kind == "repeat":
            _exact_fields(node, ["kind", "node_id", "max_iterations", "initial", "body", "update"])
            count = _flow_bound(node["max_iterations"], 64)
            _validate_ref(node["initial"], available, input_names, items)
            checked = _validate_flow_nodes(node["body"], step_ids, input_names, available, items + [node_id], seen, depth + 1)
            _validate_ref(node["update"], checked[0], input_names, items + [node_id])
            maximum = maximum + count * checked[1]
            # Exhaustion fails; an explicit return exits the entire Recipe.
            returned = True
        elif kind == "return":
            _exact_fields(node, ["kind", "node_id", "source"])
            _validate_ref(node["source"], available, input_names, items)
            returned = True
        else:
            raise RuntimeError("recipe_composition_failed")
        if maximum > 4096:
            raise RuntimeError("recipe_composition_failed")
    return [available, maximum, returned]


def _validate_flow(flow, step_ids, inputs):
    # This is a prepared runtime contract, not a new accepted database field.
    # IBS must also validate recursive schemas and pin exact approved artifacts.
    _exact_fields(flow, ["format", "body"])
    if flow["format"] != "recipe-flow/1":
        raise RuntimeError("recipe_composition_failed")
    seen = []
    checked = _validate_flow_nodes(flow["body"], step_ids, list(inputs), [], [], seen, 0)
    if not checked[2]:
        raise RuntimeError("recipe_composition_failed")
    for step_id in step_ids:
        if step_id not in seen:
            raise RuntimeError("recipe_composition_failed")


def _flow_value(ref, state, items):
    kind = ref["kind"]
    if kind == "constant":
        return ref["value"]
    if kind == "input":
        return state["inputs"][ref["name"]]
    if kind == "result":
        value = state["results"][ref["step_id"]]
    else:
        value = items[ref["loop_id"]]
    for part in ref["path"]:
        if not isinstance(value, dict) or part not in value:
            raise RuntimeError("recipe_execution_failed")
        value = value[part]
    return value


async def _run_flow_nodes(task_token, program_ref, nodes, state, items, occurrence):
    for node in nodes:
        node_id = node["node_id"]
        kind = node["kind"]
        address = occurrence + [node_id]
        if kind == "step":
            step_inputs = {}
            for name in node["inputs"]:
                step_inputs[name] = _flow_value(node["inputs"][name], state, items)
            step_id = node["step_id"]
            # Opaque program_ref and selected step_id resolve only pinned code.
            # Occurrence is data for durable effect identity, never a Tool grant.
            result = await host.run_program(task_token, program_ref, step_id, {
                "inputs": step_inputs, "occurrence": address
            })
            if not isinstance(result, dict) or result.get("ok") is not True or "return_value" not in result:
                raise RuntimeError("recipe_execution_failed")
            state["results"][step_id] = result["return_value"]
            state["previous_result"] = result["return_value"]
        elif kind == "return":
            return {"returned": True, "value": _flow_value(node["source"], state, items)}
        elif kind == "branch":
            value = _flow_value(node["source"], state, items)
            if not isinstance(value, dict) or len(value) != 1:
                raise RuntimeError("recipe_execution_failed")
            tags = list(value)
            tag = tags[0]
            if tag not in node["cases"]:
                raise RuntimeError("recipe_execution_failed")
            outcome = await _run_flow_nodes(task_token, program_ref, node["cases"][tag], state, items, address + [tag])
            if outcome["returned"]:
                return outcome
        elif kind == "foreach":
            values = _flow_value(node["source"], state, items)
            if not isinstance(values, list) or len(values) > node["max_items"]:
                raise RuntimeError("recipe_execution_failed")
            # A nested frame owns its result/item scope. A later item cannot
            # read values left by an earlier item or a different iteration.
            collected = []
            for index in range(len(values)):
                local = {"inputs": state["inputs"], "results": dict(state["results"]), "previous_result": state["previous_result"]}
                local_items = dict(items)
                local_items[node_id] = values[index]
                outcome = await _run_flow_nodes(task_token, program_ref, node["body"], local, local_items, address + [index])
                if outcome["returned"]:
                    return outcome
                collected.append(_flow_value(node["result"], local, local_items))
            state["results"][node_id] = collected
        elif kind == "repeat":
            carried = _flow_value(node["initial"], state, items)
            for index in range(node["max_iterations"]):
                local = {"inputs": state["inputs"], "results": dict(state["results"]), "previous_result": state["previous_result"]}
                local_items = dict(items)
                local_items[node_id] = carried
                outcome = await _run_flow_nodes(task_token, program_ref, node["body"], local, local_items, address + [index])
                if outcome["returned"]:
                    return outcome
                carried = _flow_value(node["update"], local, local_items)
            raise RuntimeError("recipe_execution_failed")
    return {"returned": False, "value": None}


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
    if "flow" in composed:
        # IBS returns validated captures/defaults from this pinned workflow.
        # The admitted envelope need not have the same names as local Recipe
        # inputs. Values stay data and are never pasted into step source.
        prepared_inputs = composed.get("inputs")
        if not isinstance(prepared_inputs, dict):
            raise RuntimeError("recipe_composition_failed")
        for name in prepared_inputs:
            if not _input_name(name):
                raise RuntimeError("recipe_composition_failed")
        recipe_state = {"inputs": prepared_inputs, "results": {}, "previous_result": None}
        _validate_flow(composed["flow"], step_ids, prepared_inputs)
        outcome = await _run_flow_nodes(task_token, program_ref, composed["flow"]["body"], recipe_state, {}, [])
        if not outcome["returned"]:
            raise RuntimeError("recipe_execution_failed")
        return outcome["value"]
    recipe_state = {"inputs": inputs, "results": {}, "previous_result": None}
    for step_id in step_ids:
        result = await host.run_program(task_token, program_ref, step_id, recipe_state)
        if not isinstance(result, dict) or result.get("ok") is not True or "return_value" not in result:
            raise RuntimeError("recipe_execution_failed")
        value = result.get("return_value")
        recipe_state["results"][step_id] = value
        recipe_state["previous_result"] = value
    return recipe_state["previous_result"]


async def _non_match(task_token):
    # Only the explicit No-Match branch enters this mode. The global Python
    # orchestrator owns model/Tool sequencing; Rust ports perform one operation.
    # The host assembles the selected library prefix and eligible transcript and
    # issues scoped prompt references. Source, raw prompts and grants do not
    # cross this interface. Actual model/context limits remain host-enforced.
    while True:
        surface = await host.visible_capabilities(task_token, {})
        if not isinstance(surface, dict) or not isinstance(surface.get("descriptors"), list):
            raise RuntimeError("capability_surface_invalid")
        version = surface.get("version")
        if not isinstance(version, str) or version == "":
            raise RuntimeError("capability_surface_invalid")
        visible = []
        for descriptor in surface["descriptors"]:
            if not isinstance(descriptor, dict):
                raise RuntimeError("capability_surface_invalid")
            capability_id = descriptor.get("capability_id")
            if not isinstance(capability_id, str) or capability_id == "" or capability_id in visible:
                raise RuntimeError("capability_surface_invalid")
            visible.append(capability_id)
        capability_view = {"visible_capability_ids": visible}
        bundle = await host.build_prompt_bundle(task_token, {
            "mode": "text_only", "context_cursor": None, "surface_version": version,
            "capability_view": capability_view, "checkpoint_state_ref": None,
            "max_messages": None, "inline_messages": []
        })
        if not isinstance(bundle, dict) or not isinstance(bundle.get("messages"), list):
            raise RuntimeError("prompt_bundle_invalid")
        if bundle.get("surface_version") != version:
            raise RuntimeError("prompt_bundle_invalid")
        response = await host.stream_model(task_token, {
            "messages": bundle["messages"], "surface_version": version,
            "model_preference": None, "capability_view": capability_view
        })
        if not isinstance(response, dict):
            raise RuntimeError("model_output_invalid")
        output = response.get("output")
        if not isinstance(output, dict) or len(output) != 1:
            raise RuntimeError("model_output_invalid")
        if "assistant_reply" in output:
            reply = output["assistant_reply"]
            if not isinstance(reply, dict) or not isinstance(reply.get("content"), str):
                raise RuntimeError("model_output_invalid")
            # Return typed content to the caller. Publication uses the pinned
            # reply Recipe below, with its Tool binding, kernel and effect record.
            return reply["content"]
        calls = output.get("capability_calls")
        if not isinstance(calls, list) or len(calls) == 0:
            raise RuntimeError("model_output_invalid")
        # Validate the complete batch before the first effect. The host still
        # checks current policy and the issued input reference at each dispatch.
        for call in calls:
            if not isinstance(call, dict) or call.get("surface_version") != version:
                raise RuntimeError("model_output_invalid")
            if call.get("capability_id") not in visible:
                raise RuntimeError("model_output_invalid")
            if not isinstance(call.get("input_ref"), str) or call["input_ref"] == "":
                raise RuntimeError("model_output_invalid")
            effective = call.get("effective_capability_ids", [])
            if not isinstance(effective, list):
                raise RuntimeError("model_output_invalid")
            for capability in effective:
                if capability not in visible:
                    raise RuntimeError("model_output_invalid")
            replay = call.get("provider_replay")
            if replay is not None and not isinstance(replay, dict):
                raise RuntimeError("model_output_invalid")
        for call in calls:
            outcome = await host.invoke_capability(task_token, {
                "surface_version": call["surface_version"],
                "capability_id": call["capability_id"], "input_ref": call["input_ref"]
            })
            if not isinstance(outcome, dict) or len(outcome) != 1:
                raise RuntimeError("capability_result_invalid")
            completed = outcome.get("completed")
            if completed is None:
                # Gate/process/child continuations need the durable global wait
                # adapter. Never turn a blocked/failed effect into success or
                # retry it by asking the model again. Preserve its host record.
                raise RuntimeError("capability_dispatch_incomplete")
            if not isinstance(completed, dict):
                raise RuntimeError("capability_result_invalid")
            provider_call = None
            if call.get("provider_replay") is not None:
                provider_call = dict(call["provider_replay"])
                provider_call["capability_id"] = call["capability_id"]
            # Preserve provider call/turn IDs, arguments, reasoning and signatures
            # as typed data so the next prompt can replay the actual Tool result.
            await host.append_capability_result_ref(task_token, {
                "result_ref": completed["result_ref"], "safe_summary": completed["safe_summary"],
                "provider_call": provider_call, "model_observation": None
            })


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
        answer = await _non_match(task_token)
        reply_recipe = await host.resolve_component_by_name(task_token, "host-post-reply", 21)
        if not isinstance(reply_recipe, dict):
            raise RuntimeError("recipe_composition_failed")
        reply_id = reply_recipe.get("id")
        reply_link = reply_recipe.get("step_link")
        if not isinstance(reply_id, str) or reply_id == "":
            raise RuntimeError("recipe_composition_failed")
        if not isinstance(reply_link, str) or reply_link == "":
            raise RuntimeError("recipe_composition_failed")
        reply_ref = await _execute_recipe(task_token, reply_id, reply_link, {"answer": answer})
        if not isinstance(reply_ref, str) or not reply_ref.startswith("msg:"):
            raise RuntimeError("recipe_reply_invalid")
        # The reply Recipe persists the real scoped transcript. Verify its actual
        # finalization before completing; no duplicate reply or history replay.
        answer = await host.resolve_reply(task_token, reply_ref)
        if not isinstance(answer, str):
            raise RuntimeError("recipe_reply_invalid")
        return reply_ref
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
    # Syntax of a ref is not proof of publication. Resolve only the finalized
    # reply retained by this exact admitted task host. The Rust host fences the
    # lookup before returning its content; Monty owns the history data handoff.
    answer = await host.resolve_reply(task_token, reply_ref)
    if not isinstance(answer, str):
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
        "user_input": user_input, "answer": answer, "reply_ref": reply_ref
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
            # VM hosting control, not a Tool invocation or permission grant.
            # This protected scope permits one task to fail without ending the
            # permanent worker or poisoning the instance control latch.
            host.enter_task(task_token)
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
                              "non_match_instruction_unavailable", "task_compute_exceeded",
                              "task_accounting_failed", "task_cancelled", "capability_surface_invalid",
                              "prompt_bundle_invalid", "model_output_invalid", "capability_result_invalid",
                              "capability_dispatch_incomplete"]:
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
