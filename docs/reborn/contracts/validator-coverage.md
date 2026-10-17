# Retained validator coverage

`validator_coverage` is supported metadata inside an immutable class-21 revision
document. It is not a new legacy Recipe INSERT field or approval evidence.
The initial `validator-coverage/1` contract applies to the existing flat retained
occurrence layout. Broader branch/wait coverage needs its separately qualified
runner/evidence contract.

```json
{
  "format": "validator-coverage/1",
  "rules": [{"rule_id": "source_profile", "step_id": "0:2"}],
  "aggregate_step": "0:4"
}
```

This is a declaration fragment, not an insertable or complete validator Recipe.
Resolve real component UUIDs and supported steps, contracts and input layouts.
Rules use distinct lowercase snake-case IDs and distinct selected executable
steps. They precede a collection step and aggregation. Each rule returns a strict
non-null object with required `rule_id:string`, `ok:boolean` and `errors:list`
of non-null strings. The invocation boundary validates the concrete output;
missing or invalid results cannot be supplied by defaults.

The aggregation input `required_rule_ids` must be a Recipe constant matching the
declaration's ordered IDs. `checks` must reference the complete result of an
executed preceding collection component. That component binds every complete
rule-step result exactly once under that rule's pinned ID, with no candidate inputs, constants or partial
field projections substituting for them. Its behavior still needs qualification;
binding completeness alone cannot prove that its code preserves all results.
The aggregate declares required boolean `pass` and list `errors` outputs.

The selected validator has `consumer_tags=["05:validator"]`, no ordinary intent
examples and `validates_class_code` selecting one of 0, 1, 2, 3, 13, 21, 22 or 23.
Protected class-10/50 changes need their separate owner and trust path. The
catalogue preparation consumer checks these declarations against retained IBS
before admitting a validator-tagged workflow.

Whole-generation preparation also rejects multiple selected private entries for
one class, including multiple variants. Either class-route or coverage metadata
requires the complete private validator contract even if its tag is omitted.
An activated Recipe must have at least one selected variant. These checks apply
on initial publication and retained catalogue restoration.

`validator_components::collect_draft` creates a reusable class-22 PythonCode
draft with a preloadable `collect_validator_checks(inputs)` export. Each declared
rule name is a required strict typed input. The function preserves the complete
results and diagnostics in a list and rejects a result whose label differs from
its pinned input name; it neither repairs contradictory results nor
fabricates success. Use one Recipe step per actual rule, a separate collection
step bound to their complete results, then the pinned aggregation export. The
collector needs its own qualification and exact retained revision before use.
The packaged drafts do not activate these workflows or establish approval.

This establishes preparation consistency only. It neither proves that the class
has all required semantic/behavioral checks nor that any rule ran or passed.
Approved private catalogue selection, actual observed rule labels/coverage,
attempt fencing, trusted verdict persistence, authored Q2 or controlled shipped
bootstrap evidence remain necessary. Public pass JSON, structural-only evidence,
labels and these declarations cannot activate a validator or approve a candidate.

## Executor-owned checking observations

`RetainedValidatorCoverage::observe` constructs an opaque
`ObservedValidatorChecks` only after the actual selected executor completes.
The selection checksum, complete step order, settled result fingerprints,
whole rule results, unique labels and aggregation must agree. Missing, changed,
incomplete, unobserved or foreign-selection data cannot construct that value.
The review owner supplies its report bounds; capacity failure grants no replay.

`RetainedRecipeExecution::new_child_for_behavioral_validation` enables these
observations beneath the actual same-task parent. It executes only the step
requested by Monty and changes neither workflow selection nor dispatch authority.

The resulting `observed-validator-checks/1` report has
`scope="selected-validator-execution"`, `semantic_approval=false` and
`q1_qualification=false`. It records checking outcomes rather than claiming full
class coverage, semantic approval or durable task completion. The production
review owner still needs an approved private validator, the exact sealed subject,
complete class requirements, attempt fencing, actual settlement and the supported
trusted evidence writer before producing a qualifying Q1 record.

For existing usage review, `skills.md` assigns semantic agreement to author
review, supported Q1 audits, behavioral validation and human Q2 together.
A structural Q1 record's `semantic_approval=false` is not itself a failed audit;
do not use a blanket flag check to reject that supported combination. Likewise,
the mere presence of a successful structural record cannot establish complete
class or whole-workflow qualification.
