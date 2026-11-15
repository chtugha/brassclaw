# 04 — Instruction building and retained composition

## 1. Purpose

IBS selects a Recipe's structured steps and preserves their order. The surrounding
catalogue/composition path resolves exact component revisions, typed input/result
edges, usage associations, exports and native implementations. Monty executes the
selected workflow; assembly itself performs no Tool operation. Follow
[recipe.md](../../recipe.md) for authoring and
[toolskills.md](../../toolskills.md) for binding semantics. This source map was
checked on 2026-11-15.

## 2. Location — verified sources

| Source | Symbols / role |
| --- | --- |
| [instruction_builder.rs](../../crates/brassclaw_engine/src/memory/instruction_builder.rs) | `StepDescriptionEntry`, `StepOwner`, `RecipeStepType`, `OrderedBuildInstruction`, `build_ordered_instruction` and compatibility `build_instruction`. |
| [retained_instruction.rs](../../crates/brassclaw_engine/src/memory/retained_instruction.rs) | `compile_retained_recipe`, `compile_matched_retained_recipe` and `RetainedRecipeInstruction` retain selected immutable graph/variant/order. |
| [retained_inputs.rs](../../crates/brassclaw_engine/src/memory/retained_inputs.rs) | `prepare_retained_inputs` reads exact layouts/contracts and checks typed edges. |
| [composition.rs](../../crates/brassclaw_engine/src/memory/composition.rs) | `compose_typed_program` prepares typed execution structure; legacy `compose_program` performs source substitution. |
| [retained_tools.rs](../../crates/brassclaw_engine/src/memory/retained_tools.rs) | `prepare_retained_tool_program` prepares matching binding/executable pairs and exact usage associations. |
| [retained_preload.rs](../../crates/brassclaw_engine/src/executor/retained_preload.rs) | `PreloadDeclaration` and `ExportDeclaration` parse `python-preload/2`. |
| [retained_source.rs](../../crates/brassclaw_engine/src/executor/retained_source.rs) | `InspectedRetainedProgram` checks source/declarations and selects invocations. |
| [global_recipe_ports.rs](../../crates/brassclaw_reborn_composition/src/global_recipe_ports.rs) | Task-owned composition/execution ports use the captured selection. |

These are implemented retained interfaces. The old SQL constructors/composer do
not supply their complete contract, but that does not mean ordinary retained
typed inputs or association checks are absent.

## 3. Persisted steps and ordering

The persisted schema is `StepDescriptionEntry` with `desc_idx`, label/display
metadata and structured `steps`. Each step uses `stepnumber` and `knowledge`;
`channel` and `step_id` are explanatory names, not substitute authoring fields.
IBS reads structured steps rather than `yaml_source`, goals or prose.

- `knowledge` admits `rust`, `orchestrator`, `both`. Normal Tool use has a separate
  Rust ToolSkill step immediately followed by its matching PythonCode step.
  `both` cannot bypass class/channel restrictions or binding pairing.
- `type` admits `text`, `component`, `snippet`. Text is annotation; an unpromoted
  snippet is rejected. A prose LLM instruction or invented `llm` type does not
  execute a model call.
- Every component step has exactly one stable UUID in `include`. Internal
  PythonCode dependencies form their own declared graph.
- `desc_idx` is zero-based; step ordinals are positive and strictly increasing.
  `step_link` selects ranges such as `0:1-0:E`. The ordered builder retains the
  selected interleaving as well as the channel partitions.

