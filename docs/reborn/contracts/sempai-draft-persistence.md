# Sempai draft persistence

The PostgreSQL proposal sink accepts the existing `proposed_components` transport
for classes 21 and 22. The legacy `proposed_recipe_updates` list uses the same
Recipe decoder. Unsupported classes and malformed proposals are reported and
not counted as queued; other proposals may still be saved. The original outcome
remains model proposal data, never approval evidence.

Both payloads require string `name` and `description`. PythonCode also requires
string `content`. Supported optional fields are:

| Both classes | Recipe only | PythonCode only |
| --- | --- | --- |
| `prior_knowledge_content`, `override_prompt_creation`, `consumer_tags`, `intent_examples`, `dependency_registry` | `trigger`, `steps`, `step_descriptions`, `variants`, `validates_class_code` | `includes` |

Absent `steps` and `includes` default to empty arrays; explicit null is rejected.
Includes are UUID strings. Absent override defaults to false; non-booleans and
explicit null are rejected. Optional JSON fields preserve absent versus present
JSON null. Their nested contents are retained for validation, not approved by
this decoder. Python source, Unicode, structured IBS fields and array order are
preserved; runtime values are never substituted into source.

Unknown top-level fields are rejected rather than discarded. In particular,
scope, component identity/class, source, validation status, tier, approval and
integrity checksums are host-owned. New schema/association fields need actual
constructor and review support before this adapter can accept them; they cannot
be invented in a model payload. Supplied routing tags are retained with
`05:validator` added, but tags provide neither approval nor Tool authority.

Each new legacy component and its state-1 validation queue entry commit in one
transaction. Queue-write and commit failures leave neither an orphan draft nor
a successful receipt. V105 admits the actual `sempai_proposal` PythonCode origin
while retaining the existing restricted provenance set. The sink still fixes
the source and pending status; a model cannot claim `system` provenance.
Diagnostics expose field-shape rejection or SQLSTATE without candidate source.

This repairs the legacy transport. It does **not** establish immutable review
identities, complete proposed dependency selection, unknown-commit idempotency,
all-class validation, exact association approval or activation. Sempai still
needs migration to [immutable review submissions](component-review-submissions.md)
and the durable review owner specified in [validator_v3](../../../validator_v3.md).
Duplicate raw JSON keys must be rejected at the original response-byte parser;
this Value-based sink cannot recover keys already lost upstream. Separate
Sempai reference/provider/prefix wiring follows the
[prefix upgrade plan](../../plans/prefix_v3_upgrade.md).
