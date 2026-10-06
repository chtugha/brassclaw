# Format admitted values as text. Never interpolate them into executable source.
inputs = state["inputs"]
summary = {"user_input": inputs["user_input"], "answer": inputs["answer"]}
for key in ["mode", "matched_component", "timestamp"]:
    if key in inputs:
        summary[key] = inputs[key]
body = "## Turn summary\n"
for key, value in summary.items():
    body = body + "- **" + key + "**: " + str(value) + "\n"
result = body
result
