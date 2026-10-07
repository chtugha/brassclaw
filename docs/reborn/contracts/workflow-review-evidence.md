# Retained Recipe workflow observations

`WorkflowReview` records structural and actual step-execution observations for
one selected Recipe variant. These records supplement the one-Tool usage
records; they neither replace `skill-association/1` nor establish workflow
approval, human Q2, protected-root trust or catalogue activation.

The review subject and durable task journal share the sealed
`RetainedRecipeSelection` derived from the real IBS instruction. Its exact
bytes identify the Recipe, embedded variant, `step_link`, input layout,
selected order and complete immutable dependency references. Runtime values
and executable BuildInstruction objects are not part of that selection.

Structural observations originate in the contained source inspector and
prepared typed program. Behavioral observations originate in the actual
child executor, with observation enabled before execution. They retain typed
input, argument, answer and result fingerprints, classified failures and the
actual execution order. Pure-logic steps are supported without inventing a
Tool association. A matching set of results in the wrong order is rejected.

Author-supplied expectations are compared with those observations; they cannot
write a success flag. Failed expectations remain failed records. A declared
negative case identifies the exact failing step and classification; a failed
prefix cannot certify all selected steps. Missing, reordered, repeated,
unsettled or transport-uncertain executions cannot produce passing evidence.
The current producer covers flat, once-per-step execution; branches, retries
and general continuation evidence require their corresponding runner support.

Reports distinguish selected-variant structure and step execution from semantic
approval and root task completion. Both `semantic_approval` and
`task_completion` remain false. Even a successful final child result does not
prove a user reply was published or the global task actually settled.

V106 stores these observations separately from usage-specific evidence. The
trusted writer rechecks the complete exact revision selection in one
repeatable-read transaction. Evidence and selection checksums are verified,
records cannot be updated/deleted/truncated, and uncertain commits retain the
original sealed review and ID for idempotent persistence. Recovery never
reruns completed effects or reads newer component versions.

Native acceptance uses the real worker, PostgreSQL revision/journal stores and
kernel JSON Tool path. Pure-logic probes remain explicitly constrained draft
validation; they do not claim ordinary factory startup or whole-task completion.
The approved-catalogue, whole-workflow review and protected-root owners remain
prerequisites for the production Monty cutover.
