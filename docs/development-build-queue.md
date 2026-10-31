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

Human workflow correction (2026-10-31): collect and review the currently ready
compatible patches before starting Cargo, then compile their shared affected
artifacts once. Separate owner-specific cases keep failure diagnosis clear.
While Screen runs, foreground work continues on the next implementation package;
changes to frozen inputs stay in incremental private patches. Do not spend the
foreground turn waiting for the whole native/lint tail or repeatedly reporting
unchanged progress. Inspect the runner after useful work intervals and on actual
failure/completion. Keep Cargo serial. Future input manifests should cover the
actual compiler/runtime inputs, rather than freezing unrelated repository files;
the current batch's already captured freeze stays intact until its original exit.

Current completed bundle040–043: [receipt](validation/2026-10-31-validator-cli-admission.md),
SHA d0797b99…; original65594 terminal, common lock free,3315 captured inputs
verified unchanged before the final changelog update. All selected native/
architecture/strict checks pass. The final embedded changelog delta is left to
new GitHub CI, not silently covered by the preceding exact local package receipt.
These changes are now checkpoint2522569198daf2d1c842142a6374d8af394f43e8
on main/origin; full GitHub run41197330030 targets it. Outcome remains pending.
BUILD045–051 are **Passed locally in their exact scope**:
[receipt](validation/2026-10-31-settings-catalogue-facades.md), JSON
SHAef73a537caf4df451e8890c04eae7fe3e37fa8edabab926f150a7f99f7d63dcd.
21 native Composition,28 static,32 architecture and42 Node cases; no selected
ignored cases. One successful shared compilation248.7s and strict connected/
default checks54.7s/50.3s. Run1/2 compiler failures, run3 zero selector and run4/5
lint failures remain intact; passing results were reused. The two final style
repairs preserve the concrete transaction reference and exact decimal-string
assertion; both source manifests/image scopes are in the receipt. Original72982
is gone, common lock independently acquired and3315 final hashes verified.
Source freeze released; Root keeps Cargo. Later embedded plan-prose updates are
not silently covered by the earlier exact native package identity.

BUILD052/053 are **Passed locally in their exact scope**:
[receipt](validation/2026-10-31-standalone-oauth-rereview.md), JSON SHA
5b3a687864643daca6ac9a0f2c90116f172a98d97a464d5c1d4046fa614ee993.
One shared test compilation256.8s, fresh worker15.7s;11 Rust cases (7 actual
PG/global,3 existing HTTP contracts,1 Host API),28 Node,28 static,32 architecture,
two actual Chromium paths and both strict configurations pass. Original74626
ended; common lock independently acquired,3318 captured inputs/images verified.
Current worker ef997c89… supersedes B6. Freeze released; no replacement compile.
Previous045–051 checkpoint c68cfe80c41a0e1a0a3dcaca96234624a847eccb is pushed.

