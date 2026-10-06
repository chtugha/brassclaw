# Pure prompt assembly. Inputs are values supplied by Monty, never Python source.
inputs = state["inputs"]
result = {
    "chat_history": inputs["history"],
    "user_query": inputs["user_input"],
    "prefix_placeholder": inputs.get("prefix_placeholder", "")
}
result
