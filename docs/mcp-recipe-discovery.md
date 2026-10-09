# Inbound MCP Recipe discovery

Discovery reads the same installation-owned catalogue that ordinary chat uses.
It does not query legacy Skill rows, infer eligibility from names, or execute
components. `RebornRuntime::mcp_recipe_discovery()` exposes a read-only facade;
only the catalogue owner can publish a generation. Publication uses an expected
generation comparison under one lock. Readers retain immutable snapshots. A
failed qualification makes new discovery unavailable; it does not silently
continue advertising a known invalid catalogue. A losing stale publisher does
not replace the current generation.

The current installation catalogue contains the packaged reply and internal
history workflows. Neither opts into public MCP discovery. Startup publishes a
qualified **empty** discovery generation, rather than projecting those Skills as
tools. This is different from an unavailable catalogue. The default transport
router still reports unavailable without an attached discovery facade. A host
can attach the runtime facade through `orchestrator_mcp_router_with_discovery`;
authentication belongs to host ingress. This change starts no listener, enables
no provider connection and enables no `tools/call` execution.

## Retained command declaration

An MCP candidate explicitly declares `mcp_call` in its immutable retained Recipe
revision document. This is a newly implemented discovery metadata reader, **not
a field accepted by the legacy Recipe INSERT API** and not an approval receipt.
The declaration must be part of the exact Recipe review/bootstrap subject. An
authored change still requires the authored qualification/activation path;
shipping a bundled declaration requires the installation owner's distinct
integrity and behavioral qualification.

The declaration format is `mcp-call-skill-recipe/1`. The example below is a
structural draft, not an activated Recipe:

```json
{
  "format": "mcp-call-skill-recipe/1",
  "name": "read_interval",
  "purpose": "Read a specified line interval from a file.",
  "skill_uuid": "00000000-0000-0000-0000-000000000001",
  "variant_key": "selected",
  "export_name": "read_interval",
  "sentence": "read file %; interval %",
  "variables": [
    {"name": "path", "position": 0, "encoding": "verbatim"},
    {"name": "interval", "position": 1, "encoding": "verbatim"}
  ],
  "formatting_rules": "Use the exact sentence. Values must not contain '; interval '.",
  "examples": ["read file /workspace/example.txt; interval 1:4"],
  "negative_examples": ["write file /workspace/example.txt; interval 1:4"],
  "result_description": "The selected line interval, formatted by the Recipe's normal reply step.",
  "error_description": "Invalid inputs stop before effects; other failures return through ordinary chat."
}
```

The reader rejects unknown fields. Stable Skill UUID and exact selected variant
are required. The sentence must appear in that retained variant's intent
examples. Variables must follow its `variable_patterns` names and zero-based
positions. V1 public commands require `variable_patterns.pattern` to be null:
custom capture regexes can transform values and cannot be qualified from sample
agreement alone. Use explicit typed Recipe parsing steps for such transformations.
Recursive variable contracts come from the same retained variant's
`input_layouts` task contract, not a second MCP schema. The selected executable
binding must identify that Skill exactly once. Positive examples must bind to
the same data through both the command-format reader and ordinary typed Recipe
capture. Negative examples must fail format validation. These checks establish
structural agreement; they do not prove normal intent matching or behavior.

The current reader supports required, non-null strings transported verbatim.
Values must be nonempty, and literal separators inside values are rejected.
There is no quote or backslash escaping at the command layer: quotes,
backslashes, Unicode, braces and source-looking text remain string data. JSON
escaping in the MCP transport is separate. Numeric, boolean and structured
values require supported reviewed Recipe parsing components and further
discovery-contract support; this reader does not invent coercion. Optional task
inputs may use existing consumer defaults, but every required input must have
an advertised slot. Technical command and catalogue capacity limits reject
whole requests/generations, rather than truncate them.

Duplicate tool names, identical command templates and overlaps demonstrated by
the supplied examples reject the whole generation. Complete ambiguity checking
still belongs to the normal match qualification. There is no first-row winner,
silent older-version fallback or Tier-2 replay.

## Complete declared-command qualification

The installation catalogue now qualifies discovery against real durable
normal-chat executions and the current ordinary matcher, in one repeatable-read
PostgreSQL transaction. It reads exact retained component revisions and immutable
Recipe-selection receipts. It never executes an example, calls a Tool, constructs
a second interpreter or accepts a client-supplied success report.

A public Recipe additionally declares `mcp_qualification` in its immutable
retention document (not a legacy INSERT field):

```json
{
  "format": "mcp-command-cases/1",
  "success_examples": [
    {"command": "read file /workspace/example.txt; interval 1:4",
     "reply": "The exact expected formatted reply for this controlled fixture."}
  ],
  "failure_examples": [
    {"command": "read file /workspace/missing.txt; interval 1:4",
     "reason_kind": "recipe_execution_failed"}
  ]
}
```

This is an example contract, not a qualified file-reading implementation. Every
`mcp_call.examples` command must occur exactly once in `success_examples`; extras,
duplicates, missing cases and unknown fields fail. At least one unique, correctly
formatted failure case is required (at most 64). Case expectations belong to the
reviewed Recipe revision. These finite cases supplement semantic/behavioral
approval; they do not prove all possible Tool behavior or environmental failures.

