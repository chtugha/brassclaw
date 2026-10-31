# Central development build queue

The leading Root/Auth chat owns all Cargo execution on the shared checkout and
canonical target. Other chats send requests to it; build windows are not handed
out. Only the leading chat edits this queue. The
[development policy](development-policy.md#shared-source-and-central-build-ownership-2026-10-30)
defines source isolation, review and evidence reuse. This file adds no product
queue, process, runtime API or additional target directory.

Before implementation, send the behavior, existing plan owner, paths and reused
interfaces. Flag changes to shared contracts so they are implemented once.
For validation, send the bounded patch/current before-and-after hashes,
packages/features, necessary checks, worker rebuild requirements and applicable
earlier evidence. Root assigns integration order and sends back the outcome.

During a build, captured source/dependencies stay fixed. Prepare changes as
bounded private patches if their paths intersect that input set. Root reviews
and applies them to the current tree after the batch; no alternate build tree,
target, Cargo execution or old-source restoration is permitted.

States are **Proposed**, **Ready**, **Building**, **Needs repair** and **Passed**. A ready
request can be combined with compatible work in one verified batch. Passing
results retain their exact source/image scope; a partial check never closes an
entire implementation plan. Failed attempts remain recorded.

After a chat/app restart, resume this queue by checking the original runner,
common lock and captured source/evidence first. A Building row alone proves
neither that a process is still running nor that its checks passed. Do not start
a replacement while the original is active. A different leading builder needs
an explicit human reassignment; chats do not transfer ownership among themselves.

| Request | Owner and scope | State | Evidence / next action |
| --- | --- | --- | --- |
| BUILD-026 | Root, actual CI handoff identity and Docker build inputs | Passed | Local:279 default/native+367 exact CI-profile cases,32 architecture,5 pure helper/0ignored, connected strict. Original105 host cases and262 Reborn library pass with root-llm-provider. Shared canonical stored-profile projection preserves all other custody/stop checks. Docker complete vendor/scripts, locked Chef0.1.78 and semantic edition migration; post-terminal Docker-only panic=abort removal preserves unwinding. [Receipt](validation/2026-10-31-ci-handoff-repair.json)3281e06a…; Root336 SHA646d3d90…, workerbe0f unchanged; both original runners exited/lock independently acquired. Actual Docker/GitHub acceptance PENDING; original690 CI terminal failed Docker/Reborn/rollup, other cancelled packages not passes. Root retains Cargo. |
| BUILD-025 | Root, existing K3 OAuth browser observation | Passed |7 paths/8241 inputs;26 exact-input Node cases,28 actual embedded-asset cases and1 native HTTP/PG/Chromium case/0ignored, fresh images239.4s and static strict pass. Real ticking blocked-popup observer and confirmed-terminal cleanup retain original/custody/unknown assertions. [Receipt](validation/2026-10-31-oauth-observation.json)71e5b8fa…; Root327 SHA448d35e9…, workerbe0f unchanged, original28981 exited/common lock independently free. Initial finalizer emitted no receipt; independent retry verified without rerunning tests. Sourcefreeze released, Root retains Cargo; remaining OAuth/K3–K7 open. |
| BUILD-024 | Root, existing K3 manual setup lifetimes | Passed |14 paths/8239 captured inputs; fresh compilation256.7s,46 native+32architecture/0ignored, actual PG/FS/HTTP/manual/resource/global consumers and connected/default strict pass. V195/positive u32 defaults600/300, original first expiry across live edits/CAS, expired original rejection and ENDE/effective controls proven. [Receipt](validation/2026-10-31-manual-setup-lifetimes.json)1b983910…. Root321 SHA40febe75…, fresh workerbe0f0d6e… verified; original23514 exited and lock independently acquired. Sourcefreeze released, Root retains Cargo. OAuth/client/gate/popup/transport/ingress and complete K3–K7/plans remain open. |
| BUILD-023 | Exact, existing Prefix I3 independent design acceptance | Proposed | Private009-pair-only-acceptance/proposal.md under /private/tmp/brassclaw-exact-bootstrap-packet-20261031; SHA03e3c6c7…/eight preconditions c09d8027…. Review actual V183 pair-only evaluation anchor, V184 generation-only reader, existing writer/caller/original retry/receipt/GC together before code. No new approval table or migration assumed; a Pair ID must never occupy PrefixAcceptanceReceipt.generation_qualification or generation-only approved_retry. Missing native/Pure/S/Behavior/Serving gates remain prerequisites, not an activation grant. Other003–008 proposals stay private; Root sole Cargo. |
| BUILD-022 | Root, existing K3 step6 per-call Auth backend wait | Passed |21 paths; R1 clean compilation257.1s plus40 native cases reused against exact unchanged Rust/image/worker. R2 script-only restore registry/enrollment correction preserves equality/two writes; corrected resource browser, global embedded/base/Sempai and architecture pass:42 native+32architecture/0ignored. Connected strict106.0s/default strict50.7s pass. [Receipt](validation/2026-10-31-auth-backend-wait.json)92a81859…; Root319 SHA831ff4bd…, fresh worker161293a4… verified, original runner exited/common lock independently acquired. No full lifecycle/K3/platform/approval claim. Sourcefreeze released, Root retains Cargo. |
| BUILD-021 | Root, native Chromium Tool-limit clear regression | Passed |R4 corrected Tool-policy input104.46s and R5 original resource-controls111.95s:2 real native browser cases/0ignored. Pointer focus loss and failed native SelectAll actually reproduced; final nonempty invalid-space fill/Backspace preserves every null-write/revision/BigInt/409/lost-ACK/no-replay assertion. Pinned native CLI outer deadline exercised by failed R3; original failures/cleanup retained. No Rust compilation for JS. [Receipt](validation/2026-10-31-native-browser-limit-clear.json) c14fdf2f….313 reviewed Root paths SHAafd6e5fd…, worker227a4b… unchanged; original processes exited/common lock acquired. Source freeze released, Root retains Cargo. |
| BUILD-020 | Root, actual checkpoint CI repairs after app update | Passed |23 captured package paths/8233 named inputs.125 native +32architecture +24tagged Monty +1legacy cases/0ignored, fresh connected compilation and connected/isolated strict pass. Real workflow lint/33 shell cases pass. Original selector/upstream-driver/disk failures retained; R3 tagged success and R4 fast repeat counted once. Actual images/worker verified before required package-only cleanup; original runner exited and common lock independently available. [Receipt](validation/2026-10-31-checkpoint-ci-repairs.json) fa2c25d1…. No new GitHub/platform/full-plan/activation claim. |
| BUILD-001 | Root, Simplified K3 step6: DCR gate preparation before missing Auth flow | Passed | Run2:34 distinct DCR cases/0ignored, connected/default strict and compilation passed. Both native fixtures explicitly select PostgreSQL Auth; R1 remains failed. [Receipt](validation/2026-10-30-dcr-gate-recovery.json). |
| BUILD-002 | Exact, I2/P03/§5.2: qualified retained PrefixFreeWorkflow consumers | Proposed | Consume scoped BUILD-005/006/007 producer/strict usage, BUILD-008 shared capture and BUILD-011/012 current pre-primary native custody/accounting. Actual sealed source/closed qualification, Free-specific native required-edge/positive-output binding and real technical Serving reader remain. Private projection/reader reviewed; no standalone fourth borrowed DTO. No widening/activation; final additive migration number assigned only at integration. |
| BUILD-003 | Root, K3 step6: acknowledged DCR metadata-cache lifetime | Passed | V190 and ten connected sources:548 distinct tests/0ignored, actual worker/global/browser, neutral/connected/default strict passed. [Receipt](validation/2026-10-30-dcr-metadata-cache.json). Setup lifetimes/backend/browser/ingress remain open; Root retains Cargo. |
| BUILD-004 | Root shared admission owner, Prefix I1 prerequisite: actual Tier1 ResultOnly work | Passed | Thirty sources/V191; Run6 eleven cases reused with exact inputs/images/logs, Run7 child case plus connected/default strict pass:12 distinct/0ignored. Original global/PG/SDK ResultOnly and legacy consumers accepted; failed attempts retained. [Receipt](validation/2026-10-30-internal-model-admission.json). Later BUILD-008/011/012 cover contained capture and current pre-primary review custody/accounting; actual Free/full I1I2 remain open. |
| BUILD-005 | Exact output-producer prerequisite, Root integration/review | Passed | Fifteen sources, Run3 compilation and70 fresh/4 unchanged reused cases/0ignored; actual global/SDK/PG/final-counter and enabled-limit/no-clamping acceptance, all60 gateway regressions, connected/default/retained strict pass. R1 selector and R2 two gateway failures retained; Root repaired both without weakening target enforcement. [Receipt](validation/2026-10-30-original-model-output.json). Full I2/Free remains open; Root retains Cargo. |
| BUILD-006 | Exact, bounded native strict-JSON prerequisite | Passed | Four sources, Run2 actual native identity/retention/current policy/bounds, schema and JSON/surface consumers:11 fresh/0ignored, compilation/format/connected/default strict pass. R1 test-only wrong descriptor field repaired through existing schema/identity APIs; failed diagnostic retained. [Receipt](validation/2026-10-30-native-strict-json.json). No new activated usage or Free/workflow approval; Root retains Cargo. |

| BUILD-007 | Exact, standalone strict-JSON Skill prerequisite; Root integration | Passed | Three reviewed/rebased sources;38 distinct tests/0ignored, actual retained/native/PG raw/export boundary and denial, current global SDK ResultOnly,34 service cases, fresh worker build and connected/default strict pass. Canonical fresh worker bytes remain227a4b…. [Receipt](validation/2026-10-31-standalone-strict-json.json). No activation or Free proof. |


| BUILD-008 | Root shared capture owner; Exact Catalogue/Bootstrap carrier coordination | Passed | Six Composition sources; Run2 compilation, nine distinct tests/0ignored and connected/default strict pass. Actual original contained/global/selected/child/pure paths, capture-before-binding and foreign/re-entry fences accepted. Existing private snapshot inspector supplies persisted lineage; Run1 diagnostics retained. [Receipt](validation/2026-10-31-shared-model-capture.json). Actual sealed source and independent mid-turn Sempai custody remain open; no successful proof or live parallel VM. |
| BUILD-009 | Root, existing composition compatibility acceptance | Passed | Final31-path Run6: fresh compilation,24 distinct tests/0ignored, connected and standalone all-targets strict, format/CI/YAML/scope pass. All22 original model-host cases retained plus extracted native invalid-reply test; actual coherent prepare/bind/named selections and one shared data parser, native seal separate. No dummy calls/suppression/fallback/activation. Prior diagnostics retained; only disk-triggered Composition package cache cleaned with needed images preserved. [Receipt](validation/2026-10-31-model-host-compatibility.json). Root retains sole Cargo. |
| BUILD-010 | Existing component/library compatibility owner | Proposed | Read-only inventory corrects stale DocType comment: actual deprecated v2 CodeSnippet/V2SkillMetadata are referenced internally, Engine forces v2-compat on its Skills dependency, and both crates/skill_tracker suppress deprecated warnings. Trace actual importer/telemetry consumers and retire bridge/suppressions through the existing library migration, without replacement aliases or hidden warnings. |
| BUILD-011 | Root shared native model-effect custody; Prefix consumes its original binding | Passed | Eleven sources/V192; Run2 compilation,10 distinct tests/0ignored, format and connected/default/neutral strict pass. Actual pre-primary related request/wire/provider/loop custody, independent outputs, failed-write recovery after cancellation and immutable/no-replay addresses; full request stays in PG, only digest in RAM. [Receipt](validation/2026-10-31-related-model-custody.json). Run1 broad selector's separate ignored real-inference gate remains unverified. No post-primary/Free/full I1I2 claim. |
| BUILD-012 | Root shared model accounting owner | Passed | Two sources; compilation, five distinct tests/0ignored and connected/default strict pass. Actual Governor/PG/native/SDK distinguishes pre-submit BudgetExceeded (both reservations released, no request) from observed Length BudgetExceeded (only primary released; related hold retained). Native IDs, dispatch counts, original finish reason and balances checked; existing custody/output/protocol pass. [Receipt](validation/2026-10-31-review-unsubmitted.json). In-memory governor proves caller semantics, not durable recovery/billing; no new store, category-derived proof or retry. |
| BUILD-013 | Exact source test fix, Root integration | Passed | Existing unittest.main moved after the second test class; unchanged23 test bodies. Actual `python3 -B scripts/prefix/test_bootstrap_answer.py` runs23/0skipped, including eight previously bypassed Prefill cases. No Cargo/product/approval change. Before d842b5ef… -> after a4c35b57…; package /private/tmp/brassclaw-bootstrap-test-entrypoint-current.txt, result b7f55bff…. Source-only Python coverage, not native/Q1/Free evidence. |
| BUILD-014 | Root shared review/model-input preparation | Passed | Run1 fresh compilation, nine distinct tests/0ignored, format and connected/default strict pass. Real PG/IBS/parser/restore checks six original binding forms and checksum-correct forgeries; original selections survive successors, zero approval/admission/effect rows. No new authority/Free approval. [Receipt](validation/2026-10-31-review-model-selection.json), SHA347be24f…;7996 inputs/worker227a4b… independently verified, original processes gone/lock free. Root retains Cargo. |

| BUILD-015 | Authorized definition chat, Root integration: A/S functional approval target | Passed |21 text sources integrated; Run2 compilation, three native cases/0ignored and connected strict pass. Current installation/reference/global deployment owner covers actual guide bytes,319 release sources for base/Sempai, old profiles/overrides; no new A runtime/activation. Receipt b78e1f69…;8176 inputs incl484 discovered literal embedded files, Root271/worker227a4b… verified; original processes exited/lock free. R1 pre-Cargo prose-label preflight failure retained. Native Transformers5.1.0 config warning remains visible and is the next Root repair. [Receipt](validation/2026-10-31-validation-target-documents.json). |

| BUILD-016 | Definition chat, actual BUILD014 metadata documentation | Passed | Two reviewed text corrections integrated and checked in017: optional model_input_binding beside association_approvals, exact original V150 restoration and absent/Default distinction; no API/format/authority. Patch e731ecf5…, manifest53fe913f…; one release-embedded path checked through the actual native consumer. [Shared receipt](validation/2026-10-31-model-oracle-runtime.json). |

| BUILD-017 | Root, offline independent model oracle dependency; batches016 | Passed | R4: four explicit offline Rust/HF cases and actual native/global base/Sempai consumer pass,0ignored; exact unchanged R1/R3 compilation/connected strict reused. Actual original typed config/backend supported; raw counting preserves oversized measurement and context rejection without wrapper warnings or suppression. Nine source paths afterd116810c…;8177 named inputs/Root278/worker227a4b… independently verified, original processes exited/lock free. R1 diskstop/R2 Path preflight retained; only991.6MiB required package cleanup. [Receipt](validation/2026-10-31-model-oracle-runtime.json) dc2da85e…; no inference/platform deployment/approval/Free claim. Root retains sole Cargo. |

| BUILD-018 | Root, Validator Step2 own-function subject prerequisite | Passed |16 sources after16b87c3f…; R3 fresh compilation,15 distinct native tests/0ignored (14review incl5new plus actual global/base/Sempai consumer),7 UI/0skipped and connected strict for Composition/Skills/Product/Engine pass. V193/strict /3 atomic changed pair or exact context, one V149 intent, complete typed /2 input and partial-preparation rejection accepted. R1 wrong-base400vs409 repaired for both roots; R1/R2 retained, only required disposable caches reclaimed with images/worker preserved.8180 named inputs/Root287/worker227a4b… independently verified; original processes exited/lock free. [Receipt](validation/2026-10-31-joint-function-subject.json)6ad7eea4…. No joint verdict/Q1/Q2/publication/Free. Root retains Cargo. |

| BUILD-019 | Root, Validator Step7 actual checking components | Passed |19 paths after51fb1adc…/patch02cfff7b…; R1 compilation241.4s and6 native cases reused with exact unchanged inputs/image/runtime, R2 remaining4 cases plus connected strict102.9s pass:10 distinct native/0ignored. Wrong R1 selector preserved; actual compiled listing supplied the correction. Shared private reader/all5 consumers and Skill-interface/recursive/export checks accepted. Receipt [30f6f809…](validation/2026-10-31-joint-validator-checks.json);8183 named inputs/Root301/worker227a verified, original processes exited/common lock available. No complete Q1/Q2/behavior/semantic/publication/Pure/S/Free grant; partial-preparation denial unchanged. Historical batch source freeze released; app update subsequently confirmed complete. Current BUILD020 freeze applies. Root retains Cargo. |

Incoming Exact manual setup HTTP/WebUI already passed before this queue was
introduced. Preserve its [terminal receipt](validation/2026-10-30-exact-model-setup-ui.json)
and implemented sources. It is not a new request to repeat those checks.

The original documentation chat reports no pending implementation/build requests
and is being retired at the operator's request. This does not retire Exact K3-2
or BUILD-002. Its changed embedded/teaching inputs remain covered by the existing
K5/K6 integrity, uptake, override and retained-artifact acceptance; K7 owns exact
evidence attribution. Do not add a separate closing Exact gate or duplicate build.

BUILD-002 interface decisions, 30 October:

- Catalogue/bootstrap owns the opaque qualification. Existing capture traits may
  return the already selected proof through their existing error boundaries;
  synchronous getters perform no DB I/O or latest lookup. Ordinary capture-only
  data never becomes approval. Required child/review membership is checked for
  the actual selected occurrence, not every program in a captured view.
- `TaskFinalModelRequest.request_output_tokens: Option<NonZeroU32>` is acceptable
  as a native request fallback. Prefix keeps its accepted default. Free may use
  a positive original qualified workflow/profile default; `None` requires a real
  positive SDK cap from an existing connected producer. Show that source with
  discretionary token budgets disabled and without an accepted Prefix. Missing/
  zero remains a pre-send error; no artificial default or new request DTO.
- Preserve observation bytes and checksums exactly, including native payload
  format markers. Version new storage rows/binding metadata separately; a /2
  storage row may contain the unchanged /1 observation under explicit versioned
  constraints. Do not rewrite even a length-neutral format marker. Old /1 rows,
  immutable/capacity guards, and original invocation/claim custody remain intact.
- Actual additive task-provider-binding/evidence support needs a forward
  migration assigned by Root in integration order. The initial V190 reservation
  is withdrawn before any source file or applied migration exists: the selected
  Refinery rejects a newly introduced missing lower version after a higher one
  has run. Never create placeholder SQL or weaken that check. Private proposals
  leave their final number unassigned. Inspect all constraints/readers; readers must
  distinguish versions before decoding nullable Prefix fields. No historical
  migration or evidence byte changes, new approval store, or ignored old reader.
- The actual model prompt profile may gain optional positive
  `max_output_tokens` without a default. The bootstrap producer requires its
  admitted typed value. Existing custody traits may expose that exact retained
  value through default-None `requested_output_tokens`; the existing gateway and
  cost quote consume it. One shared native Prompt parser owns validation,
  distinguishing missing from null/zero. Preserve existing approved revisions;
  successor contracts/associations require their exact existing qualification.
  Show disabled-budget, live-rejection, child and native SDK acceptance before
  Ready; model quote defaults never become output caps.
- Catalogue/Bootstrap may retain one private non-Serde execution carrier with
  distinct successful `QualifiedWorkflow` and `ContainedQualificationAttempt`
  construction paths. The latter binds the exact still-unapproved closed review
  Subject and original attempt, never ordinary activation or a successful stored
  proof. The ordinary captured-catalogue getter remains successful-proof-only;
  `InternalRecipeSource` may return its already selected native binding via a
  synchronous default-None getter using its existing error boundary. Same native
  capture/selection/family, no fourth public Prefix mode or JSON-derived authority.
  Root owns missing actual model-host/admission/ResultOnly and required native
  review custody; Exact may prepare connected private consumers without
  inventing those adapters or claiming full I2 acceptance.
