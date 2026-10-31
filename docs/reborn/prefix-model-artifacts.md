# Prefix model artifacts

The Prefix settings model section retains configuration-only model specifications
against the existing PostgreSQL provider revision. **Import local config and
tokenizer files** attaches original files to the displayed specification. Select
`config.json`, `tokenizer.json` and `tokenizer_config.json`; optional files are
`special_tokens_map.json`, `added_tokens.json`, `vocab.json`, `merges.txt` and
`chat_template.jinja` and `README.md`. Model weights and programs are not accepted here.

Original bytes are stored once in a content-addressed artifact set. Several model
specifications and prefix generations can reference that set without copying the
large tokenizer into each generation. The binding records the authenticated
operator. Changing provider/model settings does not rewrite historical bindings.

An import establishes retained bytes and integrity. It does not establish that the
provider serves those exact weights/tokenizer/template, accept a model design,
qualify recurrent/attention cache transfer or activate a prefix. A malformed or
incompatible tokenizer can remain an unverified original; execution must validate
its supported adapter separately. Original model specifications retain their
configuration-only qualification.

## Optional model documentation

Include the complete original `README.md` with the required model/tokenizer files
when the model's documentation is available. The existing importer preserves its
UTF-8 bytes, including BOM, CRLF and literal markup, and derives the file hash and
complete artifact-set identity. Changing documentation creates a different set;
it never rewrites an existing source or running job. The same total transport
limits apply, without heading/row cuts or a separate documentation store.

Pair construction proposals can cite that exact file through an evidence entry
with `file: "README.md"`, its retained `file_sha256` and `pointer: null`. It is
literal text, not JSON: JSON pointers are rejected. It cannot supply the selected
template, alter tokenizer execution or establish effective cache/serving facts.
The format-source inspector continues to select only the original template and
tokenizer files; the complete documentation remains available through the exact
artifact GET, outside the smaller inspection projection.

Import provenance identifies the operator who retained the file. It does not
verify its publisher, upstream URL/version, license or factual accuracy. Retain
and review those provenance facts before using a model card as authoritative
research. Documentation that describes an architecture remains distinct from
observations of the currently deployed provider and cache. This input supports
design authoring; it is not automatic model-design generation or acceptance.

## Retained format-source inspection

The same exact-artifact GET additionally returns optional `format_observation`,
with format `brassclaw-prefix-model-format-observation/1`. It binds specification,
artifact set and each selected original filename/hash/byte size. The existing
inspection action shows complete retained standalone and embedded/named chat
template sources plus explicit tokenizer token/declaration fields as escaped text.
Named alternatives are all retained; this reader does not guess which template
is the provider's effective default and never executes their contents. Each candidate
carries its original file identity, JSON pointer where applicable and the hash of
the exact decoded template's UTF-8 text; file hash and decoded-value hash are distinct.

`effective_template` and `serving_compatibility` remain null; qualification is
`retained-format-sources-only`. Missing versus null declarations remain distinct.
Duplicate/invalid/ambiguous input rejects the whole view rather than selecting a
fallback. The same metadata byte/node/depth inspection profile bounds the view;
capacity failure is visible and never clips the stored originals. Older responses
without this optional field remain readable with an explicit unavailable notice.
This is an evidence input for the saved construction specification, not a filled
or accepted model design, provider discovery or a serving/cache verdict.

Token IDs, external `added_tokens.json` mappings and added-token definitions/flags
remain explicit declarations; missing
and conflicting model/tokenizer declarations are not silently reconciled into
an effective framing rule.
The configuration projection also preserves declared `mamba_ssm_dtype`,
`mamba_conv_dtype`, `sliding_window` and `attention_chunk_size` where present,
without deriving effective recurrent/attention storage precision or allocator state.

## Design facts and serving boundary

