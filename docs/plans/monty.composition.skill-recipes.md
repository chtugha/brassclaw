# monty.composition — optional MCP-call and reference Recipe instructions

Implementation update (2026-10-09): the public reply successor uses
`publish literal reply %` and preserves the original reply command aliases.
After section 31's exact trusted bootstrap approval/behavior gate, section 32
activates the selected successor coherently with approval IDs/checksums pinned
in the Recipe manifest and invocation journal. Routing and prefix data share that
catalogue generation. Advertising remains empty until full ordinary-command
qualification. Section 33 now implements that qualification and publication from
complete durable ordinary-chat observations; MCP chat/provider acceptance remains
required. History stays private. See sections 30–33 of `monty.composition.md`; this does not waive the
migration review gate below.

## Reply/history migration review gate

The qualified retained reply/history successors and shared preload/export adapter
are the current baseline, as recorded in the [main plan](monty.composition.md#replyhistory-migration-review-gate)
and [implementation status](../implementation-status.md). Reuse their exact
accepted scope; do not rebuild the adapter or remigrate qualified successors
because an older draft called them unqualified. Broader library/public-command
population and general pure/recovery acceptance remain open.

For a changed successor, inspect its actual interfaces, bindings, full graph,
typed edges and consuming callers; preserve exact old selections, stable IDs,
overrides and completed effects. Qualify only the changed closure through the
supported authored or installation-owned path. Internal reply/history remains
private unless a separate qualified public contract permits exposure. Public
formatting/MCP delivery must never add a second reply or history effect.

The [concurrent ownership allocation](monty.composition.md#concurrent-implementation-with-simplified-v3)
and [simplified library integration contract](../../simplified_v3.md#gemeinsame-runtime-verträge-für-die-komponentenmigration)
apply to every entry below. Composition owns the component/variant/command and
MCP protocol wiring. Shared loading, typed transport, child/chat lifecycle,
provider identity, boot, settlement and upgrade support are implemented once
in simplified's existing owners; validator retains approval/evidence semantics.
Missing shared support stays there rather than becoming an appendix-local
adapter, queue, resource controller, store or executor. Reuse unchanged qualified
shared evidence and add only actual consumer/command cases still missing.

**MCP discovery/provider lifecycle:** derive tools/list from available approved mcp-call-skill-recipes and their exact command variants. The server remains running; Kohai connects/advertises only after final prefix addition immediately before prompt send and disconnects on the complete model answer. Refresh the server list at startup/restart and qualified Skill/Recipe catalogue changes. Preserve request-local advertised contracts and ordinary-chat result recovery; see [the authoritative lifecycle](monty.composition.md#recipe-derived-discovery-and-kohai-owned-provider-connections).

Model usages consume the Prefix owner's
[four binding assembly rules](../reborn/model-token-limits.md#prefix-selection-pair-design-and-assembly--binding-target-2026-10-26),
including the generator's same saved accepted pair design, Kohai dynamic formatting
before optional Sempai correction, Sempai's own generated reviewer prefix/design,
and exactly one unchanged insertion of the finished prefix after the final dynamic
prompt returns. No Skill-specific second formatter is introduced after review.

Plans only. This appendix covers **98 Skill execution Recipe roles**: 90 legacy companion usages and eight explicitly associated newer/planned usages. The two retired pure wrappers and pure N19 are not Skills and receive no artificial execution Recipes. Role labels do not allocate UUIDs; equivalent existing Recipes/variants are reused after exact contract review. Additional future real Skills must receive the same companion Recipe contract before MCP exposure.

The 98 roles remain a migration/reuse inventory, not 98 prerequisites for Skill
existence or completeness. Classify each as intended MCP publication, ordinary
consumer, contained test/reference or private internal invocation. A role can serve
more than one purpose only with the corresponding separate evidence. Do not delete
qualified private/reference Recipes or activate draft MCP entries to satisfy a
blanket companion rule. Existing SKR anchors remain stable for linked plans.

## Command and variable-placement contract

These 98 roles are candidate mcp-call-skill-recipes or compatible reusable variants. Only their qualified activated records produce listing entries. MCP tools/list must derive from those records and give the model the exact command sentence, every variable position/name/type, required/default/null rules, encoding and examples for the selected activated Skill/Recipe pair. It must not send Python authoring instructions. The model completes that sentence and sends the command to MCP. MCP opens a fresh ordinary chat and submits the command as one user message. Normal ingress/matching/IBS/Recipe execution posts the result in that chat; MCP forwards its correlated terminal response and closes the chat. No direct MCP execution connection is added.

For these draft designs the exact single-slot sentence is `run <listed-skill-name> with %`. The `%` is Recipe intent-template metadata: replace it with one JSON object containing that Skill's declared inputs. It is not sent literally and is never Python substitution. This single whole-value argument slot gives exact variable placement while avoiding ambiguous separators between arbitrary string parameters. For example, the file-read command is `run skill-read-file with {"path":"notes.txt"}`. Each input name/type/default/constraint is supplied by the exact linked finite usage contract; tools/list must list those concrete fields and real valid examples, not an unrestricted JSON bag.

If this proposed JSON-object sentence is selected, qualify its capture/parameter decoding inside the existing Recipe/input preparation path into the finite usage layout, rejecting duplicate keys, malformed/nonfinite/oversized/deep values, extra fields, wrong types and invalid selectors before effects. This is normal Recipe parameter preparation, not an MCP-specific input engine or component approval. Prefer an existing execution Recipe's already qualified sentence/variable layout when it fits; the JSON sentence is a draft alternative, not a requirement to change chat ingress. Strings may contain quotes, newlines or source-like text and stay data; command construction uses JSON encoding, never Python source generation. Do not claim the current positional capture automatically implements this object-to-local-input mapping. A more natural multi-slot sentence is permitted only as a separately qualified equivalent variant with exact escaping, capture and typed conversion rules; do not silently accept arbitrary wording.

The `<listed-skill-name>` is a unique admitted label mapped to canonical Skill/Recipe UUIDs in the same approved generation. Enforce the selected tools/call name's expected Skill/Recipe contract so a command for another listed usage cannot execute under that name. Matching and assembly share one snapshot and exact pinned exports. The normal chat matcher owns routing: ambiguity/store failures stay errors, and only actual No-Match can enter its existing Tier2 path. Listed commands must qualify as matches; a fallback answer is not proof the requested Skill executed. No MCP-specific fallback or matching branch is added. Discovery/call updates must reject incompatible old command contracts before effects or explicitly retain a compatible mapping.

## Shared Recipe authoring requirements

Each role below identifies a proposed MCP-call or reference workflow, not a required owner of its Skill. Qualify an unambiguous Recipe/variant for each intended MCP publication; retain private/test workflows for their actual purpose. There is no global one-Recipe-per-Skill cardinality rule. First inspect the existing Recipe inventory for the same one-usage inputs/results/effects; reuse or qualify a compatible existing variant. A task Recipe with additional writes/replies is not automatically equivalent. Keep old task selections and artifacts. Recipes carry stable component UUIDs without versions. The retained input layout maps decoded command inputs to `inputs["local_name"]` or explicit exported keyword parameters.

Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection) to every role and invocation example below. Resolve actual declared exports and exact usage contracts before filling a skeleton; a proposed public name does not install a callable. The non-runnable skeletons retain unresolved binding arrays: fill each Rust step with one exact ToolBinding record, empty params and fail policy as in [the worked composition example](monty.composition.md#102-concrete-composition-example-read-and-select-literal-lines). Replacing UUID markers alone does not complete binding preparation.

Preload the complete selected function/helper/immutable-constant graph in dependency-first order without effects. The minimal usage sequence is one Rust ToolSkill component step followed immediately by one class-22 invocation component step; each step includes exactly one UUID. Optional pure input preparation or public-result formatting uses its own canonical component step. Select the export through a supported retained interface; invent no persisted Recipe fields. The usage result feeds the Recipe's selected normal public formatter/reply owner; ordinary chat history ownership remains in the existing chat path. MCP reads that posted response and forwards it without another formatter, reply or history write. <!-- Withdrawn: Reuse the existing reply/history ownership rather than append duplicate N01/N02 calls. --> Preserve one reply/history owner, but independently review and migrate its actual Recipe and usages under the migration review gate before reuse; do not append duplicate publication/history effects. If the Skill itself is a reply/history operation, explicitly qualify one output/history owner for that Recipe to avoid a second effect.

Expose only qualified public usages. Protected root/candidate/evidence/admin/installation-owned internal usages require their existing internal admitted contract and are not automatically listed merely because this appendix gives their Recipe role. Shell and subagent usages remain Tier1. Missing adapters, unsafe fixed selectors, absent control/conditional guarantees or incompatible results keep a role BLOCKED. The preloadable interface, typed capture/layout, exact association approval and complete retained closure need real production-path acceptance; skeletons below are not insertable records.

## SKR001. skill-read-file execution Recipe

**Existing Recipe overlap findings:** Inspect [file-read](monty.composition.recipe-implementation.md#rcp001-file-read), [file-write](monty.composition.recipe-implementation.md#rcp003-file-write), [file-read-tail](monty.composition.recipe-implementation.md#rcp024-file-read-tail), [file-read-and-grep](monty.composition.recipe-implementation.md#rcp026-file-read-and-grep) and other consumers of the same canonical component. These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-read-file` executes this Skill's one-Tool usage: Use the host boundary to read a file via builtin.read_file. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l001-pc-exec-read-file).

**Exact command:** `run skill-read-file with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-read-file:** Candidate implementation `usage_l001(inputs)` (role L001); proposed public export `use_skill_read_file(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_read_file(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_read_file(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l001(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-read-file",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-read-file",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR002. skill-write-file-new execution Recipe

**Existing Recipe overlap findings:** Inspect [file-write](monty.composition.recipe-implementation.md#rcp003-file-write), [file-write-template](monty.composition.recipe-implementation.md#rcp004-file-write-template). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-write-file-new` executes this Skill's one-Tool usage: Use the host boundary to write a file via builtin.write_file. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l002-pc-exec-write-file).

**Exact command:** `run skill-write-file-new with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-write-file-new:** Candidate implementation `usage_l002(inputs)` (role L002); proposed public export `use_skill_write_file_new(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_write_file_new(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_write_file_new(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l002(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-write-file-new",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-write-file-new",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR003. skill-write-file-template execution Recipe

**Existing Recipe overlap findings:** Inspect [file-write](monty.composition.recipe-implementation.md#rcp003-file-write), [file-write-template](monty.composition.recipe-implementation.md#rcp004-file-write-template). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-write-file-template` executes this Skill's one-Tool usage: Use the host boundary to write a file via builtin.write_file. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l002-pc-exec-write-file).

**Exact command:** `run skill-write-file-template with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-write-file-template:** Candidate implementation `usage_l002(inputs)` (role L002); proposed public export `use_skill_write_file_template(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_write_file_template(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_write_file_template(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l002(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-write-file-template",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-write-file-template",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR004. skill-write-file-replace execution Recipe

**Existing Recipe overlap findings:** Inspect [file-write](monty.composition.recipe-implementation.md#rcp003-file-write), [file-write-template](monty.composition.recipe-implementation.md#rcp004-file-write-template). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-write-file-replace` executes this Skill's one-Tool usage: Use the host boundary to write a file via builtin.write_file. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l002-pc-exec-write-file).

**Exact command:** `run skill-write-file-replace with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-write-file-replace:** Candidate implementation `usage_l002(inputs)` (role L002); proposed public export `use_skill_write_file_replace(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_write_file_replace(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_write_file_replace(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l002(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-write-file-replace",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-write-file-replace",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR005. skill-list-dir execution Recipe

**Existing Recipe overlap findings:** Inspect [file-list](monty.composition.recipe-implementation.md#rcp005-file-list), [file-list-recursive](monty.composition.recipe-implementation.md#rcp006-file-list-recursive), [file-list-files-only](monty.composition.recipe-implementation.md#rcp021-file-list-files-only), [file-list-dirs-only](monty.composition.recipe-implementation.md#rcp022-file-list-dirs-only) and other consumers of the same canonical component. These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-list-dir` executes this Skill's one-Tool usage: Use the host boundary to list a directory via builtin.list_dir. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l003-pc-exec-list-dir).

**Exact command:** `run skill-list-dir with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-list-dir:** Candidate implementation `usage_l003(inputs)` (role L003); proposed public export `use_skill_list_dir(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_list_dir(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_list_dir(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l003(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-list-dir",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-list-dir",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR006. skill-list-dir-recursive execution Recipe

**Existing Recipe overlap findings:** Inspect [file-list](monty.composition.recipe-implementation.md#rcp005-file-list), [file-list-recursive](monty.composition.recipe-implementation.md#rcp006-file-list-recursive), [file-list-files-only](monty.composition.recipe-implementation.md#rcp021-file-list-files-only), [file-list-dirs-only](monty.composition.recipe-implementation.md#rcp022-file-list-dirs-only) and other consumers of the same canonical component. These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-list-dir-recursive` executes this Skill's one-Tool usage: Use the host boundary to list a directory via builtin.list_dir. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l003-pc-exec-list-dir).

**Exact command:** `run skill-list-dir-recursive with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-list-dir-recursive:** Candidate implementation `usage_l003(inputs)` (role L003); proposed public export `use_skill_list_dir_recursive(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_list_dir_recursive(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_list_dir_recursive(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l003(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-list-dir-recursive",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-list-dir-recursive",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR007. skill-glob-by-extension execution Recipe

**Existing Recipe overlap findings:** Inspect [file-glob](monty.composition.recipe-implementation.md#rcp007-file-glob), [file-glob-by-extension](monty.composition.recipe-implementation.md#rcp008-file-glob-by-extension), [file-glob-by-name](monty.composition.recipe-implementation.md#rcp009-file-glob-by-name), [file-glob-in-subdir](monty.composition.recipe-implementation.md#rcp010-file-glob-in-subdir) and other consumers of the same canonical component. These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-glob-by-extension` executes this Skill's one-Tool usage: Use the host boundary to find files via builtin.glob. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l004-pc-exec-glob).

**Exact command:** `run skill-glob-by-extension with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-glob-by-extension:** Candidate implementation `usage_l004(inputs)` (role L004); proposed public export `use_skill_glob_by_extension(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_glob_by_extension(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_glob_by_extension(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l004(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-glob-by-extension",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-glob-by-extension",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR008. skill-glob-by-name execution Recipe

**Existing Recipe overlap findings:** Inspect [file-glob](monty.composition.recipe-implementation.md#rcp007-file-glob), [file-glob-by-extension](monty.composition.recipe-implementation.md#rcp008-file-glob-by-extension), [file-glob-by-name](monty.composition.recipe-implementation.md#rcp009-file-glob-by-name), [file-glob-in-subdir](monty.composition.recipe-implementation.md#rcp010-file-glob-in-subdir) and other consumers of the same canonical component. These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-glob-by-name` executes this Skill's one-Tool usage: Use the host boundary to find files via builtin.glob. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l004-pc-exec-glob).

**Exact command:** `run skill-glob-by-name with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-glob-by-name:** Candidate implementation `usage_l004(inputs)` (role L004); proposed public export `use_skill_glob_by_name(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_glob_by_name(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_glob_by_name(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l004(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-glob-by-name",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-glob-by-name",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR009. skill-glob-in-subdir execution Recipe

**Existing Recipe overlap findings:** Inspect [file-glob](monty.composition.recipe-implementation.md#rcp007-file-glob), [file-glob-by-extension](monty.composition.recipe-implementation.md#rcp008-file-glob-by-extension), [file-glob-by-name](monty.composition.recipe-implementation.md#rcp009-file-glob-by-name), [file-glob-in-subdir](monty.composition.recipe-implementation.md#rcp010-file-glob-in-subdir) and other consumers of the same canonical component. These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-glob-in-subdir` executes this Skill's one-Tool usage: Use the host boundary to find files via builtin.glob. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l004-pc-exec-glob).

**Exact command:** `run skill-glob-in-subdir with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-glob-in-subdir:** Candidate implementation `usage_l004(inputs)` (role L004); proposed public export `use_skill_glob_in_subdir(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_glob_in_subdir(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_glob_in_subdir(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l004(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-glob-in-subdir",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-glob-in-subdir",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR010. skill-grep-files execution Recipe

**Existing Recipe overlap findings:** Inspect [file-grep](monty.composition.recipe-implementation.md#rcp012-file-grep), [file-grep-files](monty.composition.recipe-implementation.md#rcp013-file-grep-files), [file-grep-content](monty.composition.recipe-implementation.md#rcp014-file-grep-content), [file-grep-count](monty.composition.recipe-implementation.md#rcp015-file-grep-count) and other consumers of the same canonical component. These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-grep-files` executes this Skill's one-Tool usage: Use the host boundary to search content via builtin.grep. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l005-pc-exec-grep).

**Exact command:** `run skill-grep-files with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-grep-files:** Candidate implementation `usage_l005(inputs)` (role L005); proposed public export `use_skill_grep_files(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_grep_files(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_grep_files(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l005(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-grep-files",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-grep-files",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR011. skill-grep-content execution Recipe

**Existing Recipe overlap findings:** Inspect [file-grep](monty.composition.recipe-implementation.md#rcp012-file-grep), [file-grep-files](monty.composition.recipe-implementation.md#rcp013-file-grep-files), [file-grep-content](monty.composition.recipe-implementation.md#rcp014-file-grep-content), [file-grep-count](monty.composition.recipe-implementation.md#rcp015-file-grep-count) and other consumers of the same canonical component. These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-grep-content` executes this Skill's one-Tool usage: Use the host boundary to search content via builtin.grep. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l005-pc-exec-grep).

**Exact command:** `run skill-grep-content with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-grep-content:** Candidate implementation `usage_l005(inputs)` (role L005); proposed public export `use_skill_grep_content(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_grep_content(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_grep_content(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l005(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-grep-content",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-grep-content",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR012. skill-grep-count execution Recipe

**Existing Recipe overlap findings:** Inspect [file-grep](monty.composition.recipe-implementation.md#rcp012-file-grep), [file-grep-files](monty.composition.recipe-implementation.md#rcp013-file-grep-files), [file-grep-content](monty.composition.recipe-implementation.md#rcp014-file-grep-content), [file-grep-count](monty.composition.recipe-implementation.md#rcp015-file-grep-count) and other consumers of the same canonical component. These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-grep-count` executes this Skill's one-Tool usage: Use the host boundary to search content via builtin.grep. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l005-pc-exec-grep).

**Exact command:** `run skill-grep-count with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-grep-count:** Candidate implementation `usage_l005(inputs)` (role L005); proposed public export `use_skill_grep_count(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_grep_count(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_grep_count(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l005(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-grep-count",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-grep-count",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR013. skill-grep-case-insensitive execution Recipe

**Existing Recipe overlap findings:** Inspect [file-grep](monty.composition.recipe-implementation.md#rcp012-file-grep), [file-grep-files](monty.composition.recipe-implementation.md#rcp013-file-grep-files), [file-grep-content](monty.composition.recipe-implementation.md#rcp014-file-grep-content), [file-grep-count](monty.composition.recipe-implementation.md#rcp015-file-grep-count) and other consumers of the same canonical component. These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-grep-case-insensitive` executes this Skill's one-Tool usage: Use the host boundary to search content via builtin.grep. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l005-pc-exec-grep).

**Exact command:** `run skill-grep-case-insensitive with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-grep-case-insensitive:** Candidate implementation `usage_l005(inputs)` (role L005); proposed public export `use_skill_grep_case_insensitive(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_grep_case_insensitive(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_grep_case_insensitive(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l005(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-grep-case-insensitive",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-grep-case-insensitive",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR014. skill-grep-type-filtered execution Recipe

**Existing Recipe overlap findings:** Inspect [file-grep](monty.composition.recipe-implementation.md#rcp012-file-grep), [file-grep-files](monty.composition.recipe-implementation.md#rcp013-file-grep-files), [file-grep-content](monty.composition.recipe-implementation.md#rcp014-file-grep-content), [file-grep-count](monty.composition.recipe-implementation.md#rcp015-file-grep-count) and other consumers of the same canonical component. These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-grep-type-filtered` executes this Skill's one-Tool usage: Use the host boundary to search content via builtin.grep. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l005-pc-exec-grep).

**Exact command:** `run skill-grep-type-filtered with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-grep-type-filtered:** Candidate implementation `usage_l005(inputs)` (role L005); proposed public export `use_skill_grep_type_filtered(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_grep_type_filtered(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_grep_type_filtered(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l005(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-grep-type-filtered",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-grep-type-filtered",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR015. skill-apply-patch-single execution Recipe

**Existing Recipe overlap findings:** Inspect [file-patch](monty.composition.recipe-implementation.md#rcp016-file-patch), [file-patch-replace-all](monty.composition.recipe-implementation.md#rcp017-file-patch-replace-all). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-apply-patch-single` executes this Skill's one-Tool usage: Use the host boundary to apply a targeted patch via builtin.apply_patch. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l006-pc-exec-apply-patch).

**Exact command:** `run skill-apply-patch-single with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-apply-patch-single:** Candidate implementation `usage_l006(inputs)` (role L006); proposed public export `use_skill_apply_patch_single(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_apply_patch_single(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_apply_patch_single(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l006(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-apply-patch-single",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-apply-patch-single",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR016. skill-apply-patch-all execution Recipe

**Existing Recipe overlap findings:** Inspect [file-patch](monty.composition.recipe-implementation.md#rcp016-file-patch), [file-patch-replace-all](monty.composition.recipe-implementation.md#rcp017-file-patch-replace-all). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-apply-patch-all` executes this Skill's one-Tool usage: Use the host boundary to apply a targeted patch via builtin.apply_patch. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l006-pc-exec-apply-patch).

**Exact command:** `run skill-apply-patch-all with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-apply-patch-all:** Candidate implementation `usage_l006(inputs)` (role L006); proposed public export `use_skill_apply_patch_all(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_apply_patch_all(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_apply_patch_all(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l006(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-apply-patch-all",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-apply-patch-all",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR017. skill-grep-invert execution Recipe

**Existing Recipe overlap findings:** Inspect [file-grep](monty.composition.recipe-implementation.md#rcp012-file-grep), [file-grep-files](monty.composition.recipe-implementation.md#rcp013-file-grep-files), [file-grep-content](monty.composition.recipe-implementation.md#rcp014-file-grep-content), [file-grep-count](monty.composition.recipe-implementation.md#rcp015-file-grep-count) and other consumers of the same canonical component. These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-grep-invert` executes this Skill's one-Tool usage: Use the host boundary for an inverted grep via builtin.grep. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l007-pc-exec-grep-invert).

**Exact command:** `run skill-grep-invert with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-grep-invert:** Candidate implementation `usage_l005(inputs)` (role L005); proposed public export `use_skill_grep_invert(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_grep_invert(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_grep_invert(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l005(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-grep-invert",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-grep-invert",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR018. skill-list-dir-files-only execution Recipe

**Existing Recipe overlap findings:** Inspect [file-list](monty.composition.recipe-implementation.md#rcp005-file-list), [file-list-recursive](monty.composition.recipe-implementation.md#rcp006-file-list-recursive), [file-list-files-only](monty.composition.recipe-implementation.md#rcp021-file-list-files-only), [file-list-dirs-only](monty.composition.recipe-implementation.md#rcp022-file-list-dirs-only) and other consumers of the same canonical component. These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-list-dir-files-only` executes this Skill's one-Tool usage: filters a list_dir result to only entries of a given type. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l008-pc-exec-list-filter-by-type).

**Exact command:** `run skill-list-dir-files-only with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-list-dir-files-only:** Candidate implementation `usage_l003(inputs)` (role L003); proposed public export `use_skill_list_dir_files_only(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_list_dir_files_only(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_list_dir_files_only(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l003(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-list-dir-files-only",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-list-dir-files-only",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR019. skill-list-dir-dirs-only execution Recipe

**Existing Recipe overlap findings:** Inspect [file-list](monty.composition.recipe-implementation.md#rcp005-file-list), [file-list-recursive](monty.composition.recipe-implementation.md#rcp006-file-list-recursive), [file-list-files-only](monty.composition.recipe-implementation.md#rcp021-file-list-files-only), [file-list-dirs-only](monty.composition.recipe-implementation.md#rcp022-file-list-dirs-only) and other consumers of the same canonical component. These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-list-dir-dirs-only` executes this Skill's one-Tool usage: filters a list_dir result to only entries of a given type. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l008-pc-exec-list-filter-by-type).

**Exact command:** `run skill-list-dir-dirs-only with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-list-dir-dirs-only:** Candidate implementation `usage_l003(inputs)` (role L003); proposed public export `use_skill_list_dir_dirs_only(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_list_dir_dirs_only(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_list_dir_dirs_only(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l003(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-list-dir-dirs-only",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-list-dir-dirs-only",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR020. skill-read-file-tail execution Recipe

**Existing Recipe overlap findings:** Inspect [file-read-range](monty.composition.recipe-implementation.md#rcp002-file-read-range), [file-read-head](monty.composition.recipe-implementation.md#rcp023-file-read-head), [file-read-tail](monty.composition.recipe-implementation.md#rcp024-file-read-tail). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-read-file-tail` executes this Skill's one-Tool usage: reads the last 50 lines of a file (lines -50 onward). See [its exact component and Skill interface instructions](monty.composition.implementation.md#l009-pc-exec-read-file-tail).

**Exact command:** `run skill-read-file-tail with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-read-file-tail:** Candidate implementation `usage_n05(inputs)` (role N05); proposed public export `use_skill_read_file_tail(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_read_file_tail(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_read_file_tail(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_n05(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-read-file-tail",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-read-file-tail",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR021. skill-file-exists execution Recipe

**Existing Recipe overlap findings:** Inspect [file-exists](monty.composition.recipe-implementation.md#rcp025-file-exists). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-file-exists` executes this Skill's one-Tool usage: checks whether a file exists by attempting to read line 1. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l010-pc-exec-file-exists).

**Exact command:** `run skill-file-exists with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-file-exists:** Proposed public export `use_skill_file_exists(inputs)` remains BLOCKED until its actual one-Tool code/binding contract is qualified. The current local helper/preparation example is not an executable Skill implementation. Do not expose it as a successful Tool usage or invent a host callable.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_file_exists(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_file_exists(inputs)
```

This invocation uses a proposed export, not an implemented alias for `the qualified actual one-Tool function; currently BLOCKED`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-file-exists",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-file-exists",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR022. skill-read-and-grep execution Recipe

**Existing Recipe overlap findings:** Inspect [file-read](monty.composition.recipe-implementation.md#rcp001-file-read), [file-write](monty.composition.recipe-implementation.md#rcp003-file-write), [file-read-tail](monty.composition.recipe-implementation.md#rcp024-file-read-tail), [file-read-and-grep](monty.composition.recipe-implementation.md#rcp026-file-read-and-grep) and other consumers of the same canonical component. These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-read-and-grep` executes this Skill's one-Tool usage: reads a file then greps the content for a pattern. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l011-pc-exec-read-then-grep).

**Exact command:** `run skill-read-and-grep with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-read-and-grep:** Candidate implementation `usage_l001(inputs)` (role L001); proposed public export `use_skill_read_and_grep(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_read_and_grep(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_read_and_grep(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l001(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-read-and-grep",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-read-and-grep",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR023. skill-list-and-filter execution Recipe

**Existing Recipe overlap findings:** Inspect [file-list](monty.composition.recipe-implementation.md#rcp005-file-list), [file-list-recursive](monty.composition.recipe-implementation.md#rcp006-file-list-recursive), [file-list-files-only](monty.composition.recipe-implementation.md#rcp021-file-list-files-only), [file-list-dirs-only](monty.composition.recipe-implementation.md#rcp022-file-list-dirs-only) and other consumers of the same canonical component. These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-list-and-filter` executes this Skill's one-Tool usage: lists directory entries then filters by name substring. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l012-pc-exec-list-then-grep).

**Exact command:** `run skill-list-and-filter with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-list-and-filter:** Candidate implementation `usage_l003(inputs)` (role L003); proposed public export `use_skill_list_and_filter(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_list_and_filter(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_list_and_filter(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l003(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-list-and-filter",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-list-and-filter",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR024. skill-http-get execution Recipe

**Existing Recipe overlap findings:** Inspect [http-get](monty.composition.recipe-implementation.md#rcp028-http-get), [http-get-json](monty.composition.recipe-implementation.md#rcp029-http-get-json), [web-search](monty.composition.recipe-implementation.md#rcp039-web-search), [code-review-pr](monty.composition.recipe-implementation.md#rcp114-code-review-pr) and other consumers of the same canonical component. These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-http-get` executes this Skill's one-Tool usage: Use the host boundary for an HTTP GET request via builtin.http. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l016-pc-exec-http-get).

**Exact command:** `run skill-http-get with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-http-get:** Candidate implementation `usage_l016(inputs)` (role L016); proposed public export `use_skill_http_get(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_http_get(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_http_get(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l016(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-http-get",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-http-get",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR025. skill-http-post execution Recipe

**Existing Recipe overlap findings:** Inspect [http-post](monty.composition.recipe-implementation.md#rcp030-http-post), [http-post-json-webhook](monty.composition.recipe-implementation.md#rcp038-http-post-json-webhook), [github-create-pr](monty.composition.recipe-implementation.md#rcp111-github-create-pr), [github-add-comment](monty.composition.recipe-implementation.md#rcp112-github-add-comment) and other consumers of the same canonical component. These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-http-post` executes this Skill's one-Tool usage: Use the host boundary for an HTTP POST request via builtin.http. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l017-pc-exec-http-post).

**Exact command:** `run skill-http-post with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-http-post:** Candidate implementation `usage_l017(inputs)` (role L017); proposed public export `use_skill_http_post(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_http_post(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_http_post(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l017(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-http-post",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-http-post",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR026. skill-http-save-download execution Recipe

**Existing Recipe overlap findings:** Inspect [http-save](monty.composition.recipe-implementation.md#rcp031-http-save), [http-save-large](monty.composition.recipe-implementation.md#rcp033-http-save-large). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-http-save-download` executes this Skill's one-Tool usage: Use the host boundary for builtin.http.save. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l018-pc-exec-http-save).

**Exact command:** `run skill-http-save-download with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-http-save-download:** Proposed public export `use_skill_http_save_download(inputs)` remains BLOCKED until its actual one-Tool code/binding contract is qualified. The current local helper/preparation example is not an executable Skill implementation. Do not expose it as a successful Tool usage or invent a host callable.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_http_save_download(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_http_save_download(inputs)
```

This invocation uses a proposed export, not an implemented alias for `the qualified actual one-Tool function; currently BLOCKED`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-http-save-download",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-http-save-download",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR027. skill-http-patch execution Recipe

**Existing Recipe overlap findings:** Inspect [http-patch](monty.composition.recipe-implementation.md#rcp032-http-patch). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-http-patch` executes this Skill's one-Tool usage: Use the host boundary for an HTTP PATCH request via builtin.http. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l019-pc-exec-http-patch).

**Exact command:** `run skill-http-patch with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-http-patch:** Candidate implementation `usage_l019(inputs)` (role L019); proposed public export `use_skill_http_patch(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_http_patch(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_http_patch(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l019(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-http-patch",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-http-patch",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR028. skill-http-head execution Recipe

**Existing Recipe overlap findings:** Inspect [http-head](monty.composition.recipe-implementation.md#rcp034-http-head). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-http-head` executes this Skill's one-Tool usage: Use the host boundary for an HTTP HEAD request via builtin.http. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l020-pc-exec-http-head).

**Exact command:** `run skill-http-head with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-http-head:** Candidate implementation `usage_l020(inputs)` (role L020); proposed public export `use_skill_http_head(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_http_head(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_http_head(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l020(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-http-head",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-http-head",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR029. skill-http-authenticated execution Recipe

**Existing Recipe overlap findings:** Inspect [http-authenticated-get](monty.composition.recipe-implementation.md#rcp035-http-authenticated-get). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-http-authenticated` executes this Skill's one-Tool usage: Use the host boundary for an authenticated HTTP GET via builtin.http. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l021-pc-exec-http-get-authenticated).

**Exact command:** `run skill-http-authenticated with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-http-authenticated:** Candidate implementation `usage_l021(inputs)` (role L021); proposed public export `use_skill_http_authenticated(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_http_authenticated(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_http_authenticated(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l021(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-http-authenticated",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-http-authenticated",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR030. skill-http-put execution Recipe

**Existing Recipe overlap findings:** Inspect [http-put](monty.composition.recipe-implementation.md#rcp036-http-put). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-http-put` executes this Skill's one-Tool usage: Use the host boundary for an HTTP PUT request via builtin.http. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l022-pc-exec-http-put).

**Exact command:** `run skill-http-put with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-http-put:** Candidate implementation `usage_l022(inputs)` (role L022); proposed public export `use_skill_http_put(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_http_put(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_http_put(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l022(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-http-put",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-http-put",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR031. skill-http-delete execution Recipe

**Existing Recipe overlap findings:** Inspect [http-delete](monty.composition.recipe-implementation.md#rcp037-http-delete). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-http-delete` executes this Skill's one-Tool usage: Use the host boundary for an HTTP DELETE request via builtin.http. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l023-pc-exec-http-delete).

**Exact command:** `run skill-http-delete with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-http-delete:** Candidate implementation `usage_l023(inputs)` (role L023); proposed public export `use_skill_http_delete(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_http_delete(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_http_delete(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l023(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-http-delete",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-http-delete",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR032. skill-memory-search execution Recipe

**Existing Recipe overlap findings:** Inspect [memory-search](monty.composition.recipe-implementation.md#rcp040-memory-search), [memory-search-broad](monty.composition.recipe-implementation.md#rcp041-memory-search-broad), [memory-search-and-read](monty.composition.recipe-implementation.md#rcp051-memory-search-and-read). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-memory-search` executes this Skill's one-Tool usage: Use the host boundary to search persistent memory via builtin.memory_search. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l028-pc-exec-memory-search).

**Exact command:** `run skill-memory-search with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-memory-search:** Candidate implementation `usage_l028(inputs)` (role L028); proposed public export `use_skill_memory_search(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_memory_search(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_memory_search(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l028(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-memory-search",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-memory-search",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR033. skill-memory-search-broad execution Recipe

**Existing Recipe overlap findings:** Inspect [memory-search](monty.composition.recipe-implementation.md#rcp040-memory-search), [memory-search-broad](monty.composition.recipe-implementation.md#rcp041-memory-search-broad), [memory-search-and-read](monty.composition.recipe-implementation.md#rcp051-memory-search-and-read). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-memory-search-broad` executes this Skill's one-Tool usage: Use the host boundary to search persistent memory via builtin.memory_search. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l028-pc-exec-memory-search).

**Exact command:** `run skill-memory-search-broad with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-memory-search-broad:** Candidate implementation `usage_l028(inputs)` (role L028); proposed public export `use_skill_memory_search_broad(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_memory_search_broad(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_memory_search_broad(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l028(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-memory-search-broad",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-memory-search-broad",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR034. skill-memory-write-log execution Recipe

**Existing Recipe overlap findings:** Inspect [memory-write](monty.composition.recipe-implementation.md#rcp042-memory-write), [memory-write-log](monty.composition.recipe-implementation.md#rcp043-memory-write-log), [memory-write-main](monty.composition.recipe-implementation.md#rcp044-memory-write-main), [plan-create](monty.composition.recipe-implementation.md#rcp121-plan-create) and other consumers of the same canonical component. These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-memory-write-log` executes this Skill's one-Tool usage: Use the host boundary to write to persistent memory via builtin.memory_write. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l029-pc-exec-memory-write).

**Exact command:** `run skill-memory-write-log with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-memory-write-log:** Candidate implementation `usage_l029(inputs)` (role L029); proposed public export `use_skill_memory_write_log(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_memory_write_log(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_memory_write_log(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l029(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-memory-write-log",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-memory-write-log",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR035. skill-memory-write-main execution Recipe

**Existing Recipe overlap findings:** Inspect [memory-write](monty.composition.recipe-implementation.md#rcp042-memory-write), [memory-write-log](monty.composition.recipe-implementation.md#rcp043-memory-write-log), [memory-write-main](monty.composition.recipe-implementation.md#rcp044-memory-write-main), [plan-create](monty.composition.recipe-implementation.md#rcp121-plan-create) and other consumers of the same canonical component. These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-memory-write-main` executes this Skill's one-Tool usage: Use the host boundary to write to persistent memory via builtin.memory_write. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l029-pc-exec-memory-write).

**Exact command:** `run skill-memory-write-main with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-memory-write-main:** Candidate implementation `usage_l029(inputs)` (role L029); proposed public export `use_skill_memory_write_main(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_memory_write_main(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_memory_write_main(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l029(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-memory-write-main",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-memory-write-main",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR036. skill-memory-write-patch execution Recipe

**Existing Recipe overlap findings:** Inspect [memory-write-patch](monty.composition.recipe-implementation.md#rcp045-memory-write-patch), [plan-update-step](monty.composition.recipe-implementation.md#rcp124-plan-update-step). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-memory-write-patch` executes this Skill's one-Tool usage: Use the host boundary for a targeted patch to a memory document via builtin.memory_write patch mode. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l030-pc-exec-memory-patch).

**Exact command:** `run skill-memory-write-patch with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-memory-write-patch:** Candidate implementation `usage_l030(inputs)` (role L030); proposed public export `use_skill_memory_write_patch(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_memory_write_patch(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_memory_write_patch(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l030(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-memory-write-patch",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-memory-write-patch",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR037. skill-memory-read execution Recipe

**Existing Recipe overlap findings:** Inspect [memory-write-patch](monty.composition.recipe-implementation.md#rcp045-memory-write-patch), [memory-write-append](monty.composition.recipe-implementation.md#rcp046-memory-write-append), [memory-read](monty.composition.recipe-implementation.md#rcp047-memory-read), [memory-read-main](monty.composition.recipe-implementation.md#rcp048-memory-read-main) and other consumers of the same canonical component. These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-memory-read` executes this Skill's one-Tool usage: Use the host boundary to read a memory document by path via builtin.memory_read. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l031-pc-exec-memory-read).

**Exact command:** `run skill-memory-read with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-memory-read:** Candidate implementation `usage_l031(inputs)` (role L031); proposed public export `use_skill_memory_read(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_memory_read(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_memory_read(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l031(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-memory-read",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-memory-read",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR038. skill-memory-tree execution Recipe

**Existing Recipe overlap findings:** Inspect [memory-tree](monty.composition.recipe-implementation.md#rcp050-memory-tree). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-memory-tree` executes this Skill's one-Tool usage: Use the host boundary to list the memory directory tree via builtin.memory_tree. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l032-pc-exec-memory-tree).

**Exact command:** `run skill-memory-tree with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-memory-tree:** Candidate implementation `usage_l032(inputs)` (role L032); proposed public export `use_skill_memory_tree(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_memory_tree(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_memory_tree(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l032(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-memory-tree",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-memory-tree",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR039. skill-memory-write-append execution Recipe

**Existing Recipe overlap findings:** Inspect [memory-write-append](monty.composition.recipe-implementation.md#rcp046-memory-write-append). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-memory-write-append` executes this Skill's one-Tool usage: appends text to an existing memory document. Reads the current content via memory_read, then writes combined content via memory_write. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l035-pc-exec-memory-append).

**Exact command:** `run skill-memory-write-append with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-memory-write-append:** Candidate implementation `usage_l035(inputs)` (role L035); proposed public export `use_skill_memory_write_append(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_memory_write_append(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_memory_write_append(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l035(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-memory-write-append",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-memory-write-append",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR040. skill-shell-git-status execution Recipe

**Existing Recipe overlap findings:** Inspect [shell-git-status](monty.composition.recipe-implementation.md#rcp052-shell-git-status), [commit-workflow](monty.composition.recipe-implementation.md#rcp106-commit-workflow). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-shell-git-status` executes this Skill's one-Tool usage: runs 'git status' in the workspace root via builtin.shell. Command is a fixed literal. No user input enters the command string. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l036-pc-exec-shell-git-status).

**Exact command:** `run skill-shell-git-status with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-shell-git-status:** Candidate implementation `usage_l036(inputs)` (role L036); proposed public export `use_skill_shell_git_status(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_shell_git_status(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_shell_git_status(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l036(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-shell-git-status",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-shell-git-status",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR041. skill-shell-git-log execution Recipe

**Existing Recipe overlap findings:** Inspect [shell-git-log](monty.composition.recipe-implementation.md#rcp053-shell-git-log), [commit-workflow](monty.composition.recipe-implementation.md#rcp106-commit-workflow). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-shell-git-log` executes this Skill's one-Tool usage: runs 'git log --oneline -20' to get the last 20 commits. Fixed literal command. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l037-pc-exec-shell-git-log).

**Exact command:** `run skill-shell-git-log with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-shell-git-log:** Candidate implementation `usage_l037(inputs)` (role L037); proposed public export `use_skill_shell_git_log(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_shell_git_log(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_shell_git_log(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l037(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-shell-git-log",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-shell-git-log",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR042. skill-shell-git-diff-stat execution Recipe

**Existing Recipe overlap findings:** Inspect [shell-git-diff-stat](monty.composition.recipe-implementation.md#rcp054-shell-git-diff-stat). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-shell-git-diff-stat` executes this Skill's one-Tool usage: runs 'git diff --stat' to show changed file summary. Fixed literal command. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l038-pc-exec-shell-git-diff-stat).

**Exact command:** `run skill-shell-git-diff-stat with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-shell-git-diff-stat:** Candidate implementation `usage_l038(inputs)` (role L038); proposed public export `use_skill_shell_git_diff_stat(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_shell_git_diff_stat(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_shell_git_diff_stat(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l038(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-shell-git-diff-stat",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-shell-git-diff-stat",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR043. skill-shell-git-branch execution Recipe

**Existing Recipe overlap findings:** Inspect [shell-git-branch](monty.composition.recipe-implementation.md#rcp055-shell-git-branch). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-shell-git-branch` executes this Skill's one-Tool usage: runs 'git branch -a' to list all local and remote branches. Fixed literal command. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l039-pc-exec-shell-git-branch).

**Exact command:** `run skill-shell-git-branch with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-shell-git-branch:** Candidate implementation `usage_l039(inputs)` (role L039); proposed public export `use_skill_shell_git_branch(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_shell_git_branch(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_shell_git_branch(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l039(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-shell-git-branch",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-shell-git-branch",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR044. skill-shell-git-stash-list execution Recipe

**Existing Recipe overlap findings:** Inspect [shell-git-stash-list](monty.composition.recipe-implementation.md#rcp056-shell-git-stash-list). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-shell-git-stash-list` executes this Skill's one-Tool usage: runs 'git stash list' to show the stash stack. Fixed literal command. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l040-pc-exec-shell-git-stash-list).

**Exact command:** `run skill-shell-git-stash-list with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-shell-git-stash-list:** Candidate implementation `usage_l040(inputs)` (role L040); proposed public export `use_skill_shell_git_stash_list(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_shell_git_stash_list(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_shell_git_stash_list(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l040(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-shell-git-stash-list",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-shell-git-stash-list",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR045. skill-shell-git-remote execution Recipe

**Existing Recipe overlap findings:** Inspect [shell-git-remote](monty.composition.recipe-implementation.md#rcp068-shell-git-remote). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-shell-git-remote` executes this Skill's one-Tool usage: runs 'git remote -v' to list all configured remote repositories and their URLs. Fixed literal command. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l042-pc-exec-shell-git-remote).

**Exact command:** `run skill-shell-git-remote with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-shell-git-remote:** Candidate implementation `usage_l042(inputs)` (role L042); proposed public export `use_skill_shell_git_remote(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_shell_git_remote(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_shell_git_remote(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l042(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-shell-git-remote",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-shell-git-remote",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR046. skill-shell-git-show-stat execution Recipe

**Existing Recipe overlap findings:** Inspect [shell-git-show-stat](monty.composition.recipe-implementation.md#rcp069-shell-git-show-stat). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-shell-git-show-stat` executes this Skill's one-Tool usage: runs 'git show --stat HEAD' to show the last commit's changed files and line counts. Fixed literal command. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l043-pc-exec-shell-git-show-stat).

**Exact command:** `run skill-shell-git-show-stat with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-shell-git-show-stat:** Candidate implementation `usage_l043(inputs)` (role L043); proposed public export `use_skill_shell_git_show_stat(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_shell_git_show_stat(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_shell_git_show_stat(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l043(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-shell-git-show-stat",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-shell-git-show-stat",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR047. skill-shell-git-tag-list execution Recipe

**Existing Recipe overlap findings:** Inspect [shell-git-tag-list](monty.composition.recipe-implementation.md#rcp070-shell-git-tag-list). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-shell-git-tag-list` executes this Skill's one-Tool usage: runs 'git tag --list' to enumerate all tags in the repository. Fixed literal command. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l044-pc-exec-shell-git-tag-list).

**Exact command:** `run skill-shell-git-tag-list with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-shell-git-tag-list:** Candidate implementation `usage_l044(inputs)` (role L044); proposed public export `use_skill_shell_git_tag_list(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_shell_git_tag_list(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_shell_git_tag_list(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l044(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-shell-git-tag-list",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-shell-git-tag-list",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR048. skill-shell-git-diff-name-only execution Recipe

**Existing Recipe overlap findings:** Inspect [shell-git-diff-name-only](monty.composition.recipe-implementation.md#rcp072-shell-git-diff-name-only). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-shell-git-diff-name-only` executes this Skill's one-Tool usage: runs 'git diff --name-only HEAD' to list only the names of files changed since the last commit. No content shown. Fixed literal command — no slot interpolation. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l045-pc-exec-shell-git-diff-name-only).

**Exact command:** `run skill-shell-git-diff-name-only with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-shell-git-diff-name-only:** Candidate implementation `usage_l045(inputs)` (role L045); proposed public export `use_skill_shell_git_diff_name_only(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_shell_git_diff_name_only(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_shell_git_diff_name_only(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l045(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-shell-git-diff-name-only",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-shell-git-diff-name-only",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR049. skill-shell-git-log-stat execution Recipe

**Existing Recipe overlap findings:** Inspect [shell-git-log-stat](monty.composition.recipe-implementation.md#rcp073-shell-git-log-stat). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-shell-git-log-stat` executes this Skill's one-Tool usage: runs 'git log --stat --oneline -5' to show the last 5 commits with file-change counts per commit. Fixed literal. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l046-pc-exec-shell-git-log-stat).

**Exact command:** `run skill-shell-git-log-stat with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-shell-git-log-stat:** Candidate implementation `usage_l046(inputs)` (role L046); proposed public export `use_skill_shell_git_log_stat(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_shell_git_log_stat(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_shell_git_log_stat(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l046(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-shell-git-log-stat",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-shell-git-log-stat",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR050. skill-shell-git-stash-show execution Recipe

**Existing Recipe overlap findings:** Inspect [shell-git-stash-show](monty.composition.recipe-implementation.md#rcp074-shell-git-stash-show). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-shell-git-stash-show` executes this Skill's one-Tool usage: runs 'git stash show' to show the diff summary of the most recent stash entry. Fixed literal command. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l047-pc-exec-shell-git-stash-show).

**Exact command:** `run skill-shell-git-stash-show with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-shell-git-stash-show:** Candidate implementation `usage_l047(inputs)` (role L047); proposed public export `use_skill_shell_git_stash_show(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_shell_git_stash_show(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_shell_git_stash_show(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l047(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-shell-git-stash-show",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-shell-git-stash-show",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR051. skill-shell-git-config-list execution Recipe

**Existing Recipe overlap findings:** Inspect [shell-git-config-list](monty.composition.recipe-implementation.md#rcp075-shell-git-config-list). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-shell-git-config-list` executes this Skill's one-Tool usage: runs 'git config --list' to show all active git configuration values. Fixed literal command. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l048-pc-exec-shell-git-config-list).

**Exact command:** `run skill-shell-git-config-list with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-shell-git-config-list:** Candidate implementation `usage_l048(inputs)` (role L048); proposed public export `use_skill_shell_git_config_list(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_shell_git_config_list(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_shell_git_config_list(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l048(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-shell-git-config-list",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-shell-git-config-list",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR052. skill-shell-git-add execution Recipe

**Existing Recipe overlap findings:** No direct consumer of this exact canonical role was found in the per-Recipe implementation appendix. Recheck the actual eligible catalogue before allocating an identity; a missing source reference is not proof that no equivalent installed Recipe exists.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-shell-git-add` executes this Skill's one-Tool usage: Use the host boundary to run 'git add <path>'. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l049-pc-exec-shell-git-add).

**Exact command:** `run skill-shell-git-add with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-shell-git-add:** Public export `use_skill_shell_git_add(inputs)` must use the qualified shared shell-execution function plus this usage's reviewed command preparer/fixed-command profile. The local preparation block is a pure helper, not a Tool executor. Resolve the actual canonical code UUIDs/symbols before publishing the interface; preserve Tier1.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_shell_git_add(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_shell_git_add(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_proposed_shell_dispatch(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-shell-git-add",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-shell-git-add",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR053. skill-shell-git-commit execution Recipe

**Existing Recipe overlap findings:** No direct consumer of this exact canonical role was found in the per-Recipe implementation appendix. Recheck the actual eligible catalogue before allocating an identity; a missing source reference is not proof that no equivalent installed Recipe exists.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-shell-git-commit` executes this Skill's one-Tool usage: Use the host boundary to run 'git commit -m <msg>'. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l050-pc-exec-shell-git-commit).

**Exact command:** `run skill-shell-git-commit with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-shell-git-commit:** Public export `use_skill_shell_git_commit(inputs)` must use the qualified shared shell-execution function plus this usage's reviewed command preparer/fixed-command profile. The local preparation block is a pure helper, not a Tool executor. Resolve the actual canonical code UUIDs/symbols before publishing the interface; preserve Tier1.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_shell_git_commit(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_shell_git_commit(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_proposed_shell_dispatch(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-shell-git-commit",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-shell-git-commit",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR054. skill-shell-git-push execution Recipe

**Existing Recipe overlap findings:** No direct consumer of this exact canonical role was found in the per-Recipe implementation appendix. Recheck the actual eligible catalogue before allocating an identity; a missing source reference is not proof that no equivalent installed Recipe exists.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-shell-git-push` executes this Skill's one-Tool usage: Use the host boundary to run 'git push <remote> <branch>'. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l051-pc-exec-shell-git-push).

**Exact command:** `run skill-shell-git-push with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-shell-git-push:** Public export `use_skill_shell_git_push(inputs)` must use the qualified shared shell-execution function plus this usage's reviewed command preparer/fixed-command profile. The local preparation block is a pure helper, not a Tool executor. Resolve the actual canonical code UUIDs/symbols before publishing the interface; preserve Tier1.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_shell_git_push(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_shell_git_push(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_proposed_shell_dispatch(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-shell-git-push",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-shell-git-push",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR055. skill-shell-git-pull execution Recipe

**Existing Recipe overlap findings:** No direct consumer of this exact canonical role was found in the per-Recipe implementation appendix. Recheck the actual eligible catalogue before allocating an identity; a missing source reference is not proof that no equivalent installed Recipe exists.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-shell-git-pull` executes this Skill's one-Tool usage: Use the host boundary to run 'git pull <remote> <branch>'. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l052-pc-exec-shell-git-pull).

**Exact command:** `run skill-shell-git-pull with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-shell-git-pull:** Public export `use_skill_shell_git_pull(inputs)` must use the qualified shared shell-execution function plus this usage's reviewed command preparer/fixed-command profile. The local preparation block is a pure helper, not a Tool executor. Resolve the actual canonical code UUIDs/symbols before publishing the interface; preserve Tier1.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_shell_git_pull(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_shell_git_pull(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_proposed_shell_dispatch(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-shell-git-pull",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-shell-git-pull",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR056. skill-shell-git-fetch execution Recipe

**Existing Recipe overlap findings:** Inspect [shell-git-fetch](monty.composition.recipe-implementation.md#rcp076-shell-git-fetch). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-shell-git-fetch` executes this Skill's one-Tool usage: Use the host boundary to run 'git fetch --all'. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l053-pc-exec-shell-git-fetch).

**Exact command:** `run skill-shell-git-fetch with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-shell-git-fetch:** Candidate implementation `usage_l053(inputs)` (role L053); proposed public export `use_skill_shell_git_fetch(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_shell_git_fetch(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_shell_git_fetch(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l053(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-shell-git-fetch",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-shell-git-fetch",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR057. skill-shell-pwd execution Recipe

**Existing Recipe overlap findings:** Inspect [shell-pwd](monty.composition.recipe-implementation.md#rcp057-shell-pwd). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-shell-pwd` executes this Skill's one-Tool usage: runs 'pwd' to show the current working directory. Fixed literal command. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l054-pc-exec-shell-pwd).

**Exact command:** `run skill-shell-pwd with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-shell-pwd:** Candidate implementation `usage_l054(inputs)` (role L054); proposed public export `use_skill_shell_pwd(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_shell_pwd(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_shell_pwd(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l054(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-shell-pwd",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-shell-pwd",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR058. skill-shell-df execution Recipe

**Existing Recipe overlap findings:** Inspect [shell-df](monty.composition.recipe-implementation.md#rcp058-shell-df). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-shell-df` executes this Skill's one-Tool usage: runs 'df -h' to show disk usage in human-readable format. Fixed literal command. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l055-pc-exec-shell-df).

**Exact command:** `run skill-shell-df with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-shell-df:** Candidate implementation `usage_l055(inputs)` (role L055); proposed public export `use_skill_shell_df(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_shell_df(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_shell_df(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l055(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-shell-df",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-shell-df",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR059. skill-shell-ps execution Recipe

**Existing Recipe overlap findings:** Inspect [shell-ps](monty.composition.recipe-implementation.md#rcp059-shell-ps). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-shell-ps` executes this Skill's one-Tool usage: runs 'ps aux' to list running processes. Fixed literal command. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l056-pc-exec-shell-ps).

**Exact command:** `run skill-shell-ps with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-shell-ps:** Candidate implementation `usage_l056(inputs)` (role L056); proposed public export `use_skill_shell_ps(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_shell_ps(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_shell_ps(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l056(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-shell-ps",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-shell-ps",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR060. skill-shell-env execution Recipe

**Existing Recipe overlap findings:** Inspect [shell-env](monty.composition.recipe-implementation.md#rcp060-shell-env). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-shell-env` executes this Skill's one-Tool usage: runs 'env' to list all environment variables in the current session. Fixed literal command. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l057-pc-exec-shell-env).

**Exact command:** `run skill-shell-env with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-shell-env:** Candidate implementation `usage_l057(inputs)` (role L057); proposed public export `use_skill_shell_env(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_shell_env(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_shell_env(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l057(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-shell-env",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-shell-env",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR061. skill-shell-uname execution Recipe

**Existing Recipe overlap findings:** Inspect [shell-uname](monty.composition.recipe-implementation.md#rcp061-shell-uname). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-shell-uname` executes this Skill's one-Tool usage: runs 'uname -a' to show OS/kernel information. Fixed literal command. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l058-pc-exec-shell-uname).

**Exact command:** `run skill-shell-uname with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-shell-uname:** Candidate implementation `usage_l058(inputs)` (role L058); proposed public export `use_skill_shell_uname(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_shell_uname(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_shell_uname(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l058(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-shell-uname",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-shell-uname",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR062. skill-shell-which execution Recipe

**Existing Recipe overlap findings:** Inspect [shell-which](monty.composition.recipe-implementation.md#rcp062-shell-which). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-shell-which` executes this Skill's one-Tool usage: runs 'which <toolname>' to locate a binary. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l059-pc-exec-shell-which).

**Exact command:** `run skill-shell-which with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-shell-which:** Candidate implementation `usage_l059(inputs)` (role L059); proposed public export `use_skill_shell_which(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_shell_which(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_shell_which(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l059(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-shell-which",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-shell-which",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR063. skill-shell-hostname execution Recipe

**Existing Recipe overlap findings:** Inspect [shell-hostname](monty.composition.recipe-implementation.md#rcp064-shell-hostname). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-shell-hostname` executes this Skill's one-Tool usage: runs 'hostname' to print the machine hostname. Fixed literal command. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l060-pc-exec-shell-hostname).

**Exact command:** `run skill-shell-hostname with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-shell-hostname:** Candidate implementation `usage_l060(inputs)` (role L060); proposed public export `use_skill_shell_hostname(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_shell_hostname(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_shell_hostname(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l060(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-shell-hostname",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-shell-hostname",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR064. skill-shell-whoami execution Recipe

**Existing Recipe overlap findings:** Inspect [shell-whoami](monty.composition.recipe-implementation.md#rcp065-shell-whoami). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-shell-whoami` executes this Skill's one-Tool usage: runs 'whoami' to print the current user account name. Fixed literal command. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l061-pc-exec-shell-whoami).

**Exact command:** `run skill-shell-whoami with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-shell-whoami:** Candidate implementation `usage_l061(inputs)` (role L061); proposed public export `use_skill_shell_whoami(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_shell_whoami(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_shell_whoami(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l061(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-shell-whoami",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-shell-whoami",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR065. skill-shell-uptime execution Recipe

**Existing Recipe overlap findings:** Inspect [shell-uptime](monty.composition.recipe-implementation.md#rcp066-shell-uptime). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-shell-uptime` executes this Skill's one-Tool usage: runs 'uptime' to show system uptime, load average, and logged-in user count. Fixed literal command. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l062-pc-exec-shell-uptime).

**Exact command:** `run skill-shell-uptime with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-shell-uptime:** Candidate implementation `usage_l062(inputs)` (role L062); proposed public export `use_skill_shell_uptime(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_shell_uptime(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_shell_uptime(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l062(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-shell-uptime",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-shell-uptime",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR066. skill-shell-free execution Recipe

**Existing Recipe overlap findings:** Inspect [shell-free](monty.composition.recipe-implementation.md#rcp067-shell-free). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-shell-free` executes this Skill's one-Tool usage: runs 'free -h' to show memory usage in human-readable format. Fixed literal command. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l063-pc-exec-shell-free).

**Exact command:** `run skill-shell-free with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-shell-free:** Candidate implementation `usage_l063(inputs)` (role L063); proposed public export `use_skill_shell_free(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_shell_free(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_shell_free(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l063(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-shell-free",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-shell-free",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR067. skill-shell-wc-l execution Recipe

**Existing Recipe overlap findings:** No direct consumer of this exact canonical role was found in the per-Recipe implementation appendix. Recheck the actual eligible catalogue before allocating an identity; a missing source reference is not proof that no equivalent installed Recipe exists.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-shell-wc-l` executes this Skill's one-Tool usage: runs 'wc -l <filepath>' to count lines in a file. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l064-pc-exec-shell-wc-l).

**Exact command:** `run skill-shell-wc-l with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-shell-wc-l:** Public export `use_skill_shell_wc_l(inputs)` must use the qualified shared shell-execution function plus this usage's reviewed command preparer/fixed-command profile. The local preparation block is a pure helper, not a Tool executor. Resolve the actual canonical code UUIDs/symbols before publishing the interface; preserve Tier1.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_shell_wc_l(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_shell_wc_l(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_proposed_shell_dispatch(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-shell-wc-l",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-shell-wc-l",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR068. skill-trigger-list execution Recipe

**Existing Recipe overlap findings:** Inspect [trigger-list](monty.composition.recipe-implementation.md#rcp088-trigger-list), [trigger-list-active](monty.composition.recipe-implementation.md#rcp089-trigger-list-active), [trigger-list-scheduled](monty.composition.recipe-implementation.md#rcp090-trigger-list-scheduled), [trigger-remove-by-name](monty.composition.recipe-implementation.md#rcp093-trigger-remove-by-name). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-trigger-list` executes this Skill's one-Tool usage: calls host.trigger_list to list configured triggers. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l065-pc-exec-trigger-list).

**Exact command:** `run skill-trigger-list with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-trigger-list:** Candidate implementation `usage_l065(inputs)` (role L065); proposed public export `use_skill_trigger_list(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_trigger_list(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_trigger_list(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l065(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-trigger-list",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-trigger-list",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR069. skill-trigger-list-active execution Recipe

**Existing Recipe overlap findings:** Inspect [trigger-list](monty.composition.recipe-implementation.md#rcp088-trigger-list), [trigger-list-active](monty.composition.recipe-implementation.md#rcp089-trigger-list-active), [trigger-list-scheduled](monty.composition.recipe-implementation.md#rcp090-trigger-list-scheduled), [trigger-remove-by-name](monty.composition.recipe-implementation.md#rcp093-trigger-remove-by-name). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-trigger-list-active` executes this Skill's one-Tool usage: calls host.trigger_list to list configured triggers. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l065-pc-exec-trigger-list).

**Exact command:** `run skill-trigger-list-active with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-trigger-list-active:** Candidate implementation `usage_l065(inputs)` (role L065); proposed public export `use_skill_trigger_list_active(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_trigger_list_active(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_trigger_list_active(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l065(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-trigger-list-active",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-trigger-list-active",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR070. skill-trigger-list-scheduled execution Recipe

**Existing Recipe overlap findings:** Inspect [trigger-list](monty.composition.recipe-implementation.md#rcp088-trigger-list), [trigger-list-active](monty.composition.recipe-implementation.md#rcp089-trigger-list-active), [trigger-list-scheduled](monty.composition.recipe-implementation.md#rcp090-trigger-list-scheduled), [trigger-remove-by-name](monty.composition.recipe-implementation.md#rcp093-trigger-remove-by-name). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-trigger-list-scheduled` executes this Skill's one-Tool usage: calls host.trigger_list to list configured triggers. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l065-pc-exec-trigger-list).

**Exact command:** `run skill-trigger-list-scheduled with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-trigger-list-scheduled:** Candidate implementation `usage_l065(inputs)` (role L065); proposed public export `use_skill_trigger_list_scheduled(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_trigger_list_scheduled(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_trigger_list_scheduled(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l065(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-trigger-list-scheduled",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-trigger-list-scheduled",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR071. skill-time-now execution Recipe

**Existing Recipe overlap findings:** Inspect [time-now](monty.composition.recipe-implementation.md#rcp094-time-now), [time-now-tz](monty.composition.recipe-implementation.md#rcp095-time-now-tz). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-time-now` executes this Skill's one-Tool usage: Use the host boundary to get the current timestamp via builtin.time operation='now'. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l067-pc-exec-time-now).

**Exact command:** `run skill-time-now with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-time-now:** Candidate implementation `usage_l067(inputs)` (role L067); proposed public export `use_skill_time_now(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_time_now(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_time_now(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l067(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-time-now",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-time-now",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR072. skill-time-parse execution Recipe

**Existing Recipe overlap findings:** Inspect [time-parse](monty.composition.recipe-implementation.md#rcp096-time-parse). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-time-parse` executes this Skill's one-Tool usage: Use the host boundary to parse a timestamp string via builtin.time operation='parse'. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l068-pc-exec-time-parse).

**Exact command:** `run skill-time-parse with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-time-parse:** Candidate implementation `usage_l068(inputs)` (role L068); proposed public export `use_skill_time_parse(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_time_parse(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_time_parse(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l068(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-time-parse",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-time-parse",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR073. skill-time-convert execution Recipe

**Existing Recipe overlap findings:** Inspect [time-convert](monty.composition.recipe-implementation.md#rcp097-time-convert). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-time-convert` executes this Skill's one-Tool usage: Use the host boundary to convert a timestamp between timezones via builtin.time operation='convert'. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l069-pc-exec-time-convert).

**Exact command:** `run skill-time-convert with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-time-convert:** Candidate implementation `usage_l069(inputs)` (role L069); proposed public export `use_skill_time_convert(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_time_convert(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_time_convert(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l069(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-time-convert",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-time-convert",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR074. skill-time-diff execution Recipe

**Existing Recipe overlap findings:** Inspect [time-diff](monty.composition.recipe-implementation.md#rcp098-time-diff). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-time-diff` executes this Skill's one-Tool usage: Use the host boundary to compute the signed difference between two timestamps via builtin.time operation='diff'. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l070-pc-exec-time-diff).

**Exact command:** `run skill-time-diff with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-time-diff:** Candidate implementation `usage_l070(inputs)` (role L070); proposed public export `use_skill_time_diff(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_time_diff(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_time_diff(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l070(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-time-diff",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-time-diff",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR075. skill-time-format execution Recipe

**Existing Recipe overlap findings:** Inspect [time-format](monty.composition.recipe-implementation.md#rcp099-time-format). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-time-format` executes this Skill's one-Tool usage: Use the host boundary to format a timestamp as a human-readable string via builtin.time operation='format'. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l071-pc-exec-time-format).

**Exact command:** `run skill-time-format with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-time-format:** Candidate implementation `usage_l071(inputs)` (role L071); proposed public export `use_skill_time_format(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_time_format(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_time_format(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l071(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-time-format",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-time-format",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR076. skill-json-query execution Recipe

**Existing Recipe overlap findings:** Inspect [json-query](monty.composition.recipe-implementation.md#rcp100-json-query), [json-parse-and-query](monty.composition.recipe-implementation.md#rcp104-json-parse-and-query). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-json-query` executes this Skill's one-Tool usage: Use the host boundary for json query operation. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l072-pc-exec-json-query).

**Exact command:** `run skill-json-query with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-json-query:** Candidate implementation `usage_l072(inputs)` (role L072); proposed public export `use_skill_json_query(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_json_query(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_json_query(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l072(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-json-query",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-json-query",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR077. skill-json-stringify execution Recipe

**Existing Recipe overlap findings:** Inspect [json-stringify](monty.composition.recipe-implementation.md#rcp101-json-stringify). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-json-stringify` executes this Skill's one-Tool usage: Use the host boundary for json stringify or parse. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l073-pc-exec-json-stringify).

**Exact command:** `run skill-json-stringify with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-json-stringify:** Candidate implementation `usage_l073(inputs)` (role L073); proposed public export `use_skill_json_stringify(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_json_stringify(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_json_stringify(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l073(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-json-stringify",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-json-stringify",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR078. skill-json-validate execution Recipe

**Existing Recipe overlap findings:** Inspect [json-validate](monty.composition.recipe-implementation.md#rcp103-json-validate). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-json-validate` executes this Skill's one-Tool usage: Use the host boundary to validate a JSON string. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l074-pc-exec-json-validate).

**Exact command:** `run skill-json-validate with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-json-validate:** Candidate implementation `usage_l074(inputs)` (role L074); proposed public export `use_skill_json_validate(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_json_validate(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_json_validate(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l074(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-json-validate",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-json-validate",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR079. skill-github-list-issues execution Recipe

**Existing Recipe overlap findings:** Inspect [http-get](monty.composition.recipe-implementation.md#rcp028-http-get), [http-get-json](monty.composition.recipe-implementation.md#rcp029-http-get-json), [web-search](monty.composition.recipe-implementation.md#rcp039-web-search), [code-review-pr](monty.composition.recipe-implementation.md#rcp114-code-review-pr) and other consumers of the same canonical component. These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-github-list-issues` executes this Skill's one-Tool usage: GET /repos/{owner}/{repo}/issues?state=open. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l076-pc-github-list-issues).

**Exact command:** `run skill-github-list-issues with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-github-list-issues:** Candidate implementation `usage_l016(inputs)` (role L016); proposed public export `use_skill_github_list_issues(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_github_list_issues(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_github_list_issues(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l016(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-github-list-issues",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-github-list-issues",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR080. skill-github-list-prs execution Recipe

**Existing Recipe overlap findings:** Inspect [http-get](monty.composition.recipe-implementation.md#rcp028-http-get), [http-get-json](monty.composition.recipe-implementation.md#rcp029-http-get-json), [web-search](monty.composition.recipe-implementation.md#rcp039-web-search), [code-review-pr](monty.composition.recipe-implementation.md#rcp114-code-review-pr) and other consumers of the same canonical component. These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-github-list-prs` executes this Skill's one-Tool usage: GET /repos/{owner}/{repo}/pulls?state=open. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l077-pc-github-list-prs).

**Exact command:** `run skill-github-list-prs with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-github-list-prs:** Candidate implementation `usage_l016(inputs)` (role L016); proposed public export `use_skill_github_list_prs(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_github_list_prs(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_github_list_prs(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l016(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-github-list-prs",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-github-list-prs",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR081. skill-github-get-authenticated-user execution Recipe

**Existing Recipe overlap findings:** Inspect [github-get-authenticated-user](monty.composition.recipe-implementation.md#rcp109-github-get-authenticated-user). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-github-get-authenticated-user` executes this Skill's one-Tool usage: GET /user — returns login, id, name, email. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l078-pc-github-get-authenticated-user).

**Exact command:** `run skill-github-get-authenticated-user with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-github-get-authenticated-user:** Candidate implementation `usage_l078(inputs)` (role L078); proposed public export `use_skill_github_get_authenticated_user(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_github_get_authenticated_user(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_github_get_authenticated_user(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l078(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-github-get-authenticated-user",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-github-get-authenticated-user",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR082. skill-github-search-issues execution Recipe

**Existing Recipe overlap findings:** Inspect [http-get](monty.composition.recipe-implementation.md#rcp028-http-get), [http-get-json](monty.composition.recipe-implementation.md#rcp029-http-get-json), [web-search](monty.composition.recipe-implementation.md#rcp039-web-search), [code-review-pr](monty.composition.recipe-implementation.md#rcp114-code-review-pr) and other consumers of the same canonical component. These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-github-search-issues` executes this Skill's one-Tool usage: GET /search/issues?q={slot0}. slot0 = URL-encoded query. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l079-pc-github-search-issues).

**Exact command:** `run skill-github-search-issues with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-github-search-issues:** Candidate implementation `usage_l016(inputs)` (role L016); proposed public export `use_skill_github_search_issues(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_github_search_issues(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_github_search_issues(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l016(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-github-search-issues",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-github-search-issues",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR083. skill-zencoder-list-projects execution Recipe

**Existing Recipe overlap findings:** Inspect [zencoder-list-projects](monty.composition.recipe-implementation.md#rcp126-zencoder-list-projects). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-zencoder-list-projects` executes this Skill's one-Tool usage: GET /projects via host.zencoder_api. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l094-pc-zencoder-list-projects).

**Exact command:** `run skill-zencoder-list-projects with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-zencoder-list-projects:** Candidate implementation `usage_l094(inputs)` (role L094); proposed public export `use_skill_zencoder_list_projects(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_zencoder_list_projects(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_zencoder_list_projects(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l094(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-zencoder-list-projects",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-zencoder-list-projects",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR084. skill-zencoder-list-tasks execution Recipe

**Existing Recipe overlap findings:** Inspect [zencoder-list-tasks](monty.composition.recipe-implementation.md#rcp127-zencoder-list-tasks), [zencoder-list-tasks-filtered](monty.composition.recipe-implementation.md#rcp128-zencoder-list-tasks-filtered). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-zencoder-list-tasks` executes this Skill's one-Tool usage: GET /projects/{pid}/tasks[?status&limit] via host.zencoder_api. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l095-pc-zencoder-list-tasks).

**Exact command:** `run skill-zencoder-list-tasks with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-zencoder-list-tasks:** Candidate implementation `usage_l095(inputs)` (role L095); proposed public export `use_skill_zencoder_list_tasks(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_zencoder_list_tasks(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_zencoder_list_tasks(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l095(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-zencoder-list-tasks",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-zencoder-list-tasks",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR085. skill-zencoder-get-task execution Recipe

**Existing Recipe overlap findings:** Inspect [zencoder-get-task](monty.composition.recipe-implementation.md#rcp129-zencoder-get-task), [zencoder-check-solution-status](monty.composition.recipe-implementation.md#rcp131-zencoder-check-solution-status), [zencoder-update-task](monty.composition.recipe-implementation.md#rcp134-zencoder-update-task). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-zencoder-get-task` executes this Skill's one-Tool usage: GET /projects/{pid}/tasks/{tid} via host.zencoder_api. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l096-pc-zencoder-get-task).

**Exact command:** `run skill-zencoder-get-task with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-zencoder-get-task:** Candidate implementation `usage_l096(inputs)` (role L096); proposed public export `use_skill_zencoder_get_task(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_zencoder_get_task(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_zencoder_get_task(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l096(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-zencoder-get-task",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-zencoder-get-task",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR086. skill-zencoder-get-plan execution Recipe

**Existing Recipe overlap findings:** Inspect [zencoder-get-plan](monty.composition.recipe-implementation.md#rcp130-zencoder-get-plan), [zencoder-check-solution-status](monty.composition.recipe-implementation.md#rcp131-zencoder-check-solution-status). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-zencoder-get-plan` executes this Skill's one-Tool usage: GET /projects/{pid}/tasks/{tid}/plan via host.zencoder_api. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l097-pc-zencoder-get-plan).

**Exact command:** `run skill-zencoder-get-plan with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-zencoder-get-plan:** Candidate implementation `usage_l097(inputs)` (role L097); proposed public export `use_skill_zencoder_get_plan(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_zencoder_get_plan(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_zencoder_get_plan(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l097(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-zencoder-get-plan",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-zencoder-get-plan",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR087. skill-zencoder-create-task execution Recipe

**Existing Recipe overlap findings:** Inspect [zencoder-solve-coding-problem](monty.composition.recipe-implementation.md#rcp133-zencoder-solve-coding-problem). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-zencoder-create-task` executes this Skill's one-Tool usage: POST /projects/{pid}/tasks via host.zencoder_api. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l098-pc-zencoder-create-task).

**Exact command:** `run skill-zencoder-create-task with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-zencoder-create-task:** Candidate implementation `usage_l098(inputs)` (role L098); proposed public export `use_skill_zencoder_create_task(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_zencoder_create_task(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_zencoder_create_task(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l098(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-zencoder-create-task",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-zencoder-create-task",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR088. skill-zencoder-patch-task execution Recipe

**Existing Recipe overlap findings:** Inspect [zencoder-update-task](monty.composition.recipe-implementation.md#rcp134-zencoder-update-task). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-zencoder-patch-task` executes this Skill's one-Tool usage: PATCH /projects/{pid}/tasks/{tid} via host.zencoder_api. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l099-pc-zencoder-patch-task).

**Exact command:** `run skill-zencoder-patch-task with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-zencoder-patch-task:** Candidate implementation `usage_l099(inputs)` (role L099); proposed public export `use_skill_zencoder_patch_task(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_zencoder_patch_task(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_zencoder_patch_task(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l099(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-zencoder-patch-task",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-zencoder-patch-task",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR089. skill-zencoder-list-automations execution Recipe

**Existing Recipe overlap findings:** No direct consumer of this exact canonical role was found in the per-Recipe implementation appendix. Recheck the actual eligible catalogue before allocating an identity; a missing source reference is not proof that no equivalent installed Recipe exists.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-zencoder-list-automations` executes this Skill's one-Tool usage: GET /automations[?enabled] via host.zencoder_api. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l100-pc-zencoder-list-automations).

**Exact command:** `run skill-zencoder-list-automations with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-zencoder-list-automations:** Candidate implementation `usage_l100(inputs)` (role L100); proposed public export `use_skill_zencoder_list_automations(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_zencoder_list_automations(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_zencoder_list_automations(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l100(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-zencoder-list-automations",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-zencoder-list-automations",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR090. skill-zencoder-create-automation execution Recipe

**Existing Recipe overlap findings:** Inspect [zencoder-create-automation](monty.composition.recipe-implementation.md#rcp135-zencoder-create-automation). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-zencoder-create-automation` executes this Skill's one-Tool usage: POST /automations via host.zencoder_api. See [its exact component and Skill interface instructions](monty.composition.implementation.md#l101-pc-zencoder-create-automation).

**Exact command:** `run skill-zencoder-create-automation with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** **Preloadable function-interface implementation for skill-zencoder-create-automation:** Candidate implementation `usage_l101(inputs)` (role L101); proposed public export `use_skill_zencoder_create_automation(inputs)`. Apply [shared usage/export selection](monty.composition.implementation.md#shared-usage-and-export-selection): reuse an equivalent qualified usage and its declared export, or define a distinct reviewed entry point for a different contract while sharing compatible helpers. Prose creates no callable alias.

**Usage prose to implement:** Document this export's one-Tool purpose, exact finite `inputs` and return/error/wait contract, required prepared inputs and fixed/computed Tool arguments using the parent component's audited contract. Preload definitions and dependencies; invoke the export once on demand after its matching ToolSkill binding. Additional discovery, filtering, model reasoning, transformation, write or reply operations stay in the Recipe. The earlier multi-operation name does not authorize hiding those operations in this Skill. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_zencoder_create_automation(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_zencoder_create_automation(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_l101(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-zencoder-create-automation",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-zencoder-create-automation",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR091. reply:skill execution Recipe

**Existing Recipe overlap findings:** Inspect [zencoder-auth-setup](monty.composition.recipe-implementation.md#rcp132-zencoder-auth-setup), [host-post-reply](monty.composition.recipe-implementation.md#rcp139-host-post-reply), [Packaged `host-post-reply` (N01)](monty.composition.recipe-implementation.md#rcp152-packaged-host-post-reply-n01). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-reply-skill` executes this Skill's one-Tool usage: Packaged reply: reply:code / reply:skill See [its exact component and Skill interface instructions](monty.composition.implementation.md#n01-packaged-reply-replycode--replyskill).

**Exact command:** `run reply:skill with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** Reuse the associated export and exact reviewed profile described in this component section. A preparation-only body remains BLOCKED until its actual one-Tool implementation is qualified. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_reply_skill(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_reply_skill(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_n01(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-reply-skill",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-reply-skill",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR092. history:skill execution Recipe

**Existing Recipe overlap findings:** Inspect [host-save-history](monty.composition.recipe-implementation.md#rcp140-host-save-history), [Packaged `host-save-history` (N03/N02)](monty.composition.recipe-implementation.md#rcp153-packaged-host-save-history-n03n02). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-history-skill` executes this Skill's one-Tool usage: Packaged history writer: history:code / history:skill See [its exact component and Skill interface instructions](monty.composition.implementation.md#n02-packaged-history-writer-historycode--historyskill).

**Exact command:** `run history:skill with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** Reuse the associated export and exact reviewed profile described in this component section. A preparation-only body remains BLOCKED until its actual one-Tool implementation is qualified. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_history_skill(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_history_skill(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_n02(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-history-skill",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-history-skill",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR093. skill-read-file-interval execution Recipe

**Existing Recipe overlap findings:** Inspect [file-read-range](monty.composition.recipe-implementation.md#rcp002-file-read-range), [file-read-head](monty.composition.recipe-implementation.md#rcp023-file-read-head), [file-read-tail](monty.composition.recipe-implementation.md#rcp024-file-read-tail). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-skill-read-file-interval` executes this Skill's one-Tool usage: Typed interval usage See [its exact component and Skill interface instructions](monty.composition.implementation.md#n05-typed-interval-usage).

**Exact command:** `run skill-read-file-interval with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** Reuse the associated export and exact reviewed profile described in this component section. A preparation-only body remains BLOCKED until its actual one-Tool implementation is qualified. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_skill_read_file_interval(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_skill_read_file_interval(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_n05(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-skill-read-file-interval",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-skill-read-file-interval",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR094. typed-json-parse-usage execution Recipe

**Existing Recipe overlap findings:** Inspect [file-patch](monty.composition.recipe-implementation.md#rcp016-file-patch), [http-get-json](monty.composition.recipe-implementation.md#rcp029-http-get-json), [web-search](monty.composition.recipe-implementation.md#rcp039-web-search), [json-parse](monty.composition.recipe-implementation.md#rcp102-json-parse) and other consumers of the same canonical component. These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-typed-json-parse-usage` executes this Skill's one-Tool usage: Typed JSON parse usage See [its exact component and Skill interface instructions](monty.composition.implementation.md#n06-typed-json-parse-usage).

**Exact command:** `run typed-json-parse-usage with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** Reuse the associated export and exact reviewed profile described in this component section. A preparation-only body remains BLOCKED until its actual one-Tool implementation is qualified. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_typed_json_parse_usage(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_typed_json_parse_usage(inputs)
```

This invocation uses a proposed export, not an implemented alias for `usage_n06(inputs)`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-typed-json-parse-usage",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-typed-json-parse-usage",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR095. immutable-candidate-admission-usage execution Recipe

**Existing Recipe overlap findings:** Inspect [Prefix Redesign (planned role)](monty.composition.recipe-implementation.md#rcp164-prefix-redesign-planned-role), [Prefix Create (planned role)](monty.composition.recipe-implementation.md#rcp165-prefix-create-planned-role), [Prefix model-assisted design (planned role)](monty.composition.recipe-implementation.md#rcp166-prefix-model-assisted-design-planned-role). These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-immutable-candidate-admission-usage` executes this Skill's one-Tool usage: Immutable review submission See [its exact component and Skill interface instructions](monty.composition.implementation.md#n16-immutable-review-submission).

**Exact command:** `run immutable-candidate-admission-usage with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** Reuse the associated export and exact reviewed profile described in this component section. A preparation-only body remains BLOCKED until its actual one-Tool implementation is qualified. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_immutable_candidate_admission_usage(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_immutable_candidate_admission_usage(inputs)
```

This invocation uses a proposed export, not an implemented alias for `the qualified actual one-Tool function; currently BLOCKED`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-immutable-candidate-admission-usage",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-immutable-candidate-admission-usage",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR096. catalogue-export-usage execution Recipe

**Existing Recipe overlap findings:** Inspect [host-assemble-prefix-bundle](monty.composition.recipe-implementation.md#rcp151-host-assemble-prefix-bundle), [Prefix Generate / Regenerate (planned role)](monty.composition.recipe-implementation.md#rcp162-prefix-generate--regenerate-planned-role), [Prefix composite build / rebuild (planned role)](monty.composition.recipe-implementation.md#rcp163-prefix-composite-build--rebuild-planned-role), [Prefix Redesign (planned role)](monty.composition.recipe-implementation.md#rcp164-prefix-redesign-planned-role) and other consumers of the same canonical component. These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-catalogue-export-usage` executes this Skill's one-Tool usage: Catalogue sweep and export See [its exact component and Skill interface instructions](monty.composition.implementation.md#n17-catalogue-sweep-and-export).

**Exact command:** `run catalogue-export-usage with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** Reuse the associated export and exact reviewed profile described in this component section. A preparation-only body remains BLOCKED until its actual one-Tool implementation is qualified. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_catalogue_export_usage(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_catalogue_export_usage(inputs)
```

This invocation uses a proposed export, not an implemented alias for `the qualified actual one-Tool function; currently BLOCKED`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-catalogue-export-usage",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-catalogue-export-usage",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR097. compiler-worker-usage execution Recipe

**Existing Recipe overlap findings:** Inspect [host-assemble-prefix-bundle](monty.composition.recipe-implementation.md#rcp151-host-assemble-prefix-bundle), [Prefix Generate / Regenerate (planned role)](monty.composition.recipe-implementation.md#rcp162-prefix-generate--regenerate-planned-role), [Prefix composite build / rebuild (planned role)](monty.composition.recipe-implementation.md#rcp163-prefix-composite-build--rebuild-planned-role), [Prefix Redesign (planned role)](monty.composition.recipe-implementation.md#rcp164-prefix-redesign-planned-role) and other consumers of the same canonical component. These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-compiler-worker-usage` executes this Skill's one-Tool usage: Registered compiler worker invocation See [its exact component and Skill interface instructions](monty.composition.implementation.md#n18-registered-compiler-worker-invocation).

**Exact command:** `run compiler-worker-usage with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** Reuse the associated export and exact reviewed profile described in this component section. A preparation-only body remains BLOCKED until its actual one-Tool implementation is qualified. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_compiler_worker_usage(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_compiler_worker_usage(inputs)
```

This invocation uses a proposed export, not an implemented alias for `the qualified actual one-Tool function; currently BLOCKED`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-compiler-worker-usage",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-compiler-worker-usage",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.

## SKR098. prefix-generation-publication-usage execution Recipe

**Existing Recipe overlap findings:** Inspect [host-assemble-prefix-bundle](monty.composition.recipe-implementation.md#rcp151-host-assemble-prefix-bundle), [Prefix Generate / Regenerate (planned role)](monty.composition.recipe-implementation.md#rcp162-prefix-generate--regenerate-planned-role), [Prefix composite build / rebuild (planned role)](monty.composition.recipe-implementation.md#rcp163-prefix-composite-build--rebuild-planned-role), [Prefix Redesign (planned role)](monty.composition.recipe-implementation.md#rcp164-prefix-redesign-planned-role) and other consumers of the same canonical component. These are source-level reuse candidates; compare actual selectors, prerequisites, extra effects, output ownership and retained variants before reusing. Sharing a code component alone does not establish Recipe equivalence.

**Component and purpose:** Proposed/reused mcp-call-skill-recipe `execute-prefix-generation-publication-usage` executes this Skill's one-Tool usage: Prefix generation publication See [its exact component and Skill interface instructions](monty.composition.implementation.md#n20-prefix-generation-publication).

**Exact command:** `run prefix-generation-publication-usage with %`. The only variable position is `%`, immediately after ` with `; supply the JSON object of this Skill's declared local inputs. tools/list must derive this sentence and every concrete variable field from the qualified execution Recipe variant and its linked Skill contract/examples; the Skill row alone does not produce an entry. Do not interpolate these values into Python.

**Implementation/reuse disposition:** Reuse the associated export and exact reviewed profile described in this component section. A preparation-only body remains BLOCKED until its actual one-Tool implementation is qualified. Review the existing Recipe list for a compatible one-usage variant; mark both identities for duplicate review and reuse its canonical Recipe UUID if input/result/effect/continuation contracts agree. Narrower selectors/results need qualification rather than automatic merging. No new Recipe UUID is allocated here.

**Concrete assembly:** Decode and validate the whole input object into the linked finite contract before effects. Resolve the selected qualified export (proposed name `use_prefix_generation_publication_usage(inputs)`) through agreeing code/interface records and its exact dependency graph, preload definitions with zero dispatch, then bind the matching ToolSkill and invoke the class-22 export. If this is a split legacy workflow, expose only its qualified one-Tool portion; preparation/filtering/transforming remain separate pure Recipe steps. The source body's former workflow name does not broaden the Skill.

**Invocation example — internal Recipe code, never MCP source:**

```python
result = use_prefix_generation_publication_usage(inputs)
```

This invocation uses a proposed export, not an implemented alias for `the qualified actual one-Tool function; currently BLOCKED`. Reuse the actual canonical export for an equivalent usage, or implement/declare the distinct entry point under the shared selection rules before using this example. The retained compiler resolves the selected symbol; no runtime name lookup is permitted.

**Non-runnable usage-core IBS skeleton — use the existing complete execution Recipe/variant and its normal chat result/reply ownership; this binding/invocation core alone is not chat completion:**

```json
{
  "step_descriptions": [
    {
      "label": "execute-prefix-generation-publication-usage",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Prepare the matching retained usage binding",
          "content": "Binding only; no dispatch or permission grant",
          "type": "component",
          "include": [
            "<RESOLVED_COMPATIBLE_TOOLSKILL_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Invoke the selected preloaded Skill export",
          "content": "Call the pinned canonical export with typed local data and return its declared result",
          "type": "component",
          "include": [
            "<RESOLVED_ASSOCIATED_PYTHONCODE_UUID>"
          ],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "execute-prefix-generation-publication-usage",
      "description": "One existing Skill invocation with its declared typed inputs/result",
      "step_link": "0:1-0:E",
      "intent_examples": [],
      "variable_patterns": []
    }
  ]
}
```

Complete real capture patterns/at least ten meaningful positive and negative intent examples, finite recursive contracts, retained input/export selection, exact UUIDs, supported binding records and metadata through the actual store/runner before activation. Empty lists and UUID markers are unresolved work, not permission to insert the skeleton. The command sentence belongs to the variant's supported template metadata, not a fabricated extra JSON field.

**Completion/security/acceptance:** Feed the actual typed usage result/error/wait into its declared ordinary chat completion and public response formatter/reply owner; MCP forwards the correlated posted terminal response, then closes that chat. Do not add a direct result bridge or duplicate posting/history effects. Preserve dispatch/effect uncertainty, durable attempt counts, no-replay behavior, current policy/cancellation and task/child isolation. Verify valid and cross-Skill/invalid commands, hostile strings as data, fixed selectors, old/new export coexistence, effect-free preload, repeated mutable-data freshness, concurrent requests, waits and lost replies. An ordinary invocation creates no component or new Q1/Q2 record. Internal/protected or unresolved profiles remain unlisted/blocked.
