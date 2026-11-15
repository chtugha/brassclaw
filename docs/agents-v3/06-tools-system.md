# 06 — Tools, native implementations and live authority

## 1. Purpose

A Tool (class 0) is a registered Rust primitive performing one declared operation
when called through a supported host boundary. It returns a result, classified
failure or supported wait/handle. It does not select the task workflow. Recipes
and pinned PythonCode own sequencing; a ToolSkill is effect-free binding metadata.
[tools.md](../../tools.md) and [toolskills.md](../../toolskills.md) govern these
contracts. This source map was checked on 2026-11-15.

Start capability design with a Recipe and reuse the library. New Rust is justified
by a genuinely missing primitive or necessary shared runtime/persistence/binding/
lifecycle/kernel support, not by hiding a multi-Tool workflow in one operation.
A new ToolSkill normally needs metadata validation rather than compilation.
Compiling a Tool proves neither registration, approval, loading nor permission.

## 2. Location — verified sources

| Source | Symbols / role |
| --- | --- |
| [pg_tool_store.rs](../../crates/brassclaw_reborn_composition/src/pg_tool_store.rs) | `NewPgTool`, `PgToolStore` and current mutable Tool projection fields. |
| [first_party_tools/mod.rs](../../crates/brassclaw_host_runtime/src/first_party_tools/mod.rs) | First-party handler modules and registration support. |
| [host.rs](../../crates/brassclaw_capabilities/src/host.rs) | `CapabilityHost` dispatch/admission boundary. |
| [instance_policy.rs](../../crates/brassclaw_authorization/src/instance_policy.rs) | `LiveInstanceToolPolicy`, `InstanceToolPolicySnapshot` and `InstanceToolAuthorizer`. |
| [retained_tools.rs](../../crates/brassclaw_engine/src/memory/retained_tools.rs) | `RetainedToolBinding` and `prepare_retained_tool_program`. |
| [authored_monty_catalogue.rs](../../crates/brassclaw_reborn_composition/src/authored_monty_catalogue.rs) | `verify_implementation` and `verify_packaged_task_implementation` check selected registration and actual implementation identity/integrity. |
| [retained_recipe.rs](../../crates/brassclaw_engine/src/executor/retained_recipe.rs) | `RetainedRecipeExecution` checks the selected step and actual bound Tool call. |

## 3. Identity, implementation, binding, approval and permission

| Fact | What it establishes | What remains separate |
| --- | --- | --- |
| Stable Tool UUID | Component identity | Exact revision, implementation and permission. |
| Immutable revision/checksum | Selected definition bytes | Actual executable identity and registration. |
| `capability_id` / callable | Named dispatch surface | Trusted mapping to the Tool and selected artifact. |
| ToolSkill binding | Compatible declared availability | Dispatch, approval and permission. |
| Component/usage approval | Qualified reviewed subject | Workflow publication and current Tool policy. |
| Live allow decision | Current instance permission | Inputs, external auth, sandbox, freshness and completion. |

`validation_status` is component lifecycle metadata, **not the Tool's live
allow/block setting**. A validated Tool can be blocked; binding or previous
permission cannot authorize its next call. The global policy must cover every
retained version and alias of one stable Tool. Missing, conflicting or ambiguous
identity/policy mappings fail closed.

The legacy SQL `NewPgTool` shape cannot express the whole immutable implementation
contract. That is not a blanket absence of artifact checking: current retained
catalogue preparation verifies the Tool row/capability mapping against the retained
Tool UUID, reads its implementation declaration and checks the actual native
artifact. Task-bound reply/model adapters instead verify their exact packaged
adapter declaration and captured executable checksum through
`verify_packaged_task_implementation`. Neither path produces approval or permission.
Broader artifact retention across upgrades still needs its applicable lifecycle
acceptance; a metadata checksum or nonempty path alone is insufficient.

## 4. Preparation and dispatch

1. Select a coherent active approved Recipe graph with exact versions,
   dependencies, usage evidence and implementations before effects.
2. A Rust `knowledge` step references one ToolSkill UUID. Retained preparation
   checks `tool-skill-binding/1`, Tool dependency, callable/capability identity
   and the unique matching Skill/code association. It performs no operation.