Next compatible package054 remains private until its connected successor repairs
qualification isolation and captures original input bounds in the existing job
owner. Both Use and set_active-Upsert remain required. Root036 native binding/
refresh/guards and its delegated Device lifecycle remain private. Collect and
review these complete parts before the next shared compile; no SDK-only cutover.

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
| BUILD-045 | Existing Prefix Sources independent reads | Passed local scoped | Two-path4931ba2d…; source inventory and generation errors are independent, successful data retained. Included in42 passed/reused Node cases. No fabricated source-availability results or new backend. Shared run1 compiler failure retained. |
| BUILD-046 | Actual ToolSkill scoped inventory/detail readers | Passed local scoped | Three-path94d23e33…; existing PgToolSkillStore supplies real rows to facade, shared token estimator and four existing storage keys. Existing native HTTP fixture extended; missing tables/closed pools are errors, never empty successful inventories. |
| BUILD-047 | Current Recipe catalogue detail, both existing routes | Passed local scoped | H27c2ff90… integrated; one active-catalogue projector supplies exact immutable Recipe documents, references, variants and native model-input declarations. No allocation-head inference/new DTO. Root repaired the routing test pool variable; both real catalogue HTTP cases and the missing-owner case pass, with42 joint Node cases. |
| BUILD-048 | Visible unsupported ToolSkill review actions | Passed local scoped | Incremental66537e94… removes successful no-op status writes and the store's blanket dead-code suppression. Existing supported error taxonomy, row/queue bytes unchanged; immutable functional writer remains its existing Validator owner's open work. |
| BUILD-049 | Measurement observation/attempt fences | Passed local scoped | Two-path270d43a2…; profile/generation/job/read-order fencing, uncertain original retry retained. Included in42 passed/reused Node cases. No new durable UI original, backend DTO or high-measurement acceptance claim. |
| BUILD-050 | Atomic Recipe rejection in existing Queue/Recipe owners | Passed local scoped | R2 patch28ee0f62… replaces rejected R1; incremental R3 b6a79089… shares actual transaction-key locking before absence/insert, with real pg_locks concurrency case. Legitimate manual pre-Q1 actions remain; Q2 row/queue changes commit together; upgrades preserve exact live bytes; all backend/commit failures are visible. |
| BUILD-051 | Existing Auth refresh caller-owned publication transaction | Passed local scoped | Two reviewed Root paths, original scoped operation delegates to the same extracted account/action helper. Real rollback extension preserves independently retained response and original revisions; six existing durable-refresh cases pass, including the real caller-owned rollback. No new journal, generic System grant or SDK-only cutover; complete036 credential/host bridge remains private/open. |
| BUILD-052 | Existing manual Recipe re-review metadata | Passed local scoped | One status/tag transaction, no Queue/Q4 reset or approval. Both real PG/HTTP cases pass, including late-statement and deferred-COMMIT rollback. Shared receipt5b3a6878…. |
| BUILD-053 | Existing standalone OAuth setup default/maximum | Passed local scoped | V197, acknowledged600/600 defaults and coherent positive-u32 pair, three HTTP gates and original browser producer. Root account-before-await repair included. Actual upgrade/live-PG/global/two Chromium paths,28 Node and both strict configurations pass; shared receipt5b3a6878…. |
| BUILD-054 | Existing I1 original Use→Bootstrap owner | Private implementation | Existing003/005 capture/assembly reused; explicit Use AND set_active-Upsert, actual retained/draft preparation origin, one mutation/spec/setup/job commit then ordinary admission, real technical Recipe/result and original-target inspection. PrefixFreeWorkflow stays closed without actual native sealed qualification. No new catalogue/queue/snapshot/renderer; migration numbers only at Root integration. |
| BUILD-037 | Existing Prefix renderer/domain Unicode escapes | Passed scoped consumers | Five production Python paths+existing tests and real contained Monty supplementary-Unicode case pass. ASCII/BMP/unaffected text, raw cards/IDs/context maps preserved. No deployed special-token/model/Serving claim. Shared receipt bb58d98f…. |
| BUILD-038 | Existing ModelDesign identity/original view | Passed presentation scope | Four exact source paths,9 Node and actual nativeHTTP/React keyboard/literal-original tests, current embedded bytes/304 and strict pass. Provider configuration revision correctly named; no inner reserialization/new status/action/complete P06. Shared receipt bb58d98f…. |
| BUILD-039 | Prefix HTTP revision precision | Passed local scope | [Receipt](validation/2026-10-31-prefix-revision-wire.md) 0512f9ee…:78 Product,5 HTTP,28 static,1 global,32 architecture,42 Node; four actual Chromium paths, connected/default strict. No positive high-measurement, new native Integer contract or Serving/full-plan claim. All R1–R5 attempts retained; current source manifest aa6e65af…, freeze released, Root retains Cargo. |
| BUILD-040 | Existing Validator own-function rule gap | Passed local scoped | Root integrated reviewed five-path040 plus exact historical source/retention regression; runner61560 in /private/tmp/brassclaw-validator-ci-validation-20261031/run-1. Root reviewed scope-audit 522e7f05…: one Monty skill_call_targets rule over existing inventory, reusing the existing pure_calls closure walk. No DTO/parser/authority or approval-writer additions. Joint selection/observation refusal stays unchanged; individual native rule evidence cannot establish joint Q1. Existing019/C1 not duplicated. |
| BUILD-041 | Existing CLI embedding maintenance fixture | Passed actual CLI/PG and strict | Root integrated one-file fixture repair in batch040/041/042. Actual CI41188072054 job124499050121:97 CLI unit,5 extension and7 lifecycle pass; embedding credential case fails before expected Auth check. Source confirms missing explicit embedding_models contract prevents reaching authentication. One-file private fixture successor plus exact stderr/status diagnostics at /private/tmp/brassclaw-cli-embedding-fixture-041-20261031. No production/security expectation weakening. Actual run1 now shows retained guard ConfigurationChanged before adapter AuthFailed; initial diagnosis incomplete. Private assertion successor preserves actual missing-key rejection and correct-key success. Original failed log retained; corrected actual CLI/PG check and strict pass in receipt d0797b99…. The earlier failed CI remains a failure. |
| BUILD-042 | Existing large-source acceptance observer | Passed local global/strict; new CI pending | Root integrated two test-only source repairs in batch040/041/042. Actual CI41188072054 Prefix browser job124498974964 timed out at120s before browser paths. Two test-only files under /private/tmp/brassclaw-prefix-ci-observer-042-20261031: use existing600s default task budget for outer observer; preserve failures and add release elapsed/safe phase diagnostics. No production timeout/budget change. Root let-chain correction invalidates old private after hash; verify before and capture actual formatted result. |
| BUILD-043 | Existing fresh component-review admission for historical class50 | Passed native/HTTP/strict | Definition peer prepares one existing retain_subject guard after original readback and before allocation, with real legacy/repeat/negative cases. No parser ban, class21 alias, raw retention or legacy queue/boot cutover claim. Root review/integration and actual scoped native/strict acceptance complete in receipt d0797b99…. |
| BUILD-044 | Existing Defensive/HA/tomedo collectors | Passed local standalone scope | Four-path patch dd37839e… at /private/tmp/brassclaw-domain-collector-repair-20261031: visible scan/read/stat errors, one existing SQLite scan transaction, delayed working manifest/FINAL publication and real file/Git/SQLite regression cases. No new collector/schema or native domain/Serving activation. DB/FS crash atomicity remains outside this repair. Integrated after original63700 exited; actual five methods/fifteen subcases pass. [Receipt](validation/2026-10-31-domain-collector-preservation.md). No Cargo needed for these standalone files. |
| BUILD-030 | Root integration of authorized skills-chat documentation | Passed |42 actual paths reviewed: two active skill mirror pairs/valid safe-parsed metadata,11 exact-hash historical Bob bodies plus archive README, removal of explicitly unwanted video skill, GitHub/check-selection prose. Root fixes migration-number guidance to include untracked/reserved V196 via rg --files. All outside captured/release inputs; no Cargo or site/video/runtime approval claim. Scope report /private/tmp/brassclaw-skills-root-integration-20261031/review.md/current-inputs.json; include complete archives at next shared checkpoint. Remaining older add-tool/trace architecture assumptions stay explicit K4/current-guide consumer work, not silently certified. |
| BUILD-031 | Prefix reviewer private step16 C1/E, Root integration | Passed static/offline scope |13 canonical cases including real HA2026.2.3 schemas/0skipped plus actual3-domain compiler caller pass. [Receipt](validation/2026-10-31-prefix-response-hardening.json)86c02191…; Root345 SHAff06f10c…. R1 missing compiler SDK imports retained; exact unchanged HA cases reused, existing compiler interpreter records its actual versions. Third-party lru-dict/PyRIC installer metadata deprecations remain visible/unresolved; no filters/overrides. Native Recipe/Serving/semantic entailment not certified. |
| BUILD-032 | Prefix/Skills bootstrap protocol drafts | Passed closed scope | Actual3 Skills+native PG/IBS/pure candidate checks, exact original/output/protocol bindings and existing consumers/strict pass. No productive model admission, activation or new approval. Shared receipt bb58d98f…. |
| BUILD-033 | Prefix common literal segmentation | Passed | Current Python/source consumers and actual Monty LF/CRLF/source/captured-design/global checks pass, old bundle/raw bytes/unaffected IDs preserved. Native compiler/package qualification remains scoped. Shared receipt bb58d98f…. |
| BUILD-034 | Existing Prefix History UI | Passed current caller scope | Canonical19 History/Tab Node cases and actual real React/nativeHTTP/PG race/file/unmount/original-retry checks, current embedded bytes/304 and strict pass. Genuine unmount/restart has no durable UI original recovery. External caller keyboard/observed-first-request repair preserves all original assertions. Shared receipt bb58d98f…. |
| BUILD-035 | Existing bootstrap evidence/prefill phases | Passed closed native observations |20 additional fixture observations in same exact native candidate case pass; no new checker/API/provenance/approval. Original packet/schema/Golden unchanged. Shared receipt bb58d98f…. |
| BUILD-036 | Root K3 Codex host credential cutover | Private SDK ready; host/PG open | SDK-R3 /private/tmp/brassclaw-codex-sdk036-r3-d13rox4k patch8997b5a3… reviewed: immutable request-local token/guard, existing final transport/observer, explicit File vs Host, SingleSubmission, safe errors/headers. Not compiled/integrated. Root private existing PG transaction extraction under /private/tmp/brassclaw-codex-host036-root. Real instance subject/descriptor/original/rotation/publication/CAS, all consumers/UI and native proofs remain required. No SDK-only cutover/new journal. |
| BUILD-029 | Existing Prefix/Composition/Validator future construction selection | Passed local document/package scope | Single normative visible Maker/Base/Prefixless contract and owner references,9 development-only originals; old270 inventory rows/13 selected rows unchanged. Actual native328-source base/Sempai package consumers pass. No productive Free/Serving/approval grant. Shared receipt bb58d98f…, Root423 f71583df…. |
| BUILD-028 | Root, actual033 checkpoint CI prerequisites and shared chat fixture | Passed locally, GitHub pending |4 native/0ignored,5 real fixture CLI cases, affected strict and actionlint pass. Existing HF oracle/runtime/worker supplied in CI; actual pinned1125-byte download exercised. Shared request-keyed two-worker fixture keeps original history/strict failure checks. [Receipt](validation/2026-10-31-ci-encoding-chat-fixture.json)04b096b0…; Root343 SHAa7821faa…, workerb6d1101c… unchanged. Original40798 exited/common lock independently free; supplemental132 source-capture timing recorded. New GitHub acceptance required, no full-plan claim. |
| BUILD-027 | Root, existing K3 step6 task-gate OAuth lifetime | Passed |91 distinct native+32architecture/0ignored, connected/default strict pass. V196/common Google-DCR original deadline and real UI/effective uptake accepted. [Receipt](validation/2026-10-31-gate-oauth-lifetimes.json)9b805da5…; Root339 SHA9bb1d84e…, fresh workerb6d1101c… verified. Original38025 exited, common lock independently free; failed attempts retained, unchanged72 cases/compilation reused. Scope local macOS/task-gate lifetime; standalone OAuth/max/client/polling/ingress and full K3 remain open. Root retains Cargo. |
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

Current combined successor: R2 actual warning-free compilation/static/native
prerequisites pass; native browser wait failure retained. R3 original53190 runs
the same Rust/images/worker with ONLY the external browser diagnostic successor.
8403 inputs frozen, no duplicate compile. Remaining global/browser, architecture
and strict acceptance required before closing029/032/033/034/035/037/038.

036 private SDK work is now explicitly assigned and reviewed; obsolete dual
File/HostManaged mode removal requested before integration. Root prepares the
existing PG refresh transaction helper privately. No partial SDK-only cutover.
Prefix outer-u64 precision needs connected native result/capture review; retained
integer contracts, raw bytes and strict Toolpolicy decoder remain unchanged.