Use the [pair-owned design contract](model-token-limits.md#prefix-selection-pair-design-and-assembly--binding-target-2026-10-26).
Imported templates/configuration are immutable evidence, never instructions to
install or change provider configuration. Supported provider metadata, official
versioned documentation or operator input can supply evidenced facts; unavailable
facts stay Unknown. Retrieval/import alone does not accept a format standard,
prove the effective endpoint mapping or authorize a cache/serving claim.

Generator and Kohai consume the same stored, accepted design revision for a
provider/model pair; Sempai consumes that revision too when that pair is its
provider/model. Role knowledge and finished role prefixes remain distinct.
Generation formats each prefix for its concrete pair and stores it immutably.
At runtime, Kohai formats only the dynamic Orchestrator prompt first. Optional
connected/enabled Sempai corrects that formatted prompt using its own generated
reviewer prefix/target-pair design and returns the final dynamic prompt. Kohai
then inserts the intended finished prefix unchanged exactly once before it,
without another formatting pass. Artifact inspection alone qualifies none of
these runtime handoffs.
Use the [explicit handoff acceptance](model-token-limits.md#explicit-handoff-between-the-two-model-requests)
to qualify design consumers: the returned final dynamic prompt is distinct from
the original and from Sempai's separately prefixed review request.
Manually editable input ceilings resolve to the canonical provider record and
endpoint in every settings view; artifact/design snapshots keep labelled historical
references, not independent editable copies. Capture/import does not activate a
role, regenerate a prefix or change a server template/service.

Model/default-template research must populate the pair design's
[construction specification](model-token-limits.md#formatting-design-must-contain-construction-instructions),
not just attach files or show a configuration summary. Retain exact originals and
connect supported template/tokenizer/config facts to concrete prefix/prompt/join
rules and validation cases. Imported architecture declarations are not effective
server cache geometry; manual/observed/researched provenance stays distinct.

## API

All routes use the existing instance-operator Bearer token and product facade.
Neither filesystem paths nor executable/compiler choices are request fields.

- `POST /api/webchat/v2/prefix-model-artifacts` accepts
  `{"specification":"<specification SHA-256>","artifact_set_bytes":"<JSON>"}`.
- `GET /api/webchat/v2/prefix-model-artifacts/{specification}/{artifact_set}`
  returns the exact canonical set and summary after checking every original file,
  the inventory and binding integrity.
- `GET /api/webchat/v2/prefix-model-designs` includes `artifact_sets` summaries
  under the existing model specifications. It does not send tokenizer contents in
  the ordinary catalogue response.

The browser upload format is:

```json
{
  "format": "brassclaw-prefix-model-artifact-import/1",
  "artifacts": {
    "config.json": {"encoding": "base64", "data": "<original file bytes>"},
    "tokenizer.json": {"encoding": "base64", "data": "<original file bytes>"},
    "tokenizer_config.json": {"encoding": "base64", "data": "<original file bytes>"}
  }
}
```

The retained format is `brassclaw-prefix-model-artifact-set/1`; each artifact also
contains its derived `sha256`. Uploading that retained format verifies the supplied
hashes. Both formats normalize to the same content identity for identical original
files, independent of property order or upload location. The HTTP envelope is
bounded to 64 MiB. The browser rejects a decoded set above 45 MiB before reading it;
no file is dropped to fit. An oversized set needs an explicit future transport
adapter, not clipping or a claim that the files were retained.

Repeated identical requests return the original binding. A conflicting binding
origin fails rather than replacing provenance. These records are immutable;
replacement means importing a new set and explicitly selecting it in a future
qualified build/design. Retention cannot supply its own approval or qualification
fields.

The default outer ingress ceiling is 64 MiB, matching this declared import route.
Chat, attachment and ordinary settings routes retain their smaller descriptor
limits. An explicit operator-configured smaller outer ceiling still applies and
can reject a large artifact import; it is never silently raised by this API.


## Retained configuration inspection

The existing exact-artifact GET response additionally contains
`configuration_observation`: a JSON observation tied to specification, artifact-set
and original `config.json` SHA-256. The WebUI's **Inspect retained model
configuration** action reads this response on demand. Ordinary catalogue lists do
not fetch tokenizers/configuration bodies. No model/provider request, additional
source store, acceptance or settings change is introduced.

`brassclaw-prefix-model-config-observation/1` projects declared top-level and
`text_config` model/layer/position/dtype fields and selected weight-quantization
fields by JSON pointer. Missing values stay absent; explicit null stays null.
No model-name inference, default layer pattern, derived context capacity or cache
layout is supplied. The full original remains unchanged. `weights_identity`,
`serving_context_limit`, `runtime_cache`, `attention_kv_cache` and `recurrent_state`
remain null: imported declarations do not identify a deployed server or weights.
This projection does not replace the immutable configuration-only model contract.

Inspection has its own finite 1 MiB / 16,384-node / depth-32 metadata contract.
Oversized or unsupported/invalid JSON is reported visibly without rejecting or
clipping the retained original. Ambiguous duplicate-key JSON is not interpreted.
Borrowed package strings avoid copying tokenizer contents to inspect a small
configuration. UI identity checks reject a response for another model/artifact;
late responses and unmounted views cannot replace the current observation.
The view displays quoted JSON text and never executes imported fields.

## Shared pair construction drafts

V180 extends the existing immutable `reborn_prefix_design_proposals` owner. A
`brassclaw-prefix-pair-construction-proposal/1` has no profile; it is bound to one
provider/model specification and its exact retained artifact set. Historical
profile-specific v1/v2 proposals stay separate and readable. The existing
per-profile acceptance foreign key cannot accept a NULL-profile pair draft.
No new settings table, active-design pointer or acceptance writer is introduced.

The same instance-operator API family provides
`POST /api/webchat/v2/prefix-model-designs/proposals` with
`{"specification":"<specification SHA-256>","proposal_bytes":"<exact JSON text>"}`.
The Model designs section imports that JSON file and displays it under its captured
specification. The catalogue returns these records as `pair_proposals`, separate
from per-profile `proposals`. Identical original bytes return the same record;
changed bytes create a different immutable draft. They do not change model limits,
provider roles, existing proposals or generations. Generate model design stays
unavailable pending its qualified Recipe and consumers.

Every draft field is required; unknown fields are rejected. Use these exact fields:

| Field | Contract |
| --- | --- |
| `format` | `brassclaw-prefix-pair-construction-proposal/1`. |
| `specification`, `artifact_set` | Exact retained identities for this same namespace/pair revision. |
| `rationale` | Nonempty proposed design reasoning, not approval evidence. |
| `evidence` | Nonempty array of `{id,file,file_sha256,pointer}`. IDs are distinct ASCII letters/digits/underscore/hyphen. Allowed original files are config, tokenizer config, standalone template and designated special/added-token maps. `pointer` is explicit null for original-file identity, or an existing JSON pointer for a JSON declaration. It is not permission to copy an entire original into model context. |
| `template` | `{evidence,template_sha256}` selects an exact observed standalone/embedded/named source using that evidence's file and pointer. The template hash is of decoded UTF-8 text; `file_sha256` identifies its original container. No default or serving identity is inferred. |
| `construction` | The separate instruction blocks, modes and fixed join described below. |
| `sample_reference` | Nonempty fixed reference used consistently across the worked cases; this is test data, not a role prefix generation. |
| `worked_cases` | Cases described below, covering every declared supported mode. |

`construction` has exactly `prefix`, `dynamic_prompt`, `special_tokens`, `memory`,
`supported_modes`, `unsupported_modes`, `join`, `hard_limits_owner` and
`runtime_cache`. The three instruction blocks `dynamic_prompt`, `special_tokens`
and `memory` contain `{instructions,evidence}`: nonempty concrete instructions and
nonempty distinct evidence-ID references. `prefix` additionally contains
`leading_literal`, `trailing_literal` and `edge_requirement`. Its edge requirement
is `preserve-exact` or `non-whitespace-edges`; validation rejects a noncanonical
sample rather than trimming it. Literal framing and instruction text remain data;
no template or generated program executes during import.

`supported_modes` is a nonempty distinct list of mode IDs. `unsupported_modes` maps
disjoint mode IDs to nonempty reasons; no unlisted mode becomes supported by
omission. `join` must be `prepend-fixed-prefix`. `hard_limits_owner` must be
`provider-definition-revision`: there are no editable prefix/prompt token values
in this document. The captured specification references historical canonical
policy; current ceilings still come from the provider owner before real use.
`runtime_cache` must be explicit null in this retained-source draft format. Cache
architecture declarations may inform proposed instructions, but cannot become
trusted deployed geometry or precision without a future qualified evidence path.

Each worked case contains exactly `id`, `mode`, `render_context_json`,
`expected_fixed_prefix`, `formatted_prompt`, `expected_request` and `evidence`.
Case IDs are distinct; modes refer to declared supported modes. Context is original
ordered JSON-object text, parsed for syntax without execution or reserialization.
For every case, validation checks these exact string equations:

```text
expected_fixed_prefix = leading_literal + sample_reference + trailing_literal
expected_request = expected_fixed_prefix + formatted_prompt
```

This rejects a changed prefix, a contradictory expected join, dangling evidence,
missing mode coverage, copied settings, forged original/template hashes and
foreign source bindings. It **does not** prove that the proposed literals/prompt
match the provider default template, that a mode actually works, that tokenization
is correct or that the full request fits. Those are independent qualification and
acceptance gates. The retained qualification always remains
`unaccepted-pair-construction-proposal`; supplied success/approval/code fields do
not change it and are rejected. Do not activate or consume it as an accepted design.

The existing 1 MiB proposal-route envelope and bounded strict JSON parser reject
oversized input as a whole; JSON-string transport overhead counts too. The UI
also rejects an original draft file above 1 MiB before reading it and decodes UTF-8
strictly, without replacement characters or silent BOM stripping. Neither layer
shortens content. Keep credentials and private conversations out of examples.

## Complete worked-case projection and comparison candidates

`GET /api/webchat/v2/prefix-model-designs/proposals/{specification}/{proposal}/cases`
uses the same instance-operator facade/store. It rechecks the exact retained
NULL-profile proposal, namespace, artifact originals and template/evidence binding,
then projects **all** cases without rendering or storing another copy. Historical
role proposals and foreign namespaces cannot use this reader. The response binds
`proposal`, `specification`, `artifact_set`, canonical `case_packet_bytes` and
`qualification=unqualified-case-input`. Identical reads are identical; an oversized
whole projection fails rather than dropping a case or altering the original draft.

The canonical packet format is `brassclaw-prefix-pair-case-packet/1`. Its fields are
`format`, `proposal`, `specification`, `artifact_set`, `template_sha256`, `cases`
and `qualification=unqualified-case-input`. Every case has exactly `id`, `mode`,
`context_json`, `expected_prefix` and `expected_render`. Original nested context
order/text and expected literal bytes remain unchanged. Context must describe the
complete intended model render, including the fixed sample system content and
actual generation/thinking/tools flags. A structurally consistent draft can still
have an incorrect context or expected render; this projection does not fix it.

The reusable **candidate** Recipe `compare-prefix-pair-default-template-cases`
prepares every case, reuses the existing UTF-8 digest usage, reuses the existing
**original-template** encoding usage, then compares actual complete rendered bytes
and token-ID hashes with the expected full text. Its two independent primitive
calls have separate binding/execution steps. No `template_utf8`, injection adapter,
Jinja rewrite, fallback, new provider pool or native workflow Tool is involved.
The leading-chat primitive verifies the exact prefix at byte zero and attributes
actual token offsets; the comparison does not infer the boundary from an LCP or
add independently tokenized prefix/suffix counts.

The output format is `brassclaw-prefix-pair-case-comparison/1`, with individual
case matches, actual prefix/prompt/total token counts, measured prefix-token
identity stability and source/worker/preparation identities. Qualification remains
`retained-template-cases-only`; `provider_wire_qualified=false`, `context_fit=null`
and cache/model quality stay unmeasured. A wrong dynamic literal can produce a
completed comparison with `all_cases_match=false`. A missing leading prefix,
incorrect original template, invalid resource/identity/partition data or unsupported
batch fails the operation. Nothing is truncated to pass an existing primitive's
acknowledged batch/resource limits.

The PythonCode stages and Recipe constructors are not an installed/approved
workflow, ordinary chat command, durable public measurement job or accepted design.
They require the existing installation-owned workflow qualification/admission and
native-journal verification before those uses. Never accept a caller-supplied
comparison JSON or a standalone pure evaluation as native provenance. Subsequent
acceptance must bind the exact source-owned packet, full retained workflow/leaf
approvals and actual primitive journals. Provider/API-to-model mapping, a usable
machine-readable formatter, full-prefix/current-policy capacity, cache restoration
and held-out answer quality remain separate gates.
