# Complete nonexecuting source inventories

The existing registered `builtin.source_inspection` primitive supports two exact
input profiles. Neither executes candidate code or establishes Q1/approval:

```json
{"source":"def identity(value):\n    return value\n"}
```

```json
{"sources":{"opaque-id":"def identity(value):\n    return value\n"}}
```

Supply exactly one field. Inventory keys are nonempty opaque strings, including
Unicode; they are not Python identifiers or paths to read. Every value is a
nonnull string. Empty source text and an explicitly empty inventory are valid
mechanical inputs. Extra fields, mixed profiles, nulls and nonstrings fail.
There is no caller-selected worker, policy, budget or executable.

The single-source profile retains `python-source-facts/1`. The inventory profile
returns one exact JSON **string** with these fields:

| Field | Meaning |
| --- | --- |
| `format` | `python-source-inventory/1`. |
| `preparation_revision` | Actual acknowledged preparation settings captured once for this operation. |
| `worker_checksum` | SHA-256 of the retained worker image actually used. |
| `sources` | Complete map with exactly the original IDs, each carrying the result below. |

Each inspected entry is `{kind:"inspected",structure:SourceStructure}` with the
existing full source-checksummed native facts: preload/scopes/signatures,
syntactic calls, imports/references, ranges and traversal observations. An actual
compiler-profile rejection is
`{kind:"compile_rejected",source_checksum:string,category:string}`. Categories
are `syntax`, `unsupported` and `import`, derived from the native compiler's
exception type without exposing exception/source text. A syntax category can
also reflect the selected compiler's nesting guard. These facts do not establish
semantic invalidity, behavioral correctness, complete class review or permission.
Native resource/infrastructure failure is an error of the **whole operation**,
never a successful rejection entry or a skipped source.

One disposable process compiles and inspects the whole map under one captured
deadline and physical allocator ceiling. No opcode runs; there are no host ports.
The acknowledged `max_source_bytes` applies to the sum of source bytes. Input
nodes/ID-and-text bytes, total inspected AST nodes and complete output
nodes/bytes/depth also share their captured limits. The full encoded reply is
counted without allocating another frame first. Overflow fails the whole request;
there is no partial successful inventory. Actual source hashes, original IDs and
aggregate bounds are checked again on the returned native result.

The primitive uses the existing instance-owned retained worker and settings
owner, with ordinary live kernel Tool checks. It creates no catalogue, reviewer,
workflow scheduler, facts cache or global VM. Existing wall-time observation still
includes the owned request copy and native operation. Completing or failing this
utility does not replace the instance orchestrator.

The paired disposable utility protocol is **5**; the global/process service
protocol stays **20**, and the interpreter dump ABI is unchanged. Old utility
workers are rejected rather than used as a fallback. Product and worker artifacts
must remain paired. [Scoped native acceptance](../validation/2026-10-23-source-inventory.md)
includes both profiles, actual policy/settings, existing parser/formatter consumers
and the unchanged model-encoding operation against a real offline reference.

The canonical inventory Skill/export/ToolSkill/execution Recipe, original-subject
source extraction and complete class-22 checking workflow still need qualification
and integration under [validator Step 5–10](../../validator_v3.md#remaining-implementation--consolidated-2026-10-22).
Use actual Tool-result edges and the existing subject-bound observer. Caller-shaped
facts or these report fields alone are not trusted review evidence. General pure
approval, mandatory rules, durable scheduling/claim/attempt/settlement/interpreter
records and activation remain governed by [the shared cutover contract](pure-component-review.md#exact-remaining-cutover-contract).
