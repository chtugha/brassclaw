# Active Task Registry

Updated by the orchestrator. Shows all live sub-tasks, their status, and assigned files.

| Task ID | Package | Status | Assigned Files | Notes |
|---------|---------|--------|----------------|-------|
| TASK-001 | PKG-B: ServiceClient wiring | IN_PROGRESS | installed_monty_catalogue.rs, runtime.rs, validation_queue.rs | Add OnceLock<ValidatorServiceBridge>; wire in runtime.rs after GlobalMontyDriver creation; update validation_queue test |

## Completed Tasks

| Task ID | Package | Outcome | Commit |
|---------|---------|---------|--------|
| (session bootstrap) | PKG-A / BUILD-036 / BUILD-054..061 | DONE | d12dc8d2b |