For every positive and failure case, the real matcher must unambiguously select
the exact declared sentence, Recipe, variant and complete retained selection.
Actual typed capture must equal MCP capture. A settled ordinary execution must
exist for that command/input/generation/selection. Positive cases require complete
Recipe and task execution, a correlated finalized reply whose content checksum
matches the expected text, valid accounting and no withheld answers. A failure
case requires the exact terminal classification and a failed selected Recipe
with its actual completed prefix and failed step; a failure in later history or
another child cannot substitute. Missing/unsettled observations, wrong replies,
changed generations and unsuccessful examples cannot qualify.

Every declared negative must fail command formatting and return genuine No-Match
from the ordinary matcher. Matching errors, another match and disambiguation
reject qualification. Additionally, qualification compares the command language
with **all** incoming eligible intent templates, including other templates of
the same Recipe. Compatible literal prefixes and suffixes conservatively reject
potential overlap; it never relies on scores, row order or the supplied examples
alone. This may reject disjoint templates whose separation needs a stronger
language proof; authors should choose clearly distinct command sentences.

Qualification also checks the actual eligible intent rows against every retained
template, including its template flag and prefix/suffix anchors. Missing rows or
corrupt anchors reject qualification. The normal matcher repeats this integrity
check for a public packaged catalogue in its qualification scope before routing;
such corruption is a composition error, never permission to enter Tier 2.
The immutable proof includes the observed routing-metadata checksum.

The selected public export must be the actual inspected invocation for the
matching Skill binding. Every executable step must use the inspected preload
path. Exact interface/code/transitive dependencies and association references
remain in the retained selection; the normal catalogue owner separately verifies
package integrity, semantic approval and actual immutable Tool implementations.
Qualification neither creates approval nor grants Tool permission.

The committed `mcp-command-qualification/1` artifact records command/selection
fingerprints, observed run/outcome/root identities, replies, failures, negatives,
preload order and the routing selection set. Migration V123 stores it immutably
and rejects update/delete/truncate. Only this committed, privately constructed
proof can populate discovery. Publication requires the exact generation and
complete command-contract set; advertised fingerprints also include the proof.
An empty proof cannot qualify a newly declared command.

To avoid circular bootstrap, missing execution observations leave MCP discovery
unavailable while the already approved normal Recipe catalogue continues to
serve ordinary chat. After acknowledged durable task settlement, that same
catalogue owner retries qualification. It performs no example replay or hidden
qualification effects. Structural/DB failures remain errors, and a stale
publisher cannot overwrite a successful refresh. Once qualified, further turns
do not rebuild that immutable generation's proof. Startup/restart rechecks it.
Authored activation/withdrawal still needs its separate supported owner/event
integration; these observations cannot admit unapproved components.

Today's packaged reply/history workflows still have no public `mcp_call`, so
startup qualifies and retains an empty discovery generation. No public command,
MCP chat bridge, authenticated listener or provider window is enabled by this
implementation.

## Normal-match execution observations

The normal installation catalogue now creates `monty-normal-match/1` evidence
only after its actual PostgreSQL matcher/IBS transaction commits. It identifies
the pinned catalogue generation, exact Recipe revision/checksum, embedded
variant and step link, and complete retained-selection checksum. Command,
matched-template and validated task-input checksums link an observed command
to that selection without storing its raw values in this report. These hashes
are correlation/integrity data, not encryption or an authorization grant.

The global Recipe adapter retains that record only on the matched selection.
Internal named reply/history lookup cannot create it. Before completion, the normal reply check captures a checksum of the finalized
reply content. After service quiescence, the existing durable task settlement
stores it together with actual Recipe completion, root/task completion and the
correlated reply reference. Settlement reuses this task-owned observation even
if the host has since been fenced; it performs no reply read or dispatch to
reconstruct it. Matching
errors, genuine No-Match and disambiguation remain separate routing outcomes;
a failed task can retain a successful matching observation without claiming a
completed command. Settlement retries retain the original report and never
repeat matching or effects to reconstruct evidence.

The command qualification owner above consumes these observations and checks
the complete declared suite against the same selection. A single completed match
cannot enable a listing. Semantic approval and activation remain separate;
clients cannot submit a normal-match record or a success boolean to enable MCP.

## Advertisement and command admission

An advertised snapshot pins command contracts. Before ordinary chat submission,
`validate_advertised_command` requires the name to remain present with the same
contract fingerprint; withdrawn or changed commands are stale. Unrelated
catalogue additions need not invalidate unchanged contracts. The facade performs
no chat submission itself. The later chat task selects and retains its own
coherent current catalogue through the existing matcher/IBS path; the MCP
snapshot is never a shortcut to execute a historical Recipe.

The listing projection supplies MCP `name`, `description` and an `inputSchema`
that accepts only `arguments.command`. Description supplies the exact sentence,
variable positions/contracts, formatting rules, examples and result/error
descriptions. It makes no unsupported `outputSchema` claim about the future
formatted chat response. The bounded list is one generation without pagination;
cursor/filter parameters are rejected. SSE list-change notifications remain
unsupported and `listChanged` remains false, following the
[MCP tools protocol](https://modelcontextprotocol.io/specification/2025-06-18/server/tools).

Future authored activation/withdrawal must publish discovery with the normal
catalogue owner, including durable generation identity and failure handling.
There is currently no authored activation event to subscribe to. Startup and
restart rebuild the view from the retained packaged catalogue. Kohai must still
capture and advertise the selected view for its request window; the chat bridge
must still provide durable correlation, non-destructive closure and no-replay
recovery. Neither connection lifecycle nor call admission is established by a
successful discovery response. Legacy outbound MCP clients remain suspended.
