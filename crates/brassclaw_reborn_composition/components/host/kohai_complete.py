# The preceding assembler's result is explicitly handed off by Monty.
response = host.kohai_complete(prompt=state["previous_result"])
if not isinstance(response, dict) or response.get("ok") is not True:
    raise RuntimeError("kohai_step_failed")
answer = response.get("answer")
if not isinstance(answer, str):
    raise RuntimeError("kohai_answer_invalid")
result = answer
result
