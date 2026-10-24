# Pure recursive-contract review

`recursive_contract_draft` is an installation-retained, unreviewed PythonCode
component. Its export `check_recursive_contract(inputs)` checks one current
consumer-input, Tool-parameter or result contract as pure Recipe logic.
[Scoped native acceptance](../validation/2026-10-24-recursive-contract.md) covers
actual Monty execution, native grammar readers and installation retention.
It supplies no Skill, Tool call, evidence, approval or active route.

The pinned validator Recipe supplies two required strings:

| Input | Contract |
| --- | --- |
| `contract_json` | Complete JSON encoded from the original strict retained subject reader; this pure decoder is not a duplicate-key/integrity boundary. |
| `position` | Pinned Recipe choice: `input`, `parameters` or `result`. The reviewed candidate must not choose a weaker position. Computed arguments require a separate check and are rejected here. |

The complete result is `{rule_id:"recursive_contract",ok:boolean,errors:list[string]}`.
Diagnostics include no private field names/values. Collection/Aggregation binds
the actual result under its selected rule identity, not a caller success flag.

The rule checks closed fields, required/null/default rules, nested list items,
object fields/extras, supported checks and cross-input references. Optional
consumer inputs need defaults; genuinely optional Tool parameters may be omitted.
Defaults are checked without changing data and apply only to missing consumer
fields, never explicit null/invalid values or outputs. Numeric checks exclude
booleans and use exact normalized decimal ordering above 2**53. Integer literals
describe the native JSON representation, not a new task budget. Mutable worklists
are invocation-local; loading defines functions without effects. Iterative
traversal introduces no new depth/node/byte/row cap.

This is **grammar/default/type/constraint consistency**, not resource admission.
Native preparation/runtime owners still enforce acknowledged schema/data/default-
expansion bounds. A positive result alone does not prove those admissions accepted.
Computed arguments, compatibility, source/call/binding agreement, behavior,
provenance and complete trusted Q1 remain separate. A native grammar/profile
change requires reassessing the rule and evidence, not retaining old qualification
blindly. The installation owner preserves original drafts and operator successors
without approval or activation. Integrate through [the existing validator owner](pure-component-review.md#exact-remaining-cutover-contract)
with actual subject/native facts, run/attempt/settlement/interpreter records and
approved mandatory coverage before production Q1 or general pure approval.