3. Its immediately following executable step references one class-22 UUID.
   Monty invokes its pinned preloaded export using typed local inputs.
4. Before each actual call, native enforcement checks current instance-wide Tool
   policy, technical parameters, cancellation and fresh execution ownership.
   External-service authentication, network/secret/filesystem/sandbox controls
   and resource limits remain effective.
5. Native execution returns its actual result/failure. Preserve confirmed or
   unresolved effect status before deciding continuation or completion.

The instance-token operator has no additional final-v3 tenant/project/feature-role
Tool checks. Run/attempt claims fence stale execution and replies; they are not
Tool grants. Effect classification describes technical effects, not a new `Ask`
or invocation-approval lease. Existing scoped/operation-approval compatibility
paths remain until their coordinated cutover, never as new v3 requirements or
as permission to bypass technical controls. This does not certify that every
legacy host operation has completed that cutover.

## 5. Failures, resources and recovery

Ordinary retained execution currently admits one bound Tool call per executable
step and `stop` / `max_attempts: 1`. Its preparation rejects legacy binding
parameters and Ignore/Retry/Fallback policies. Independent calls need separate
binding/execution pairs. A prose dependent-chain exception does not extend this
runner. See [the exact retry contract](../../skills.md#exact-failure-and-retry-contract)
before enabling any broader profile: positive counts include initial dispatch,
survive waits/reclaim and require explicit eligible outcomes plus verified
read-only/durable deduplication evidence. Keep original keys/arguments. Never
repeat a completed effect after output validation, a later step or reply failure.
A timeout or missing acknowledgement is not evidence of no effect.

Kernel/infrastructure may deny, fence, cancel, account, supervise and settle work
without a new business step. These responsibilities do not form a second workflow
loop or a closed list of four legacy exceptions. Only actual No-Match enters
Tier 2; matching/composition and begun Recipe failures remain errors.

The binding resource contract is [Simplified Phase 3a](../../simplified_v3.md#phase-3a--globalen-monty-beim-boot-starten-und-dauerhaft-betreiben):
600 executing VM seconds per task by default, preserving consumption across nested
execution/continuations and excluding idle/queue/external waits. Global Monty
lifetime, external deadlines and shared heap limits are separate. Token budgets
default to disabled; accounting and technical model limits remain. Startup/manual
memory has a 512 MiB floor, with desired/effective revisions and actual runtime
acknowledgement. See [12-agent-loop.md](12-agent-loop.md) for the lifecycle map.

Fatal recovery must fence the old generation and reconcile native effects before
supervised replacement. VM memory is not a checkpoint. Do not replay whole Recipes,
create parallel global VMs or claim exactly-once external execution without a
supported operation contract. Preserve original artifacts while task/checkpoint
references need them.

## 6. Approval, compatibility and remaining work

Installed authored changes need applicable Q1, observed behavior and human Q2.
The own-functional A successor reviews changed functions with approved dependency
context; current exact authored readers retain their supported meaning until
connected cutover. S separately qualifies the Tool/native implementation, then
ToolSkill, then Skill/code and consuming workflows with noncircular integrity/
behavior evidence. `source='system'` never supplies provenance by itself.
See [05-skills-system.md](05-skills-system.md) for the exact distinction.

Legacy `compose_program`, `PgCompositionPort` and cdylib `rust_directives` are
compatibility paths, not proof that every ToolSkill loads a handler or the target
to activate for ordinary global tasks. Retained typed preparation, association and
implementation checks already exist; general library migration, successor
approval/publication and durable recovery remain with existing owners. The
[implementation index](../implementation-status.md) owns scoped acceptance.

## 7. LLM-relevant summary

A Tool is one native primitive. Its stable identity, selected actual implementation,
binding, approval and live permission are distinct. ToolSkill preparation dispatches
nothing. Monty invokes exact exports; the kernel checks current global policy and
technical freshness on every call. Preserve real effect certainty. The ordinary
retained runner is single-call and stop-only; legacy approval gates, source
substitution and wall-clock VM budgets are not the binding v3 design.