Use the [complete schema/example](../../recipe.md#7-emit-the-actual-persisted-ibs-schema)
for a Recipe document. Bindings, recursive contracts and exact component metadata
must agree; a syntactically valid range is not approval or execution evidence.

### 3.1 Typed input bindings

`input_layouts` is a per-variant map in the immutable Recipe document. The following
is a **partial design fragment**, not an insert request, approved Recipe or claim
that its illustrative variant has been published:

```json
{
  "input_layouts": {
    "reply-from-input": {
      "format": "recipe-input-layout/1",
      "task_inputs": {
        "reply_text": {"type": "string", "required": true, "checks": []}
      },
      "steps": {
        "0:2": {
          "answer": {"kind": "task_input", "reference": "{{vars.reply_text}}"}
        }
      }
    }
  },
  "invocations": {
    "reply-from-input": {"0:2": "publish_turn_reply"}
  }
}
```

Here `0:2` denotes the matching executable reply step in a separately complete
Recipe. `reply_text` must also agree with its declared capture/admission and the
selected usage's checks. `publish_turn_reply` is a real mapping export in
[global_bootstrap_components.rs](../../crates/brassclaw_skills/src/global_bootstrap_components.rs);
that source declares the required string `answer` and calls
`host.post_reply(answer=inputs['answer'])`. Empty/invalid concrete replies still
fail the actual adapter; the fragment grants no permission or workflow approval.

The exact supported input binding shapes are:

| Source | Record |
| --- | --- |
| Task capture/input | `{"kind":"task_input","reference":"{{vars.reply_text}}"}` |
| Constant | `{"kind":"constant","value":"Ready."}` |
| Declared consumer default | `{"kind":"consumer_default"}` |
| Prior result field | `{"kind":"result","step_id":"0:2","path":["field"]}` |

An empty result path selects the complete prior result. The ordered layout does
not support forward references, list indexing or unguarded optional-field paths.
`steps` must cover the selected executable steps and their local inputs. All
references bind whole typed values; runtime text is never reinterpreted as a
reference or pasted into Python. Defaults apply only to missing inputs, not null,
invalid inputs or outputs. See the
[retained input contract](../reborn/contracts/retained-component-inputs.md).

### 3.2 Invocation selection and preload

`invocations[variant_key][step_id]` selects the export name. The actual retained
source preparation uses that selection or the code's declared `default_export`.
If neither supplies a valid export, preparation fails. Never infer one from
prose, array order or matching names. The declaration, implementation symbol,
parameter mapping and usage contracts must agree.

Load definitions in deterministic dependency-first order with stable-UUID
tie-breaking, rejecting cycles/conflicts/effectful initializers. Preloading is
not an invocation or an extra effect step. Mutable task/attempt/invocation state
stays isolated. See [Skills](05-skills-system.md) and [PythonCode](07-pythoncode-system.md).

## 4. Binding, execution and failure

Retained Tool preparation reads `tool-skill-binding/1`, checks the Tool dependency,
callable/capability identity and unique matching Skill/code association. It does
not load an implementation, dispatch or grant permission. Catalogue preparation
separately checks required usage evidence and implementation availability. The
actual host then enforces current global policy and execution freshness.

The ordinary retained runner admits one bound Tool dispatch per executable step
and `stop` with `max_attempts: 1`. Its binding metadata requires empty `params`
and `error_policy: {"policy":"fail"}`; executable Python supplies actual arguments.
Legacy `ErrorPolicy::Ignore`, `Retry` and `Fallback` are compatibility type values,
not supported retained recovery. In particular, ignore cannot turn an error into
an invented empty success, and fallback cannot replay a completed effect.

The broader [failure/retry contract](../../skills.md#exact-failure-and-retry-contract)
requires durable logical invocation counts, explicit retryable outcomes and
verified read-only/deduplication evidence before retries can be activated. A
lost response does not prove no effect. Do not emulate retries inside exports,
new child tasks or replacement steps. Only true No-Match enters Tier 2;
composition unavailability, invalid selection and begun failures remain errors.

## 5. Legacy composition boundary

`PgCompositionPort` in
[pg_composition_port.rs](../../crates/brassclaw_reborn_composition/src/pg_composition_port.rs)
and `compose_program` describe the older source-substitution path. Its
`ComposedProgram` fields, body selection and `rust_directives` do not establish
the retained graph above. Directives derive from explicit bindings, not
necessarily every ToolSkill include; an artifact path or capability name alone
proves neither registration nor immutable implementation identity.

Ordinary global tasks use retained selection and pinned exports. Do not promote
legacy raw compose/run to the future primary driver. Do not preserve an old
fallback-to-Non-Matching prescription when the selected composition fails.

## 6. Support and remaining work

Ordered IBS, retained input layouts, exact binding/association preparation and
preload/export selection exist. General pure-function approval/publication,
broader successor compatibility readers, qualified dynamic occurrences and
durable recovery retain their separate owners and acceptance. V35-01/02 reconcile
guidance, V35-03/04 consume pure approval, V35-05/06 qualify reference reuse and
V35-13/14 own remaining occurrence work. Use the
[implementation index](../implementation-status.md) for actual accepted scope.

## 7. LLM-relevant summary

IBS preserves the Recipe's selected order and exact graph. Typed layouts supply
local inputs; retained declarations select valid exports, with an explicit
invocation or declared default. Loading is effect-free. Monty invokes the pinned
code; Rust checks binding membership, live authority and effect custody. The
ordinary retained runner is single-call and stop-only. Missing support or
execution errors never authorize downgrade, No-Match fallback or effect replay.
