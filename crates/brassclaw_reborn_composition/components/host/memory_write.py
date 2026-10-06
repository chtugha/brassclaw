# The formatter's value is task-local data passed by the Monty orchestrator.
result = host.memory_write(content=state["previous_result"], target="daily_log")
result
