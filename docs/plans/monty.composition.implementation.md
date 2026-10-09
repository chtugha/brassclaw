# monty.composition — component implementation instructions

This appendix is part of [monty.composition](monty.composition.md). It implements the specification requested in sections 10–12: small reusable executable units assembled by Recipes. It does not modify or activate seeded rows.

## Common exact authoring contract

For each concrete successor, resolve a canonical stable UUID and create an immutable revision with its reviewed description, source, recursive input/result contracts and pinned dependencies. These are target retained-document contracts; do not paste unsupported fields into current legacy upsert constructors. Declare every object field and unknown-field rule, list item schema, missing default, nullability and numeric/size bound. The per-entry input descriptions below define the semantic shape; implementation must instantiate it with the real retained schema validator, using actual adapter maximums rather than invented universal caps. Validate producer outputs before consumers; null, false, zero and missing remain distinct.

For each one-Tool usage, store prose and an explicitly associated class-22 entry point, plus ToolSkill/Tool references and exact arguments/result/failure contract under skills.md's skill-association/1. Use default stop/max_attempts=1/not_assumed/no retryable outcomes until verified read-only or durable deduplication evidence supports an exact alternative. Noncircular packaged system_seed qualification requires structural/Q1/integrity and behavioral evidence; installed authoring requires human Q2 as well. Do not treat a component document, successful parse or boolean verdict as approval or activation.

The snippets are concrete design examples, not proof of pinned Monty acceptance or full implementations where a blocker/subset is stated. Compile unchanged source with BrassClaw's pinned Monty and retained-source inspector; verify typed results/effects through the actual host adapter. General Monty language support does not imply BrassClaw permits every construct: use assigned result, direct syntactic host calls, no dynamic host access or runtime source slots. Ordinary child bodies have zero host calls for pure logic or one matching call for a normal usage. The class-10 root has a separate lifecycle contract. Every Tool example still requires verified registration/identity/parameter compatibility and live policy dispatch checks.

Source references and older changes are in the main tables. Each Skill subsection below deliberately points to its canonical executable; do not duplicate code because several Skills share an operation. A Recipe sequences binding/execution and pure components with the input-layout forms in main §10.2. Shell Recipes are Tier 1. Whole-file inverse operations, complete security verdicts, absent/malformed probes and missing adapters must not be replaced with partial successful approximations.


### Audited schema and association completion gates

**The draft prose and semantic input descriptions are not complete insertable component documents.** Before implementation admission, replace every generic Skill draft with its exact one-usage purpose, actual local parameters/defaults, prerequisites, computed/fixed arguments, success/error/wait/effect contract and examples. Follow skills.md §§5–7 rather than copying a shared paragraph unchanged. Specialize single/all patch (replace_all=False/True), file-new/template/replace (preconditions and overwrite semantics), recursive directory (recursive=True and explicit depth), grep content/count/files/case/glob and authenticated HTTP usages separately. A writer that always overwrites cannot claim create-only behavior merely after a non-atomic existence read. Missing conditional-create/concurrency guarantees are adapter gaps. Tail/read-filter/name-removal descriptions belong to Recipes; associated Skills describe only their actual single read/list/remove usage with precomputed inputs. Pure-only wrappers remain documentation.

For **every one-Tool usage**, complete the exact `skill-association/1` document, not only a name pairing. Use exactly format, skill_uuid, python_code_uuid, tool_skill_uuid, tool_uuid, callable, inputs, arguments, code_arguments, result and failure. Direct arguments map real parameter names to local inputs; remaining fixed/computed arguments declare supported recursive schemas, checks, depends_on and meaning. Verify the actual computed values against Tool schema/transport before dispatch. Stop-default failure is exactly action=stop, max_attempts=1, idempotency=not_assumed, idempotency_evidence_ref=null, retryable_outcomes=[]. Refuse duplicate/unknown keys, nil/wrong-class IDs or incomplete argument maps.

A separate trusted `skill-association-approval/1` record contains exactly format, approval_id, association_checksum, components, validation_mode, q1_ref, q2_ref and behavioral_refs. Its unique UUID/class/version/checksum references cover the complete exact code/Skill/ToolSkill/Tool closure and retained actual Tool artifact; its evidence must resolve successfully for that combination. Authored requires trusted Q1, behavioral evidence and human Q2; verified installation-owned system_seed requires noncircular Q1/integrity/behavior evidence and permits q2_ref=null. A label, self-issued record or task manifest is insufficient. IBS verifies the committed exact combination then pins the approval_id; activation is coherent and old references remain valid for retained tasks. Unchanged Skill prose need not get an artificial new revision merely to pair it with a newly reviewed code revision.

V3 value schemas are finite inline trees of one declared type per node, with homogeneous list items; **no any, unions, arbitrary JSON, recursive schema references, enum/pattern/max_length/max_items extension fields or inferred object fields**. Use separately qualified exact-shape profiles; enforce additional semantic/technical bounds through supported preflight/code/adapter checks. Top-level inputs require required:boolean and checks:list; optional top-level inputs require a validated default. Object fields require presence rules; result roots/list items forbid root presence/defaults, and result fields never have defaults. Persist actual interfaces using these contracts, not JSON-Schema properties/oneOf copied from Tool manifests. The Tool's registered JSON Schema and the v3 usage schema are different contracts that must agree.

Where a snippet selects a dynamic field/path/method, each approved usage restricts it to an explicit concrete schema/selector and validates even empty collections. A shared implementation does not provide an unlimited generic contract. Arbitrary JSON parse/query/serialization, dynamic record projection, mixed path segments and HTTP body alternatives require separate exact profiles or explicit schema/adapter work before admission; never silently weaken the final v3 architecture.

**Review data is untrusted.** The trusted review owner resolves complete checksum-bound parser facts, dependency/Tool-artifact manifests and successful evidence before handing typed facts to pure checkers. No candidate-supplied compatible/complete/retry_evidence_verified flag can satisfy a trusted requirement. Keep infrastructure failure, invalid candidate and incomplete review distinct. Partial N08–N14 examples report complete=False and passed=False; N07 covers only its explicitly narrow predicate. N15 is pure aggregation, not trusted evidence persistence, semantic approval, human Q2 or activation. Diagnostic records use one concrete list-of-objects schema with path:list[string] and code:string; evidence refs are homogeneous strings with a verified owner/subject contract.

**Security and result boundaries:** verify conversion at the Rust typed Monty-node boundary before lossy repr/cycle/depth placeholder conversion; rejecting strings by appearance cannot distinguish lossy values from legitimate identical data. Check exact source and Tool parameter/result constraints, subject ownership, current global policy and resource limits at each effect. Monty input isolation alone does not make filesystem/API operations race-free. A Tool payload, class-22 local result and run_program ok/return_value/error envelope are distinct; Recipe layouts bind the validated payload value. HTTP output uses status, headers:list[{name,value,...}], optional text body/saved_body and explicit truncation metadata. Never claim successful parse/full coverage from truncated HTTP/list/read output. Shell output uses output, exit_code, success and sandboxed; it does not promise separate stdout/stderr fields.

Prepared-input examples in L018 and N16–N20, the existence classifier in L010, partial validator examples and the class-10 excerpt are **not deployable completed executors**. They stay blocked/draft in the conversion ledger until their actual operation/result contracts and complete behavior exist. Unknown host callables are never guessed. Current legacy scoped authorization code is an implementation cutover dependency, not a v3 tenant/project role requirement; preserve technical enforcement while moving to instance-global policy.


### Concrete-profile acceptance requirements

Shared code is accepted once per exact usage combination, not as a schema-free dispatcher. Where the supported usage schema cannot express a fixed selector/value, the approved body must fix it or perform an explicit pre-effect guard, with code_arguments matching actual computation; passing a Recipe constant alone must not falsely describe an unconstrained standalone Skill as intrinsically fixed. Internal guard/helper composition must be supported and pinned, or author a small explicit entry point; do not invent an enum constraint in the value schema. All independent host calls remain separate steps. The binding preflight currently accepts one syntactic direct call for a Tool-bound child or zero for pure logic; v3's direct dependent-chain exception needs its own qualified binding layout before use. Counting call sites is not proof of at-most-once dispatch when a call appears in a loop or recursive function; source/behavior review must establish the declared effect count and attempt semantics.

Directory list output is best-effort as well as capped: vanished/failed-stat entries can be skipped. A filter cannot prove filesystem absence or complete type coverage. Raw versus numbered read text, terminal newlines, empty input and nested Markdown headings must be explicit result semantics. The memory-section example is a deliberate revised ATX-only profile (deeper subsections retained), not the old stop-at-any-heading behavior or a full Markdown parser; record that semantic change, reconcile each consumer and keep old revisions. Plan snapshot/progress summaries require one coherent API snapshot or clearly documented best-effort reads, not atomicity inferred from two GETs.

Shell commands inherit actual process-context/workdir/technical constraints and installed platform behavior. Fixed git commands can execute configured hooks/helpers/filters or expose data; do not label them mutation-free merely from their spelling. In particular fetch changes refs and pull changes worktree; preserve unknown-effect outcomes. Review remote/ref/path option semantics and Git pathspec interpretation (including magic) separately from shell quoting. If a usage promises literal paths, reject or deliberately encode pathspec magic under the actual installed Git semantics. Values beginning '-' must not become options; POSIX quoting is not an option/endpoint validator. Tier1 classification does not itself require an extra invocation approval lease; live instance-global Tool policy and technical restrictions remain the authority boundary.

Authenticated HTTP uses the supported secret/auth adapter and rejects CR/LF header injection. Avoid raw credentials in durable general component inputs/logs; reference secret identities through the supported boundary where available, and treat missing secure transport as an adapter gap. Review redirects/origins, request-body representation, actual status/headers/body_text/body_base64/saved_body schema and truncation limits per usage. Zencoder has a seeded Tool row but no registered handler established by this audit; all illustrative host.zencoder_api bodies remain blocked until actual loading/identity/result/auth support is proven. A plan 404 is not automatically plan-not-created unless the qualified API error contract establishes that distinction.

## Legacy entries

### L001. pc-exec-read-file

**Prose description:** Use the host boundary to read a file via builtin.read_file.

**Implementation:** Pass typed path and supported offset/limit values; remove the unsupported range argument and the assumed line_count result. Keep the whole-file/default-cap usage distinct from the foreseen bounded interval usage, and validate total_lines, lines_shown and truncation metadata. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association. **Duplicate review:** newer section-2 entries 5 overlap this functionality. Compare purpose, inputs/results and identity before converting; reuse compatible code and preserve distinct contracts.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** path: nonempty string. Return the retained read_file object: content, path, total_lines, lines_shown and truncated_by_default.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = host.read_file(path=inputs["path"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-read-file

**Usage prose draft to specialize:** “Use the host boundary to read a file via builtin.read_file. Supply the typed inputs described for `pc-exec-read-file`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-read-file`'s block above.

### L002. pc-exec-write-file

**Prose description:** Use the host boundary to write a file via builtin.write_file.

**Implementation:** Read path/content from typed inputs, preserve text verbatim, and validate actual write acknowledgements. A completed write must never be repeated because a later step or reply fails. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** path: nonempty string; content: string, including empty. Return the actual write acknowledgement.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = host.write_file(path=inputs["path"], content=inputs["content"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-write-file-new

**Usage prose draft to specialize:** “Use the host boundary to write a file via builtin.write_file. Supply the typed inputs described for `pc-exec-write-file`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-write-file`'s block above.

#### skill-write-file-template

**Usage prose draft to specialize:** “Use the host boundary to write a file via builtin.write_file. Supply the typed inputs described for `pc-exec-write-file`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-write-file`'s block above.

#### skill-write-file-replace

**Usage prose draft to specialize:** “Use the host boundary to write a file via builtin.write_file. Supply the typed inputs described for `pc-exec-write-file`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-write-file`'s block above.

### L003. pc-exec-list-dir

**Prose description:** Use the host boundary to list a directory via builtin.list_dir.

**Implementation:** Bind path, recursive and max_depth with explicit missing/default rules and bounds. Use the actual directory-entry schema; false, zero, null and missing must not silently collapse into the same case. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** path: nonempty string; recursive: boolean; max_depth: nonnegative integer, not bool. Return the actual list_dir envelope: path:string, entries:list[string] of display text, count:integer and truncated:boolean. This inspected primitive does not currently expose structured name/type fields.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = host.list_dir(path=inputs["path"], recursive=inputs["recursive"], max_depth=inputs["max_depth"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-list-dir

**Usage prose draft to specialize:** “Use the host boundary to list a directory via builtin.list_dir. Supply the typed inputs described for `pc-exec-list-dir`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-list-dir`'s block above.

#### skill-list-dir-recursive

**Usage prose draft to specialize:** “Use the host boundary to list a directory via builtin.list_dir. Supply the typed inputs described for `pc-exec-list-dir`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-list-dir`'s block above.

### L004. pc-exec-glob

**Prose description:** Use the host boundary to find files via builtin.glob.

**Implementation:** Bind a typed pattern, optional root and bounded result count; verify current keyword/result contracts. Treat ordering and truncation as explicit primitive behavior rather than assuming all matches are returned. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** pattern/path: nonempty strings; max_results: bounded positive integer, not bool; use the actual adapter maximum. Return the actual files:list[string], count:integer, truncated:boolean, duration_ms:nonnegative integer, with optional limit_reason:string, visited_entries/max_visited_entries:nonnegative integers. The primitive sorts by descending modification time, using UNIX_EPOCH for absent timestamps, then caps results; traversal limits can prevent global newest-file completeness. Declare every field and optional presence rule.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = host.glob(pattern=inputs["pattern"], path=inputs["path"], max_results=inputs["max_results"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-glob-by-extension

**Usage prose draft to specialize:** “Use the host boundary to find files via builtin.glob. Supply the typed inputs described for `pc-exec-glob`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-glob`'s block above.

#### skill-glob-by-name

**Usage prose draft to specialize:** “Use the host boundary to find files via builtin.glob. Supply the typed inputs described for `pc-exec-glob`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-glob`'s block above.

#### skill-glob-in-subdir

**Usage prose draft to specialize:** “Use the host boundary to find files via builtin.glob. Supply the typed inputs described for `pc-exec-glob`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-glob`'s block above.

### L005. pc-exec-grep

**Prose description:** Use the host boundary to search content via builtin.grep.

**Implementation:** Bind pattern, path, mode, glob and case flag as typed values. Verify allowed modes and real result shapes, and reject unsupported arguments before dispatch. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** pattern/path/glob: strings; output_mode: retained adapter enum; case_insensitive: boolean. Declare a separate exact result schema per approved output_mode usage.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = host.grep(pattern=inputs["pattern"], path=inputs["path"], output_mode=inputs["output_mode"], glob=inputs["glob"], case_insensitive=inputs["case_insensitive"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-grep-files

**Usage prose draft to specialize:** “Use the host boundary to search content via builtin.grep. Supply the typed inputs described for `pc-exec-grep`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-grep`'s block above.

#### skill-grep-content

**Usage prose draft to specialize:** “Use the host boundary to search content via builtin.grep. Supply the typed inputs described for `pc-exec-grep`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-grep`'s block above.

#### skill-grep-count

**Usage prose draft to specialize:** “Use the host boundary to search content via builtin.grep. Supply the typed inputs described for `pc-exec-grep`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-grep`'s block above.

#### skill-grep-case-insensitive

**Usage prose draft to specialize:** “Use the host boundary to search content via builtin.grep. Supply the typed inputs described for `pc-exec-grep`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-grep`'s block above.

#### skill-grep-type-filtered

**Usage prose draft to specialize:** “Use the host boundary to search content via builtin.grep. Supply the typed inputs described for `pc-exec-grep`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-grep`'s block above.

### L006. pc-exec-apply-patch

**Counterfactual overlap review:** use C02 for the reusable observed patch precondition; retain this single file-patch dispatcher and its actual dispatch freshness requirements.

**Prose description:** Use the host boundary to apply a targeted patch via builtin.apply_patch.

**Implementation:** Keep old/new text as typed strings and replace_all as a boolean. Validate the selected patch result and preserve the original effect identity on interruption; do not redispatch an acknowledged edit. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** path: nonempty string; old_string: nonempty string; new_string: string; replace_all: boolean. Return the actual patch acknowledgement.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = host.apply_patch(path=inputs["path"], old_string=inputs["old_string"], new_string=inputs["new_string"], replace_all=inputs["replace_all"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-apply-patch-single

**Usage prose draft to specialize:** “Use the host boundary to apply a targeted patch via builtin.apply_patch. Supply the typed inputs described for `pc-exec-apply-patch`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-apply-patch`'s block above.

#### skill-apply-patch-all

**Usage prose draft to specialize:** “Use the host boundary to apply a targeted patch via builtin.apply_patch. Supply the typed inputs described for `pc-exec-apply-patch`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-apply-patch`'s block above.

### L007. pc-exec-grep-invert

**Prose description:** Use the host boundary for an inverted grep via builtin.grep.

**Implementation:** The inspected grep implementation has no invert_match option; verify the final retained adapter, and do not activate an unsupported keyword. If absent, express the defined inverse operation using explicit retrieval and reusable pure filtering, without mistaking truncated results for a complete inverse. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Replace unsupported invert_match with explicit retrieval/filtering only for a reviewed literal-line variant. Original regex/files-without-match semantics need a qualified adapter or complete enumeration plus trusted regex facts; the example is not an equivalent replacement.

**Typed inputs/result:** content/substring: strings; invert: boolean; result: bounded list of strings. This is literal substring filtering over exactly the supplied text.

**Recipe wiring:** `pc-exec-read-file` → `proposed-filter-text-lines`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = []
for line in inputs["content"].split("\n"):
    matches = inputs["substring"] in line
    if matches != inputs["invert"]:
        result.append(line)
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-grep-invert

**Usage prose draft to specialize:** “Use the host boundary for an inverted grep via builtin.grep. Supply the typed inputs described for `pc-exec-grep-invert`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** the local pure/preparation snippet is not this Skill's executor. Associate this Skill only with the relevant single-Tool retrieval/write/removal usage in the Recipe above; pure portions stay separately referenced PythonCode. A pure-only historical wrapper becomes PythonCode documentation, not a zero-Tool Skill. For multi-Tool historical prose, retain task behavior in the Recipe and author separate usage Skills.

### L008. pc-exec-list-filter-by-type

**Structured-directory adapter gap:** current list_dir returns display strings, not name/type records. This example requires a qualified structured result profile exposing actual identity fields; do not feed display strings to it or infer file type/name by parsing suffixes. Reuse existing primitive internals through a supported adapter upgrade where possible. Trigger records can use this helper independently once their schema is qualified.

**Prose description:** filters a list_dir result to only entries of a given type.

**Implementation:** Receive the producer entries as a typed list of objects. Reject malformed elements instead of silently replacing them with an empty list; declare the filtered-entry and count result contracts. Keep the companions as list-directory usages with explicit associations to the list executor; reference this pure filter separately in the Recipe rather than defining a zero-Tool Skill.

**Split/reuse:** Reuse generic record predicate with field=type, mode=equals; retain usage-specific result packaging only if consumers need it.

**Typed inputs/result:** entries: bounded list of actual directory-entry objects; entry_type: retained file/directory enum, matched to real producer spelling; output: same entry schema plus count and entry_type.

**Recipe wiring:** `proposed-filter-records`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
entries = inputs["entries"]
filtered = []
for entry in entries:
    if entry["type"] == inputs["entry_type"]:
        filtered.append(entry)
result = {"entries": filtered, "entry_type": inputs["entry_type"], "count": len(filtered)}
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-list-dir-files-only

**Usage prose draft to specialize:** “filters a list_dir result to only entries of a given type. Supply the typed inputs described for `pc-exec-list-filter-by-type`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** the local pure/preparation snippet is not this Skill's executor. Associate this Skill only with the relevant single-Tool retrieval/write/removal usage in the Recipe above; pure portions stay separately referenced PythonCode. A pure-only historical wrapper becomes PythonCode documentation, not a zero-Tool Skill. For multi-Tool historical prose, retain task behavior in the Recipe and author separate usage Skills.

#### skill-list-dir-dirs-only

**Usage prose draft to specialize:** “filters a list_dir result to only entries of a given type. Supply the typed inputs described for `pc-exec-list-filter-by-type`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** the local pure/preparation snippet is not this Skill's executor. Associate this Skill only with the relevant single-Tool retrieval/write/removal usage in the Recipe above; pure portions stay separately referenced PythonCode. A pure-only historical wrapper becomes PythonCode documentation, not a zero-Tool Skill. For multi-Tool historical prose, retain task behavior in the Recipe and author separate usage Skills.

### L009. pc-exec-read-file-tail

**Prose description:** reads the last 50 lines of a file (lines -50 onward).

**Implementation:** Replace both range calls and line_count with the real offset/limit and total_lines contracts. Split metadata read, bounded tail arithmetic and final interval read into explicit steps, reusing the foreseen interval code; preserve empty-file and EOF behavior. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association. **Duplicate review:** newer section-2 entries 5 overlap this functionality. Compare purpose, inputs/results and identity before converting; reuse compatible code and preserve distinct contracts.

**Split/reuse:** Split: read metadata → tail-window → interval read. Reuse newer interval component 5; do not retain a second two-call executor.

**Typed inputs/result:** total_lines: nonnegative integer; count: positive bounded integer; output offset >=1, limit >=0, both integer-not-bool. Empty file has limit 0.

**Recipe wiring:** `pc-exec-read-file` → `proposed-tail-window` → `newer-5`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
total = inputs["total_lines"]
count = inputs["count"]
if type(total) is not int or total < 0 or type(count) is not int or count < 1:
    raise ValueError("invalid tail bounds")
result = {"offset": max(1, total - count + 1), "limit": min(count, total)}
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-read-file-tail

**Usage prose draft to specialize:** “reads the last 50 lines of a file (lines -50 onward). Supply the typed inputs described for `pc-exec-read-file-tail`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** the local pure/preparation snippet is not this Skill's executor. Associate this Skill only with the relevant single-Tool retrieval/write/removal usage in the Recipe above; pure portions stay separately referenced PythonCode. A pure-only historical wrapper becomes PythonCode documentation, not a zero-Tool Skill. For multi-Tool historical prose, retain task behavior in the Recipe and author separate usage Skills.

### L010. pc-exec-file-exists

**Prose description:** checks whether a file exists by attempting to read line 1.

**Implementation:** Remove catch-all failure-to-exists=False behavior. Use a verified existence/not-found contract and distinguish absence from denied access, cancellation, binary/oversized files and transport failure; a failed read does not establish nonexistence. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** path: string; outcome: trusted typed probe outcome with kind success/not_found or classified other failure. Result exists/path.

**Concrete caveat:** BLOCKED: current read failures do not establish an existence probe or typed not_found handling. First qualify a supported probe/outcome adapter; never use catch-all read failure. The pure classifier is an example for that future trusted contract, not a working existence Tool.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
outcome = inputs["outcome"]
if outcome["kind"] == "success":
    result = {"exists": True, "path": inputs["path"]}
elif outcome["kind"] == "not_found":
    result = {"exists": False, "path": inputs["path"]}
else:
    raise ValueError("existence indeterminate")
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-file-exists

**Usage prose draft to specialize:** “checks whether a file exists by attempting to read line 1. Supply the typed inputs described for `pc-exec-file-exists`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** the local pure/preparation snippet is not this Skill's executor. Associate this Skill only with the relevant single-Tool retrieval/write/removal usage in the Recipe above; pure portions stay separately referenced PythonCode. A pure-only historical wrapper becomes PythonCode documentation, not a zero-Tool Skill. For multi-Tool historical prose, retain task behavior in the Recipe and author separate usage Skills.

### L011. pc-exec-read-then-grep

**Prose description:** reads a file then greps the content for a pattern.

**Implementation:** Separate the reusable read usage from pure content filtering and pass the successful content field as data. Specify substring versus regex matching, line-number formatting and truncated-read behavior; do not fabricate empty content on a malformed read. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Split read retrieval from reusable literal line filtering.

**Typed inputs/result:** content/substring: strings; invert: boolean; result: bounded list of strings. This is literal substring filtering over exactly the supplied text.

**Recipe wiring:** `pc-exec-read-file` → `proposed-filter-text-lines`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = []
for line in inputs["content"].split("\n"):
    matches = inputs["substring"] in line
    if matches != inputs["invert"]:
        result.append(line)
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-read-and-grep

**Usage prose draft to specialize:** “reads a file then greps the content for a pattern. Supply the typed inputs described for `pc-exec-read-then-grep`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** the local pure/preparation snippet is not this Skill's executor. Associate this Skill only with the relevant single-Tool retrieval/write/removal usage in the Recipe above; pure portions stay separately referenced PythonCode. A pure-only historical wrapper becomes PythonCode documentation, not a zero-Tool Skill. For multi-Tool historical prose, retain task behavior in the Recipe and author separate usage Skills.

### L012. pc-exec-list-then-grep

**Structured-directory adapter gap:** current list_dir returns display strings, not name/type records. This example requires a qualified structured result profile exposing actual identity fields; do not feed display strings to it or infer file type/name by parsing suffixes. Reuse existing primitive internals through a supported adapter upgrade where possible. Trigger records can use this helper independently once their schema is qualified.

**Prose description:** lists directory entries then filters by name substring.

**Implementation:** Separate directory retrieval from pure entry-name filtering using typed results. Filter the actual name field instead of str(entry), and reject malformed results instead of silently returning an empty list. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Split directory retrieval from reusable typed record filtering.

**Typed inputs/result:** rows: bounded homogeneous object list; field: reviewed allowed field; mode: equals or contains; value: declared comparable type, string for contains. Require field/type compatibility in the exact usage contract. Output retains every row field.

**Recipe wiring:** `pc-exec-list-dir` → `proposed-filter-records`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
if inputs["mode"] not in ("equals", "contains"):
    raise ValueError("unsupported predicate")
result = []
for row in inputs["rows"]:
    value = row[inputs["field"]]
    if inputs["mode"] == "equals":
        matched = type(value) is type(inputs["value"]) and value == inputs["value"]
    else:
        if type(value) is not str or type(inputs["value"]) is not str:
            raise ValueError("contains requires strings")
        matched = inputs["value"] in value
    if matched:
        result.append(row)
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-list-and-filter

**Usage prose draft to specialize:** “lists directory entries then filters by name substring. Supply the typed inputs described for `pc-exec-list-then-grep`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** the local pure/preparation snippet is not this Skill's executor. Associate this Skill only with the relevant single-Tool retrieval/write/removal usage in the Recipe above; pure portions stay separately referenced PythonCode. A pure-only historical wrapper becomes PythonCode documentation, not a zero-Tool Skill. For multi-Tool historical prose, retain task behavior in the Recipe and author separate usage Skills.

### L013. pc-path-join

**Prose description:** join two path segments with a '/' separator.

**Implementation:** Bind two strings without source substitution. Define root, empty, absolute-child, separator and traversal behavior; this lexical helper does not establish filesystem access or containment.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** base/child: strings; result: string. Proposed POSIX lexical join: absolute child replaces base; empty child preserves base; no dot-segment resolution or containment proof.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
base = inputs["base"]
child = inputs["child"]
if child.startswith("/"):
    result = child
elif base == "":
    result = child
elif child == "":
    result = base
else:
    result = base.rstrip("/") + "/" + child
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

### L014. pc-path-basename

**Prose description:** extract the filename (last path component) from a path.

**Implementation:** Consume a typed path and document its intended POSIX lexical semantics. Test root, empty and trailing-slash cases without assuming CPython os.path behavior or importing forbidden modules.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** path: string; result: string. Last nonempty POSIX segment; root and empty produce empty. This intentionally defines trailing-slash semantics rather than claiming os.path equivalence.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
path = inputs["path"].rstrip("/")
result = path.split("/")[-1] if path else ""
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

### L015. pc-path-dirname

**Prose description:** extract the directory part of a path.

**Implementation:** Consume typed path data and correct the unconditional leading-slash/root result for relative paths. Declare and test empty/root/trailing-slash behavior; use only the pinned Monty-supported lexical operations.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** path: string; result: string. Relative leaf has empty dirname; /leaf and / have root; a/b has a. Preserve relative versus absolute identity.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
path = inputs["path"].rstrip("/")
if "/" not in path:
    result = "/" if inputs["path"].startswith("/") else ""
else:
    parent = "/".join(path.split("/")[:-1])
    result = parent if parent else "/"
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

### L016. pc-exec-http-get

**Prose description:** Use the host boundary for an HTTP GET request via builtin.http.

**Implementation:** Bind URL and headers as typed data, with the approved fixed GET selector. Use the exact reviewed Accept/auth profile; an empty headers object is an explicit public-request constant or a missing-only approved default, never a replacement for required credentials. Reject forbidden header names/values at the retained adapter; do not let model text supply authorization or host routing. Verify the actual HTTP body/response representation, numeric limits and error envelope; distinguish an HTTP response from confirmed workflow success. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** url: nonempty string; headers: object with string-valued extra fields under the exact approved header/secret profile; response_body_limit: positive integer within the retained adapter maximum; return the actual HTTP status/headers/body_text/body_base64/saved_body and truncation envelope. Non-2xx response is not workflow success.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = host.http(method="GET", url=inputs["url"], headers=inputs["headers"], response_body_limit=inputs["response_body_limit"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-http-get

**Usage prose draft to specialize:** “Use the host boundary for an HTTP GET request via builtin.http. Supply the typed inputs described for `pc-exec-http-get`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-http-get`'s block above.

### L017. pc-exec-http-post

**Prose description:** Use the host boundary for an HTTP POST request via builtin.http.

**Implementation:** Bind URL, body and headers as typed data, with the approved fixed POST selector. Verify the actual HTTP body/response representation, numeric limits and error envelope; distinguish an HTTP response from confirmed workflow success; do not retry a potentially completed mutation without durable evidence. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** url: nonempty string; body: supported JSON value or text; headers: string-valued object; return the actual HTTP status/headers/body_text/body_base64/saved_body and truncation envelope. Non-2xx response is not workflow success.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = host.http(method="POST", url=inputs["url"], body=inputs["body"], headers=inputs["headers"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-http-post

**Usage prose draft to specialize:** “Use the host boundary for an HTTP POST request via builtin.http. Supply the typed inputs described for `pc-exec-http-post`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-http-post`'s block above.

### L018. pc-exec-http-save

**Prose description:** Use the host boundary for builtin.http.save.

**Implementation:** Replace the nested host.http.save syntax with a verified single-attribute callable for the retained builtin.http.save implementation. Bind URL/save target as typed data and validate both HTTP outcome and file-write acknowledgement; a guessed alias is not registration. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** url/save_to: nonempty strings; output here is only prepared request data, not an HTTP/save result.

**Concrete caveat:** BLOCKED executor: resolve and qualify the single-attribute host alias for builtin.http.save. This preparation example deliberately makes zero effects. Replace it with the exact direct call only after registration and result contracts exist; never ship this as the completed save usage.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = {"url": inputs["url"], "save_to": inputs["save_to"]}
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-http-save-download

**Usage prose draft to specialize:** “Use the host boundary for builtin.http.save. Supply the typed inputs described for `pc-exec-http-save`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** the local pure/preparation snippet is not this Skill's executor. Associate this Skill only with the relevant single-Tool retrieval/write/removal usage in the Recipe above; pure portions stay separately referenced PythonCode. A pure-only historical wrapper becomes PythonCode documentation, not a zero-Tool Skill. For multi-Tool historical prose, retain task behavior in the Recipe and author separate usage Skills.

### L019. pc-exec-http-patch

**Prose description:** Use the host boundary for an HTTP PATCH request via builtin.http.

**Implementation:** Bind URL, body and headers as typed data, with the approved fixed PATCH selector. Verify the actual HTTP body/response representation, numeric limits and error envelope; distinguish an HTTP response from confirmed workflow success; do not retry a potentially completed mutation without durable evidence. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** url: nonempty string; body: supported JSON value or text; headers: string-valued object; return the actual HTTP status/headers/body_text/body_base64/saved_body and truncation envelope. Non-2xx response is not workflow success.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = host.http(method="PATCH", url=inputs["url"], body=inputs["body"], headers=inputs["headers"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-http-patch

**Usage prose draft to specialize:** “Use the host boundary for an HTTP PATCH request via builtin.http. Supply the typed inputs described for `pc-exec-http-patch`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-http-patch`'s block above.

### L020. pc-exec-http-head

**Prose description:** Use the host boundary for an HTTP HEAD request via builtin.http.

**Implementation:** Bind URL as typed data, with the approved fixed HEAD selector. Verify the actual HTTP body/response representation, numeric limits and error envelope; distinguish an HTTP response from confirmed workflow success. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** url: nonempty string; return the actual HTTP status/headers/body_text/body_base64/saved_body and truncation envelope. Non-2xx response is not workflow success.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = host.http(method="HEAD", url=inputs["url"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-http-head

**Usage prose draft to specialize:** “Use the host boundary for an HTTP HEAD request via builtin.http. Supply the typed inputs described for `pc-exec-http-head`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-http-head`'s block above.

### L021. pc-exec-http-get-authenticated

**Prose description:** Use the host boundary for an authenticated HTTP GET via builtin.http.

**Implementation:** Supply credentials through the protected host secret/auth boundary rather than interpolated or model-visible Authorization text. Bind nonsecret request data, verify the registered authenticated adapter, and retain the actual HTTP result contract. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** url: nonempty string; authorization: secret-bearing string supplied by the supported secret boundary. Return actual HTTP envelope; never log credentials.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = host.http(method="GET", url=inputs["url"], headers={"Authorization": inputs["authorization"]})
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-http-authenticated

**Usage prose draft to specialize:** “Use the host boundary for an authenticated HTTP GET via builtin.http. Supply the typed inputs described for `pc-exec-http-get-authenticated`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-http-get-authenticated`'s block above.

### L022. pc-exec-http-put

**Prose description:** Use the host boundary for an HTTP PUT request via builtin.http.

**Implementation:** Bind URL, body and headers as typed data, with the approved fixed PUT selector. Verify the actual HTTP body/response representation, numeric limits and error envelope; distinguish an HTTP response from confirmed workflow success; do not retry a potentially completed mutation without durable evidence. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** url: nonempty string; body: supported JSON value or text; headers: string-valued object; return the actual HTTP status/headers/body_text/body_base64/saved_body and truncation envelope. Non-2xx response is not workflow success.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = host.http(method="PUT", url=inputs["url"], body=inputs["body"], headers=inputs["headers"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-http-put

**Usage prose draft to specialize:** “Use the host boundary for an HTTP PUT request via builtin.http. Supply the typed inputs described for `pc-exec-http-put`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-http-put`'s block above.

### L023. pc-exec-http-delete

**Prose description:** Use the host boundary for an HTTP DELETE request via builtin.http.

**Implementation:** Bind URL, headers as typed data, with the approved fixed DELETE selector. Verify the actual HTTP body/response representation, numeric limits and error envelope; distinguish an HTTP response from confirmed workflow success; do not retry a potentially completed mutation without durable evidence. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** url: nonempty string; headers: string-valued object; return the actual HTTP status/headers/body_text/body_base64/saved_body and truncation envelope. Non-2xx response is not workflow success.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = host.http(method="DELETE", url=inputs["url"], headers=inputs["headers"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-http-delete

**Usage prose draft to specialize:** “Use the host boundary for an HTTP DELETE request via builtin.http. Supply the typed inputs described for `pc-exec-http-delete`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-http-delete`'s block above.

### L024. pc-http-status-check

**Prose description:** returns True when the HTTP status code indicates success (2xx range), False otherwise.

**Implementation:** Bind a bounded integer status code, rejecting booleans and invalid values. Preserve the declared 2xx classification without promoting an incomplete HTTP/file operation to completed success.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** status_code: integer 100..599, not bool; result: exact object with boolean is_success and integer status_code. This is HTTP classification, not semantic completion.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
status = inputs["status_code"]
if type(status) is not int or not 100 <= status <= 599:
    raise ValueError("invalid HTTP status")
result = {"is_success": 200 <= status < 300, "status_code": status}
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

### L025. pc-json-extract-field

**Audit correction:** There is no heterogeneous list[string|integer], union, any or unconstrained recursive JSON schema in skills.md. Compile each selected path/profile against the actual producer schema; optional/null parents and list bounds require explicit handling. Reject all malformed path segments before traversal, including ones following a missing field.

**Prose description:** extracts a value from a JSON object by dot-separated path.

**Implementation:** Receive structured data and a typed selector; distinguish an existing null value from a missing path. Reject negative/out-of-range list indices and invalid traversals, and declare the exact found/value contract rather than swallowing all lookup failures.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** data: one exact finite per-usage recursive schema; segments: homogeneous list of exact objects {kind:string,key:nullable string,index:nullable nonnegative integer}, all fields required and no extras. kind=key requires index=null; kind=index requires key=null. Result value: one declared per-usage type made nullable for absence/present null; found:boolean. Empty path selects root.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
current = inputs["data"]
for segment in inputs["segments"]:
    kind = segment["kind"]
    if kind == "key":
        if type(segment["key"]) is not str or segment["index"] is not None:
            raise ValueError("invalid key segment")
    elif kind == "index":
        if type(segment["index"]) is not int or segment["index"] < 0 or segment["key"] is not None:
            raise ValueError("invalid index segment")
    else:
        raise ValueError("invalid segment kind")
found = True
for segment in inputs["segments"]:
    if segment["kind"] == "key" and isinstance(current, dict) and segment["key"] in current:
        current = current[segment["key"]]
    elif segment["kind"] == "index" and isinstance(current, list) and segment["index"] < len(current):
        current = current[segment["index"]]
    else:
        found = False
        current = None
        break
result = {"value": current, "found": found}
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

### L026. pc-web-search-extract

**Counterfactual coverage refinement:** web-search promises top N. Add a required limit:positive integer to the selected finite projection profile and return result[:inputs["limit"]] after validating every acquired row. Request/provider completeness and truncation still travel separately; fewer displayed rows do not establish complete search coverage. Add no duplicate extraction/HTTP code. If the provider already enforces the exact display limit, reuse that profile directly.

**Prose description:** extract title+url+snippet list from a search API JSON response.

**Implementation:** Receive the parsed HTTP body as a typed object instead of a quoted slot. Declare supported provider shapes and list-item fields, and reject malformed provider responses rather than returning a success-shaped error dictionary or leaked raw preview.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** response: provider-specific validated object; rows_field/title_field/url_field/snippet_field: reviewed typed constants. limit:required positive integer, only in the selected top-N successor profile. Selected rows: bounded list of objects with those required string fields. Result: list of exact title/url/snippet objects.

**Concrete caveat:** Choose the provider in the Recipe and validate its complete nested shape before projection. No truthiness-based fallback between unrelated envelopes, no malformed-row omission. Add a distinct normalization helper only when a real provider needs nested groups; the flat example is not DuckDuckGo RelatedTopics normalization.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
limit = inputs["limit"]
if isinstance(limit, bool) or not isinstance(limit, int) or limit < 1:
    raise ValueError("invalid_search_display_limit")
data = inputs["response"]
rows = data[inputs["rows_field"]]
result = []
for row in rows:
    result.append({"title": row[inputs["title_field"]],
                   "url": row[inputs["url_field"]],
                   "snippet": row[inputs["snippet_field"]]})
result = result[:limit]
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

### L027. pc-url-encode

**Prose description:** URL-encodes a string (percent-encoding, spaces as %20). No imports - uses pure built-in character-by-character encoding.

**Implementation:** Encode UTF-8 bytes rather than formatting Unicode code points as percent escapes. Preserve or explicitly separate whitespace normalization, and test spaces, percent signs, reserved characters and multibyte text against the pinned Monty encoding behavior.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** text: bounded string without implicit strip; result: exact encoded/raw strings. UTF-8 percent encoding for one path/query value, not a full URL. Verify bytes iteration, encode and format in pinned Monty.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
safe = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~"
encoded = ""
for byte in inputs["text"].encode("utf-8"):
    encoded += chr(byte) if byte in safe else "%" + format(byte, "02X")
result = {"encoded": encoded, "raw": inputs["text"]}
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

### L028. pc-exec-memory-search

**Prose description:** Use the host boundary to search persistent memory via builtin.memory_search.

**Implementation:** Bind query and limit as typed data with actual primitive bounds. Distinguish successful zero matches from a failed search and expose the real result fields without artificial token truncation. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** query: string; limit: bounded positive integer, not bool. Return the retained search envelope, including ranking, source references and limits.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = host.memory_search(query=inputs["query"], limit=inputs["limit"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-memory-search

**Usage prose draft to specialize:** “Use the host boundary to search persistent memory via builtin.memory_search. Supply the typed inputs described for `pc-exec-memory-search`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-memory-search`'s block above.

#### skill-memory-search-broad

**Usage prose draft to specialize:** “Use the host boundary to search persistent memory via builtin.memory_search. Supply the typed inputs described for `pc-exec-memory-search`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-memory-search`'s block above.

### L029. pc-exec-memory-write

**Prose description:** Use the host boundary to write to persistent memory via builtin.memory_write.

**Implementation:** Bind content, target and append with explicit missing/null rules; inspect real daily_log append behavior. Keep this general target/replace usage: the packaged history writer only covers daily-log appending and is not an equivalent replacement. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association. **Duplicate review:** newer section-2 entries 2 overlap this functionality. Compare purpose, inputs/results and identity before converting; reuse compatible code and preserve distinct contracts.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** content: nonblank string accepted by the actual memory primitive; target: validated memory-relative string; append: boolean. daily_log always appends even when append=False; restrict replace usage to an actual replace-capable target. Return the actual status/path/append/content_length acknowledgement, not a fabricated success.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = host.memory_write(content=inputs["content"], target=inputs["target"], append=inputs["append"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-memory-write-log

**Usage prose draft to specialize:** “Use the host boundary to write to persistent memory via builtin.memory_write. Supply the typed inputs described for `pc-exec-memory-write`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-memory-write`'s block above.

#### skill-memory-write-main

**Usage prose draft to specialize:** “Use the host boundary to write to persistent memory via builtin.memory_write. Supply the typed inputs described for `pc-exec-memory-write`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-memory-write`'s block above.

### L030. pc-exec-memory-patch

**Prose description:** Use the host boundary for a targeted patch to a memory document via builtin.memory_write patch mode.

**Implementation:** Pass typed target and exact old/new strings with a declared replace_all boolean. Verify patch operation/result fields and do not replay a completed patch on a downstream failure. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** target: validated memory-relative string; old_string: nonempty string; new_string: string; replace_all: boolean. Return the actual patch acknowledgement.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = host.memory_write(target=inputs["target"], old_string=inputs["old_string"], new_string=inputs["new_string"], replace_all=inputs["replace_all"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-memory-write-patch

**Usage prose draft to specialize:** “Use the host boundary for a targeted patch to a memory document via builtin.memory_write patch mode. Supply the typed inputs described for `pc-exec-memory-patch`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-memory-patch`'s block above.

### L031. pc-exec-memory-read

**Prose description:** Use the host boundary to read a memory document by path via builtin.memory_read.

**Implementation:** Pass the memory-relative path as typed data and validate the actual successful payload. Missing documents and denied/failed reads must remain different from an empty successful document. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** path: validated memory-relative string. Return actual read content and source metadata; missing is a classified error, not empty content.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = host.memory_read(path=inputs["path"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-memory-read

**Usage prose draft to specialize:** “Use the host boundary to read a memory document by path via builtin.memory_read. Supply the typed inputs described for `pc-exec-memory-read`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-memory-read`'s block above.

### L032. pc-exec-memory-tree

**Prose description:** Use the host boundary to list the memory directory tree via builtin.memory_tree.

**Implementation:** Use typed path/depth with declared bounds and missing defaults. Declare each directory/item shape and any truncation; do not assume truthiness implements valid optional inputs. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** path: supported memory-relative string; depth: integer 1..10, not bool. Return recursive tree entries. Apply documented defaults only to missing input.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = host.memory_tree(path=inputs["path"], depth=inputs["depth"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-memory-tree

**Usage prose draft to specialize:** “Use the host boundary to list the memory directory tree via builtin.memory_tree. Supply the typed inputs described for `pc-exec-memory-tree`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-memory-tree`'s block above.

### L033. pc-memory-extract-section

**Prose description:** extracts a named section from a Markdown document using heading matching.

**Implementation:** Bind Markdown and heading as data. Define heading depth, nested sections, repeated headings and found-but-empty sections; return a distinct presence flag rather than conflating empty content with absence.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** content: string; heading: nonempty string; output: found boolean, nullable section_content, heading string. First matching ATX heading; include deeper subsections; empty present section differs from absence.

**Concrete caveat:** This example covers a deliberately restricted ATX-only document profile. It is not a full Markdown parser: Recipes using fenced code/Setext headings need trusted parser facts or an explicit upgraded parser before activation.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
lines = inputs["content"].split("\n")
found = False
level = 0
collected = []
for line in lines:
    prefix = len(line) - len(line.lstrip("#"))
    is_heading = 1 <= prefix <= 6 and line[prefix:prefix+1] == " "
    title = line[prefix+1:].strip() if is_heading else ""
    if not found and is_heading and title == inputs["heading"]:
        found = True
        level = prefix
    elif found and is_heading and prefix <= level:
        break
    elif found:
        collected.append(line)
result = {"found": found, "section_content": "\n".join(collected) if found else None,
          "heading": inputs["heading"]}
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

### L034. pc-memory-format-entry

**Prose description:** formats a memory entry string ready for appending to a memory document.

**Implementation:** Bind text and an explicitly supplied timestamp as typed strings. Keep formatting pure, publish its typed result to the later write, and do not access the ambient clock or assume task-local variables survive implicitly. **Duplicate review:** newer section-2 entries 3 overlap this functionality. Compare purpose, inputs/results and identity before converting; reuse compatible code and preserve distinct contracts.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** timestamp_str: validated single-line timestamp text; text: string; result: exact formatted_entry string. Timestamp retrieval is another Tool step. This differs from the packaged completed-turn formatter.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = {"formatted_entry": "### " + inputs["timestamp_str"] + "\n\n" + inputs["text"] + "\n"}
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

### L035. pc-exec-memory-append

**Prose description:** appends text to an existing memory document. Reads the current content via memory_read, then writes combined content via memory_write.

**Implementation:** Replace the read-concatenate-write body with the existing append operation if its exact semantics fit, using target rather than the unsupported write path keyword. Otherwise decompose and validate read/format/write steps and their race/effect behavior; never silently replace failed reads with empty content. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association. **Duplicate review:** newer section-2 entries 2 overlap this functionality. Compare purpose, inputs/results and identity before converting; reuse compatible code and preserve distinct contracts.

**Split/reuse:** Collapse to one real append usage, reusing pc-exec-memory-write with append=True. No separate read step or new generic string-concat component.

**Typed inputs/result:** target: validated memory-relative string; content: nonblank string prepared according to append formatting; output: actual append acknowledgement.

**Concrete caveat:** Replace read/concatenate/write with actual append primitive where its newline semantics satisfy the usage. If exact legacy blank-line insertion is required, document it as content formatting; do not reintroduce racy read-modify-write.

**Recipe wiring:** `pc-exec-memory-write`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.memory_write(target=inputs["target"], content=inputs["content"], append=True)
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-memory-write-append

**Usage prose draft to specialize:** “appends text to an existing memory document. Reads the current content via memory_read, then writes combined content via memory_write. Supply the typed inputs described for `pc-exec-memory-append`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-memory-append`'s block above.

### L036. pc-exec-shell-git-status

**Prose description:** runs 'git status' in the workspace root via builtin.shell. Command is a fixed literal. No user input enters the command string.

**Implementation:** Keep the fixed command literal, compile it in the pinned Monty worker, and add the exact shell usage association plus process/exit/output contracts. Remove the historical Tier-0/shell-safe-fixed claim: every consuming shell Recipe is Tier 1; classify real effects and preserve cancellation/completion uncertainty. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Reuse one shell dispatcher with this reviewed fixed command as typed Recipe data; retain distinct usage/effect contracts.

**Typed inputs/result:** No dynamic command arguments. Recipe constant command='git status'; return actual shell output/exit_code/success/sandboxed object.

**Recipe wiring:** `proposed-shell-dispatch`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.shell(command='git status')
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-shell-git-status

**Usage prose draft to specialize:** “runs 'git status' in the workspace root via builtin.shell. Command is a fixed literal. No user input enters the command string. Supply the typed inputs described for `pc-exec-shell-git-status`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-shell-git-status`'s block above.

### L037. pc-exec-shell-git-log

**Prose description:** runs 'git log --oneline -20' to get the last 20 commits. Fixed literal command.

**Implementation:** Keep the fixed command literal, compile it in the pinned Monty worker, and add the exact shell usage association plus process/exit/output contracts. Remove the historical Tier-0/shell-safe-fixed claim: every consuming shell Recipe is Tier 1; classify real effects and preserve cancellation/completion uncertainty. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Reuse one shell dispatcher with this reviewed fixed command as typed Recipe data; retain distinct usage/effect contracts.

**Typed inputs/result:** No dynamic command arguments. Recipe constant command='git log --oneline -20'; return actual shell output/exit_code/success/sandboxed object.

**Recipe wiring:** `proposed-shell-dispatch`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.shell(command='git log --oneline -20')
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-shell-git-log

**Usage prose draft to specialize:** “runs 'git log --oneline -20' to get the last 20 commits. Fixed literal command. Supply the typed inputs described for `pc-exec-shell-git-log`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-shell-git-log`'s block above.

### L038. pc-exec-shell-git-diff-stat

**Prose description:** runs 'git diff --stat' to show changed file summary. Fixed literal command.

**Implementation:** Keep the fixed command literal, compile it in the pinned Monty worker, and add the exact shell usage association plus process/exit/output contracts. Remove the historical Tier-0/shell-safe-fixed claim: every consuming shell Recipe is Tier 1; classify real effects and preserve cancellation/completion uncertainty. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Reuse one shell dispatcher with this reviewed fixed command as typed Recipe data; retain distinct usage/effect contracts.

**Typed inputs/result:** No dynamic command arguments. Recipe constant command='git diff --stat'; return actual shell output/exit_code/success/sandboxed object.

**Recipe wiring:** `proposed-shell-dispatch`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.shell(command='git diff --stat')
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-shell-git-diff-stat

**Usage prose draft to specialize:** “runs 'git diff --stat' to show changed file summary. Fixed literal command. Supply the typed inputs described for `pc-exec-shell-git-diff-stat`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-shell-git-diff-stat`'s block above.

### L039. pc-exec-shell-git-branch

**Prose description:** runs 'git branch -a' to list all local and remote branches. Fixed literal command.

**Implementation:** Keep the fixed command literal, compile it in the pinned Monty worker, and add the exact shell usage association plus process/exit/output contracts. Remove the historical Tier-0/shell-safe-fixed claim: every consuming shell Recipe is Tier 1; classify real effects and preserve cancellation/completion uncertainty. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Reuse one shell dispatcher with this reviewed fixed command as typed Recipe data; retain distinct usage/effect contracts.

**Typed inputs/result:** No dynamic command arguments. Recipe constant command='git branch -a'; return actual shell output/exit_code/success/sandboxed object.

**Recipe wiring:** `proposed-shell-dispatch`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.shell(command='git branch -a')
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-shell-git-branch

**Usage prose draft to specialize:** “runs 'git branch -a' to list all local and remote branches. Fixed literal command. Supply the typed inputs described for `pc-exec-shell-git-branch`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-shell-git-branch`'s block above.

### L040. pc-exec-shell-git-stash-list

**Prose description:** runs 'git stash list' to show the stash stack. Fixed literal command.

**Implementation:** Keep the fixed command literal, compile it in the pinned Monty worker, and add the exact shell usage association plus process/exit/output contracts. Remove the historical Tier-0/shell-safe-fixed claim: every consuming shell Recipe is Tier 1; classify real effects and preserve cancellation/completion uncertainty. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Reuse one shell dispatcher with this reviewed fixed command as typed Recipe data; retain distinct usage/effect contracts.

**Typed inputs/result:** No dynamic command arguments. Recipe constant command='git stash list'; return actual shell output/exit_code/success/sandboxed object.

**Recipe wiring:** `proposed-shell-dispatch`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.shell(command='git stash list')
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-shell-git-stash-list

**Usage prose draft to specialize:** “runs 'git stash list' to show the stash stack. Fixed literal command. Supply the typed inputs described for `pc-exec-shell-git-stash-list`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-shell-git-stash-list`'s block above.

### L041. pc-exec-shell-git-log-n

**Prose description:** runs 'git log --oneline -N' where N is a validated integer (1–100).

**Implementation:** Bind an integer count in 1–100, rejecting booleans and invalid values rather than silently resetting them to 20. Construct only the validated command argument; every consuming shell Recipe remains Tier 1.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** count: integer 1..100, not bool; missing default 20 only at input binding; output actual shell output/exit_code/success/sandboxed object. Invalid values never become default20.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
count = inputs["count"]
if type(count) is not int or not 1 <= count <= 100:
    raise ValueError("invalid log count")
result = host.shell(command="git log --oneline -" + str(count))
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

### L042. pc-exec-shell-git-remote

**Prose description:** runs 'git remote -v' to list all configured remote repositories and their URLs. Fixed literal command.

**Implementation:** Keep the fixed command literal, compile it in the pinned Monty worker, and add the exact shell usage association plus process/exit/output contracts. Remove the historical Tier-0/shell-safe-fixed claim: every consuming shell Recipe is Tier 1; classify real effects and preserve cancellation/completion uncertainty. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association. Verify secret filtering for environment values, configuration, process arguments and credential-bearing URLs before exposing output.

**Split/reuse:** Reuse one shell dispatcher with this reviewed fixed command as typed Recipe data; retain distinct usage/effect contracts.

**Typed inputs/result:** No dynamic command arguments. Recipe constant command='git remote -v'; return actual shell output/exit_code/success/sandboxed object.

**Recipe wiring:** `proposed-shell-dispatch`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.shell(command='git remote -v')
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-shell-git-remote

**Usage prose draft to specialize:** “runs 'git remote -v' to list all configured remote repositories and their URLs. Fixed literal command. Supply the typed inputs described for `pc-exec-shell-git-remote`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-shell-git-remote`'s block above.

### L043. pc-exec-shell-git-show-stat

**Prose description:** runs 'git show --stat HEAD' to show the last commit's changed files and line counts. Fixed literal command.

**Implementation:** Keep the fixed command literal, compile it in the pinned Monty worker, and add the exact shell usage association plus process/exit/output contracts. Remove the historical Tier-0/shell-safe-fixed claim: every consuming shell Recipe is Tier 1; classify real effects and preserve cancellation/completion uncertainty. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Reuse one shell dispatcher with this reviewed fixed command as typed Recipe data; retain distinct usage/effect contracts.

**Typed inputs/result:** No dynamic command arguments. Recipe constant command='git show --stat HEAD'; return actual shell output/exit_code/success/sandboxed object.

**Recipe wiring:** `proposed-shell-dispatch`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.shell(command='git show --stat HEAD')
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-shell-git-show-stat

**Usage prose draft to specialize:** “runs 'git show --stat HEAD' to show the last commit's changed files and line counts. Fixed literal command. Supply the typed inputs described for `pc-exec-shell-git-show-stat`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-shell-git-show-stat`'s block above.

### L044. pc-exec-shell-git-tag-list

**Prose description:** runs 'git tag --list' to enumerate all tags in the repository. Fixed literal command.

**Implementation:** Keep the fixed command literal, compile it in the pinned Monty worker, and add the exact shell usage association plus process/exit/output contracts. Remove the historical Tier-0/shell-safe-fixed claim: every consuming shell Recipe is Tier 1; classify real effects and preserve cancellation/completion uncertainty. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Reuse one shell dispatcher with this reviewed fixed command as typed Recipe data; retain distinct usage/effect contracts.

**Typed inputs/result:** No dynamic command arguments. Recipe constant command='git tag --list'; return actual shell output/exit_code/success/sandboxed object.

**Recipe wiring:** `proposed-shell-dispatch`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.shell(command='git tag --list')
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-shell-git-tag-list

**Usage prose draft to specialize:** “runs 'git tag --list' to enumerate all tags in the repository. Fixed literal command. Supply the typed inputs described for `pc-exec-shell-git-tag-list`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-shell-git-tag-list`'s block above.

### L045. pc-exec-shell-git-diff-name-only

**Prose description:** runs 'git diff --name-only HEAD' to list only the names of files changed since the last commit. No content shown. Fixed literal command — no slot interpolation.

**Implementation:** Keep the fixed command literal, compile it in the pinned Monty worker, and add the exact shell usage association plus process/exit/output contracts. Remove the historical Tier-0/shell-safe-fixed claim: every consuming shell Recipe is Tier 1; classify real effects and preserve cancellation/completion uncertainty. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Reuse one shell dispatcher with this reviewed fixed command as typed Recipe data; retain distinct usage/effect contracts.

**Typed inputs/result:** No dynamic command arguments. Recipe constant command='git diff --name-only HEAD'; return actual shell output/exit_code/success/sandboxed object.

**Recipe wiring:** `proposed-shell-dispatch`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.shell(command='git diff --name-only HEAD')
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-shell-git-diff-name-only

**Usage prose draft to specialize:** “runs 'git diff --name-only HEAD' to list only the names of files changed since the last commit. No content shown. Fixed literal command — no slot interpolation. Supply the typed inputs described for `pc-exec-shell-git-diff-name-only`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-shell-git-diff-name-only`'s block above.

### L046. pc-exec-shell-git-log-stat

**Prose description:** runs 'git log --stat --oneline -5' to show the last 5 commits with file-change counts per commit. Fixed literal.

**Implementation:** Keep the fixed command literal, compile it in the pinned Monty worker, and add the exact shell usage association plus process/exit/output contracts. Remove the historical Tier-0/shell-safe-fixed claim: every consuming shell Recipe is Tier 1; classify real effects and preserve cancellation/completion uncertainty. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Reuse one shell dispatcher with this reviewed fixed command as typed Recipe data; retain distinct usage/effect contracts.

**Typed inputs/result:** No dynamic command arguments. Recipe constant command='git log --stat --oneline -5'; return actual shell output/exit_code/success/sandboxed object.

**Recipe wiring:** `proposed-shell-dispatch`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.shell(command='git log --stat --oneline -5')
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-shell-git-log-stat

**Usage prose draft to specialize:** “runs 'git log --stat --oneline -5' to show the last 5 commits with file-change counts per commit. Fixed literal. Supply the typed inputs described for `pc-exec-shell-git-log-stat`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-shell-git-log-stat`'s block above.

### L047. pc-exec-shell-git-stash-show

**Prose description:** runs 'git stash show' to show the diff summary of the most recent stash entry. Fixed literal command.

**Implementation:** Keep the fixed command literal, compile it in the pinned Monty worker, and add the exact shell usage association plus process/exit/output contracts. Remove the historical Tier-0/shell-safe-fixed claim: every consuming shell Recipe is Tier 1; classify real effects and preserve cancellation/completion uncertainty. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Reuse one shell dispatcher with this reviewed fixed command as typed Recipe data; retain distinct usage/effect contracts.

**Typed inputs/result:** No dynamic command arguments. Recipe constant command='git stash show'; return actual shell output/exit_code/success/sandboxed object.

**Recipe wiring:** `proposed-shell-dispatch`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.shell(command='git stash show')
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-shell-git-stash-show

**Usage prose draft to specialize:** “runs 'git stash show' to show the diff summary of the most recent stash entry. Fixed literal command. Supply the typed inputs described for `pc-exec-shell-git-stash-show`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-shell-git-stash-show`'s block above.

### L048. pc-exec-shell-git-config-list

**Prose description:** runs 'git config --list' to show all active git configuration values. Fixed literal command.

**Implementation:** Keep the fixed command literal, compile it in the pinned Monty worker, and add the exact shell usage association plus process/exit/output contracts. Remove the historical Tier-0/shell-safe-fixed claim: every consuming shell Recipe is Tier 1; classify real effects and preserve cancellation/completion uncertainty. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association. Verify secret filtering for environment values, configuration, process arguments and credential-bearing URLs before exposing output.

**Split/reuse:** Reuse one shell dispatcher with this reviewed fixed command as typed Recipe data; retain distinct usage/effect contracts.

**Typed inputs/result:** No dynamic command arguments. Recipe constant command='git config --list'; return actual shell output/exit_code/success/sandboxed object.

**Recipe wiring:** `proposed-shell-dispatch`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.shell(command='git config --list')
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-shell-git-config-list

**Usage prose draft to specialize:** “runs 'git config --list' to show all active git configuration values. Fixed literal command. Supply the typed inputs described for `pc-exec-shell-git-config-list`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-shell-git-config-list`'s block above.

### L049. pc-exec-shell-git-add

**Prose description:** Use the host boundary to run 'git add <path>'.

**Implementation:** Bind explicit selected paths; remove the empty-input default to git add . and shell-string concatenation. Use a verified argv adapter or reviewed POSIX quoting with option termination, and keep shell workflows Tier 1. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Split reusable POSIX quoting from command-specific preparation and shared shell dispatch. Keep tiny command joining internal to the usage unless another consumer needs its own contract.

**Typed inputs/result:** quoted_arguments: output of proposed-shell-quote-arguments; exact count: commit/wc=1, push=2, pull=1 or 2, add=1 or more. For push/pull validate permitted remote/ref and reject leading - before quoting. Result: command string.

**Concrete caveat:** This command-specific preparation is internally composed with the shared quoting helper; it is not by itself a Tool-usage Skill. Confirm POSIX shell and installed command option grammar. Quoting alone never prevents option injection. No implicit git add . or branch/remote fallback.

**Recipe wiring:** `proposed-shell-quote-arguments` → `proposed-shell-dispatch`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = "git add -- " + " ".join(inputs["quoted_arguments"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-shell-git-add

**Usage prose draft to specialize:** “Use the host boundary to run 'git add <path>'. Supply the typed inputs described for `pc-exec-shell-git-add`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** the local pure/preparation snippet is not this Skill's executor. Associate this Skill only with the relevant single-Tool retrieval/write/removal usage in the Recipe above; pure portions stay separately referenced PythonCode. A pure-only historical wrapper becomes PythonCode documentation, not a zero-Tool Skill. For multi-Tool historical prose, retain task behavior in the Recipe and author separate usage Skills.

### L050. pc-exec-shell-git-commit

**Prose description:** Use the host boundary to run 'git commit -m <msg>'.

**Implementation:** Bind the message as data; Python repr is not shell quoting. Use a supported argv/quoting boundary, preserve quotes/newlines exactly, validate commit outcome and never replay a completed commit; shell workflows remain Tier 1. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Split reusable POSIX quoting from command-specific preparation and shared shell dispatch. Keep tiny command joining internal to the usage unless another consumer needs its own contract.

**Typed inputs/result:** quoted_arguments: output of proposed-shell-quote-arguments; exact count: commit/wc=1, push=2, pull=1 or 2, add=1 or more. For push/pull validate permitted remote/ref and reject leading - before quoting. Result: command string.

**Concrete caveat:** This command-specific preparation is internally composed with the shared quoting helper; it is not by itself a Tool-usage Skill. Confirm POSIX shell and installed command option grammar. Quoting alone never prevents option injection. No implicit git add . or branch/remote fallback.

**Recipe wiring:** `proposed-shell-quote-arguments` → `proposed-shell-dispatch`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = "git commit -m " + inputs["quoted_arguments"][0]
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-shell-git-commit

**Usage prose draft to specialize:** “Use the host boundary to run 'git commit -m <msg>'. Supply the typed inputs described for `pc-exec-shell-git-commit`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** the local pure/preparation snippet is not this Skill's executor. Associate this Skill only with the relevant single-Tool retrieval/write/removal usage in the Recipe above; pure portions stay separately referenced PythonCode. A pure-only historical wrapper becomes PythonCode documentation, not a zero-Tool Skill. For multi-Tool historical prose, retain task behavior in the Recipe and author separate usage Skills.

### L051. pc-exec-shell-git-push

**Prose description:** Use the host boundary to run 'git push <remote> <branch>'.

**Implementation:** Bind remote/branch with explicit missing defaults and reject invalid or option-like values. Replace shell concatenation with verified argument handling, declare network/mutation effects and unknown-completion behavior, and keep shell workflows Tier 1. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Split reusable POSIX quoting from command-specific preparation and shared shell dispatch. Keep tiny command joining internal to the usage unless another consumer needs its own contract.

**Typed inputs/result:** quoted_arguments: output of proposed-shell-quote-arguments; exact count: commit/wc=1, push=2, pull=1 or 2, add=1 or more. For push/pull validate permitted remote/ref and reject leading - before quoting. Result: command string.

**Concrete caveat:** This command-specific preparation is internally composed with the shared quoting helper; it is not by itself a Tool-usage Skill. Confirm POSIX shell and installed command option grammar. Quoting alone never prevents option injection. No implicit git add . or branch/remote fallback.

**Recipe wiring:** `proposed-shell-quote-arguments` → `proposed-shell-dispatch`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = "git push " + " ".join(inputs["quoted_arguments"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-shell-git-push

**Usage prose draft to specialize:** “Use the host boundary to run 'git push <remote> <branch>'. Supply the typed inputs described for `pc-exec-shell-git-push`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** the local pure/preparation snippet is not this Skill's executor. Associate this Skill only with the relevant single-Tool retrieval/write/removal usage in the Recipe above; pure portions stay separately referenced PythonCode. A pure-only historical wrapper becomes PythonCode documentation, not a zero-Tool Skill. For multi-Tool historical prose, retain task behavior in the Recipe and author separate usage Skills.

### L052. pc-exec-shell-git-pull

**Prose description:** Use the host boundary to run 'git pull <remote> <branch>'.

**Implementation:** Bind remote/branch with explicit missing defaults and reject invalid or option-like values. Replace shell concatenation with verified argument handling, declare network/mutation effects and unknown-completion behavior, and keep shell workflows Tier 1. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Split reusable POSIX quoting from command-specific preparation and shared shell dispatch. Keep tiny command joining internal to the usage unless another consumer needs its own contract.

**Typed inputs/result:** quoted_arguments: output of proposed-shell-quote-arguments; exact count: commit/wc=1, push=2, pull=1 or 2, add=1 or more. For push/pull validate permitted remote/ref and reject leading - before quoting. Result: command string.

**Concrete caveat:** This command-specific preparation is internally composed with the shared quoting helper; it is not by itself a Tool-usage Skill. Confirm POSIX shell and installed command option grammar. Quoting alone never prevents option injection. No implicit git add . or branch/remote fallback.

**Recipe wiring:** `proposed-shell-quote-arguments` → `proposed-shell-dispatch`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = "git pull " + " ".join(inputs["quoted_arguments"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-shell-git-pull

**Usage prose draft to specialize:** “Use the host boundary to run 'git pull <remote> <branch>'. Supply the typed inputs described for `pc-exec-shell-git-pull`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** the local pure/preparation snippet is not this Skill's executor. Associate this Skill only with the relevant single-Tool retrieval/write/removal usage in the Recipe above; pure portions stay separately referenced PythonCode. A pure-only historical wrapper becomes PythonCode documentation, not a zero-Tool Skill. For multi-Tool historical prose, retain task behavior in the Recipe and author separate usage Skills.

### L053. pc-exec-shell-git-fetch

**Prose description:** Use the host boundary to run 'git fetch --all'.

**Implementation:** Keep the fixed command literal, compile it in the pinned Monty worker, and add the exact shell usage association plus process/exit/output contracts. Remove the historical Tier-0/shell-safe-fixed claim: every consuming shell Recipe is Tier 1; classify real effects and preserve cancellation/completion uncertainty. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Reuse one shell dispatcher with this reviewed fixed command as typed Recipe data; retain distinct usage/effect contracts.

**Typed inputs/result:** No dynamic command arguments. Recipe constant command='git fetch --all'; return actual shell output/exit_code/success/sandboxed object.

**Recipe wiring:** `proposed-shell-dispatch`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.shell(command='git fetch --all')
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-shell-git-fetch

**Usage prose draft to specialize:** “Use the host boundary to run 'git fetch --all'. Supply the typed inputs described for `pc-exec-shell-git-fetch`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-shell-git-fetch`'s block above.

### L054. pc-exec-shell-pwd

**Prose description:** runs 'pwd' to show the current working directory. Fixed literal command.

**Implementation:** Keep the fixed command literal, compile it in the pinned Monty worker, and add the exact shell usage association plus process/exit/output contracts. Remove the historical Tier-0/shell-safe-fixed claim: every consuming shell Recipe is Tier 1; classify real effects and preserve cancellation/completion uncertainty. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Reuse one shell dispatcher with this reviewed fixed command as typed Recipe data; retain distinct usage/effect contracts.

**Typed inputs/result:** No dynamic command arguments. Recipe constant command='pwd'; return actual shell output/exit_code/success/sandboxed object.

**Recipe wiring:** `proposed-shell-dispatch`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.shell(command='pwd')
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-shell-pwd

**Usage prose draft to specialize:** “runs 'pwd' to show the current working directory. Fixed literal command. Supply the typed inputs described for `pc-exec-shell-pwd`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-shell-pwd`'s block above.

### L055. pc-exec-shell-df

**Prose description:** runs 'df -h' to show disk usage in human-readable format. Fixed literal command.

**Implementation:** Keep the fixed command literal, compile it in the pinned Monty worker, and add the exact shell usage association plus process/exit/output contracts. Remove the historical Tier-0/shell-safe-fixed claim: every consuming shell Recipe is Tier 1; classify real effects and preserve cancellation/completion uncertainty. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Reuse one shell dispatcher with this reviewed fixed command as typed Recipe data; retain distinct usage/effect contracts.

**Typed inputs/result:** No dynamic command arguments. Recipe constant command='df -h'; return actual shell output/exit_code/success/sandboxed object.

**Recipe wiring:** `proposed-shell-dispatch`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.shell(command='df -h')
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-shell-df

**Usage prose draft to specialize:** “runs 'df -h' to show disk usage in human-readable format. Fixed literal command. Supply the typed inputs described for `pc-exec-shell-df`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-shell-df`'s block above.

### L056. pc-exec-shell-ps

**Prose description:** runs 'ps aux' to list running processes. Fixed literal command.

**Implementation:** Keep the fixed command literal, compile it in the pinned Monty worker, and add the exact shell usage association plus process/exit/output contracts. Remove the historical Tier-0/shell-safe-fixed claim: every consuming shell Recipe is Tier 1; classify real effects and preserve cancellation/completion uncertainty. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association. Verify secret filtering for environment values, configuration, process arguments and credential-bearing URLs before exposing output.

**Split/reuse:** Reuse one shell dispatcher with this reviewed fixed command as typed Recipe data; retain distinct usage/effect contracts.

**Typed inputs/result:** No dynamic command arguments. Recipe constant command='ps aux'; return actual shell output/exit_code/success/sandboxed object.

**Recipe wiring:** `proposed-shell-dispatch`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.shell(command='ps aux')
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-shell-ps

**Usage prose draft to specialize:** “runs 'ps aux' to list running processes. Fixed literal command. Supply the typed inputs described for `pc-exec-shell-ps`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-shell-ps`'s block above.

### L057. pc-exec-shell-env

**Prose description:** runs 'env' to list all environment variables in the current session. Fixed literal command.

**Implementation:** Keep the fixed command literal, compile it in the pinned Monty worker, and add the exact shell usage association plus process/exit/output contracts. Remove the historical Tier-0/shell-safe-fixed claim: every consuming shell Recipe is Tier 1; classify real effects and preserve cancellation/completion uncertainty. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association. Verify secret filtering for environment values, configuration, process arguments and credential-bearing URLs before exposing output.

**Split/reuse:** Reuse one shell dispatcher with this reviewed fixed command as typed Recipe data; retain distinct usage/effect contracts.

**Typed inputs/result:** No dynamic command arguments. Recipe constant command='env'; return actual shell output/exit_code/success/sandboxed object.

**Recipe wiring:** `proposed-shell-dispatch`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.shell(command='env')
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-shell-env

**Usage prose draft to specialize:** “runs 'env' to list all environment variables in the current session. Fixed literal command. Supply the typed inputs described for `pc-exec-shell-env`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-shell-env`'s block above.

### L058. pc-exec-shell-uname

**Prose description:** runs 'uname -a' to show OS/kernel information. Fixed literal command.

**Implementation:** Keep the fixed command literal, compile it in the pinned Monty worker, and add the exact shell usage association plus process/exit/output contracts. Remove the historical Tier-0/shell-safe-fixed claim: every consuming shell Recipe is Tier 1; classify real effects and preserve cancellation/completion uncertainty. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Reuse one shell dispatcher with this reviewed fixed command as typed Recipe data; retain distinct usage/effect contracts.

**Typed inputs/result:** No dynamic command arguments. Recipe constant command='uname -a'; return actual shell output/exit_code/success/sandboxed object.

**Recipe wiring:** `proposed-shell-dispatch`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.shell(command='uname -a')
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-shell-uname

**Usage prose draft to specialize:** “runs 'uname -a' to show OS/kernel information. Fixed literal command. Supply the typed inputs described for `pc-exec-shell-uname`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-shell-uname`'s block above.

### L059. pc-exec-shell-which

**Prose description:** runs 'which <toolname>' to locate a binary.

**Implementation:** Bind a validated command name and use full-string regex validation in the pinned Monty re engine. Do not return an ordinary successful payload for invalid input; retain shell result semantics and Tier 1. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** name: bounded ASCII binary name, no leading hyphen; output actual shell output/exit_code/success/sandboxed object. Unavailable binary is process failure, not invalid-input success.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
import re
name = inputs["name"]
if re.fullmatch(r"[A-Za-z0-9_][A-Za-z0-9_-]{0,63}", name) is None:
    raise ValueError("invalid binary name")
result = host.shell(command="which " + name)
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-shell-which

**Usage prose draft to specialize:** “runs 'which <toolname>' to locate a binary. Supply the typed inputs described for `pc-exec-shell-which`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-shell-which`'s block above.

### L060. pc-exec-shell-hostname

**Prose description:** runs 'hostname' to print the machine hostname. Fixed literal command.

**Implementation:** Keep the fixed command literal, compile it in the pinned Monty worker, and add the exact shell usage association plus process/exit/output contracts. Remove the historical Tier-0/shell-safe-fixed claim: every consuming shell Recipe is Tier 1; classify real effects and preserve cancellation/completion uncertainty. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Reuse one shell dispatcher with this reviewed fixed command as typed Recipe data; retain distinct usage/effect contracts.

**Typed inputs/result:** No dynamic command arguments. Recipe constant command='hostname'; return actual shell output/exit_code/success/sandboxed object.

**Recipe wiring:** `proposed-shell-dispatch`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.shell(command='hostname')
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-shell-hostname

**Usage prose draft to specialize:** “runs 'hostname' to print the machine hostname. Fixed literal command. Supply the typed inputs described for `pc-exec-shell-hostname`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-shell-hostname`'s block above.

### L061. pc-exec-shell-whoami

**Prose description:** runs 'whoami' to print the current user account name. Fixed literal command.

**Implementation:** Keep the fixed command literal, compile it in the pinned Monty worker, and add the exact shell usage association plus process/exit/output contracts. Remove the historical Tier-0/shell-safe-fixed claim: every consuming shell Recipe is Tier 1; classify real effects and preserve cancellation/completion uncertainty. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Reuse one shell dispatcher with this reviewed fixed command as typed Recipe data; retain distinct usage/effect contracts.

**Typed inputs/result:** No dynamic command arguments. Recipe constant command='whoami'; return actual shell output/exit_code/success/sandboxed object.

**Recipe wiring:** `proposed-shell-dispatch`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.shell(command='whoami')
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-shell-whoami

**Usage prose draft to specialize:** “runs 'whoami' to print the current user account name. Fixed literal command. Supply the typed inputs described for `pc-exec-shell-whoami`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-shell-whoami`'s block above.

### L062. pc-exec-shell-uptime

**Prose description:** runs 'uptime' to show system uptime, load average, and logged-in user count. Fixed literal command.

**Implementation:** Keep the fixed command literal, compile it in the pinned Monty worker, and add the exact shell usage association plus process/exit/output contracts. Remove the historical Tier-0/shell-safe-fixed claim: every consuming shell Recipe is Tier 1; classify real effects and preserve cancellation/completion uncertainty. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Reuse one shell dispatcher with this reviewed fixed command as typed Recipe data; retain distinct usage/effect contracts.

**Typed inputs/result:** No dynamic command arguments. Recipe constant command='uptime'; return actual shell output/exit_code/success/sandboxed object.

**Recipe wiring:** `proposed-shell-dispatch`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.shell(command='uptime')
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-shell-uptime

**Usage prose draft to specialize:** “runs 'uptime' to show system uptime, load average, and logged-in user count. Fixed literal command. Supply the typed inputs described for `pc-exec-shell-uptime`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-shell-uptime`'s block above.

### L063. pc-exec-shell-free

**Prose description:** runs 'free -h' to show memory usage in human-readable format. Fixed literal command.

**Implementation:** Keep the fixed command literal, compile it in the pinned Monty worker, and add the exact shell usage association plus process/exit/output contracts. Remove the historical Tier-0/shell-safe-fixed claim: every consuming shell Recipe is Tier 1; classify real effects and preserve cancellation/completion uncertainty. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Reuse one shell dispatcher with this reviewed fixed command as typed Recipe data; retain distinct usage/effect contracts.

**Typed inputs/result:** No dynamic command arguments. Recipe constant command='free -h'; return actual shell output/exit_code/success/sandboxed object.

**Recipe wiring:** `proposed-shell-dispatch`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.shell(command='free -h')
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-shell-free

**Usage prose draft to specialize:** “runs 'free -h' to show memory usage in human-readable format. Fixed literal command. Supply the typed inputs described for `pc-exec-shell-free`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-shell-free`'s block above.

### L064. pc-exec-shell-wc-l

**Prose description:** runs 'wc -l <filepath>' to count lines in a file.

**Implementation:** Bind the path as data; the existing regex admits option-looking paths and is not a containment proof. Use verified quoting/argv and option termination, retain host path checks and distinguish process failure from a count; shell workflows remain Tier 1. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Split reusable POSIX quoting from command-specific preparation and shared shell dispatch. Keep tiny command joining internal to the usage unless another consumer needs its own contract.

**Typed inputs/result:** quoted_arguments: output of proposed-shell-quote-arguments; exact count: commit/wc=1, push=2, pull=1 or 2, add=1 or more. For push/pull validate permitted remote/ref and reject leading - before quoting. Result: command string.

**Concrete caveat:** This command-specific preparation is internally composed with the shared quoting helper; it is not by itself a Tool-usage Skill. Confirm POSIX shell and installed command option grammar. Quoting alone never prevents option injection. No implicit git add . or branch/remote fallback.

**Recipe wiring:** `proposed-shell-quote-arguments` → `proposed-shell-dispatch`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = "wc -l -- " + inputs["quoted_arguments"][0]
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-shell-wc-l

**Usage prose draft to specialize:** “runs 'wc -l <filepath>' to count lines in a file. Supply the typed inputs described for `pc-exec-shell-wc-l`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** the local pure/preparation snippet is not this Skill's executor. Associate this Skill only with the relevant single-Tool retrieval/write/removal usage in the Recipe above; pure portions stay separately referenced PythonCode. A pure-only historical wrapper becomes PythonCode documentation, not a zero-Tool Skill. For multi-Tool historical prose, retain task behavior in the Recipe and author separate usage Skills.

### L065. pc-exec-trigger-list

**Audit correction:** The retained registered schema/handler accepts limit, not scope. all/active/scheduled are Recipe filtering usages over validated state/is_active/source fields; no scope selector may be dispatched. The inspected capped list cannot prove a globally unique name or absence. A complete enumeration or qualified exact lookup is required before name-based removal.

**Prose description:** calls host.trigger_list to list configured triggers.

**Implementation:** Replace unsupported scope dispatch with the actual limit input (1..100) and validated trigger-list records. Active/scheduled are separately reviewed pure filters, not host parameters. The capped result has no completeness metadata; do not infer unique names or absence from it. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** limit: integer 1..100, not bool; result: object with required triggers list of exact trigger_output records. Current primitive returns no completeness/pagination flag.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = host.trigger_list(limit=inputs["limit"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-trigger-list

**Usage prose draft to specialize:** “calls host.trigger_list to list configured triggers. Supply the typed inputs described for `pc-exec-trigger-list`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-trigger-list`'s block above.

#### skill-trigger-list-active

**Usage prose draft to specialize:** “calls host.trigger_list to list configured triggers. Supply the typed inputs described for `pc-exec-trigger-list`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-trigger-list`'s block above.

#### skill-trigger-list-scheduled

**Usage prose draft to specialize:** “calls host.trigger_list to list configured triggers. Supply the typed inputs described for `pc-exec-trigger-list`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-trigger-list`'s block above.

### L066. pc-exec-trigger-resolve-and-remove

**Audit correction:** BLOCKED name-resolution workflow until complete lookup is qualified. The final independently bound remove call uses trigger_id. Do not convert this legacy name workflow into guessed trigger_name dispatch or a scoped role requirement.

**Prose description:** lists all triggers, finds the one matching the given name exactly, and removes it.

**Implementation:** Split listing, exact-name selection and removal into explicit usages with typed handoffs. Check duplicate/missing names and the actual removal acknowledgement; the current removed=True wrapper must not conceal a failed remove, and the second call is not bound by the list binding.

**Split/reuse:** Split list → name equality filter → require one → separately bound removal. Return actual removal result and fail on ambiguity.

**Typed inputs/result:** rows: exact filtered trigger record list; complete: trusted full-query enumeration evidence, not available from the current capped list alone. Output: one exact trigger record containing trigger_id.

**Recipe wiring:** `pc-exec-trigger-list` → `proposed-filter-records` → `proposed-require-one-record` → `proposed-trigger-remove`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
if not inputs["complete"]:
    raise ValueError("incomplete lookup")
if len(inputs["rows"]) == 0:
    raise ValueError("lookup_not_found")
if len(inputs["rows"]) != 1:
    raise ValueError("lookup_ambiguous")
result = inputs["rows"][0]
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

### L067. pc-exec-time-now

**Prose description:** Use the host boundary to get the current timestamp via builtin.time operation='now'.

**Implementation:** Bind optional timezone as typed inputs with explicit missing/null rules. Verify the real time-operation keyword/result contract and boundary cases; retain the host time primitive rather than substituting ambient Python time. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** timezone: validated supported timezone string. Return actual iso/utc_iso/unix/unix_millis and timezone fields for this profile.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = host.time(operation="now", timezone=inputs["timezone"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-time-now

**Usage prose draft to specialize:** “Use the host boundary to get the current timestamp via builtin.time operation='now'. Supply the typed inputs described for `pc-exec-time-now`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-time-now`'s block above.

### L068. pc-exec-time-parse

**Prose description:** Use the host boundary to parse a timestamp string via builtin.time operation='parse'.

**Implementation:** Bind timestamp and optional timezone as typed inputs with explicit missing/null rules. Verify the real time-operation keyword/result contract and boundary cases; retain the host time primitive rather than substituting ambient Python time. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** timestamp: string; timezone: supported timezone string. Return iso/unix/unix_millis; invalid or ambiguous input remains an error.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = host.time(operation="parse", input=inputs["timestamp"], timezone=inputs["timezone"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-time-parse

**Usage prose draft to specialize:** “Use the host boundary to parse a timestamp string via builtin.time operation='parse'. Supply the typed inputs described for `pc-exec-time-parse`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-time-parse`'s block above.

### L069. pc-exec-time-convert

**Prose description:** Use the host boundary to convert a timestamp between timezones via builtin.time operation='convert'.

**Implementation:** Bind timestamp and source/destination timezones as typed inputs with explicit missing/null rules. Verify the real time-operation keyword/result contract and boundary cases; retain the host time primitive rather than substituting ambient Python time. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** timestamp/from_timezone/to_timezone: strings validated against the actual time adapter. Return its input/utc_iso/output/timezone object.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = host.time(operation="convert", input=inputs["timestamp"], from_timezone=inputs["from_timezone"], to_timezone=inputs["to_timezone"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-time-convert

**Usage prose draft to specialize:** “Use the host boundary to convert a timestamp between timezones via builtin.time operation='convert'. Supply the typed inputs described for `pc-exec-time-convert`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-time-convert`'s block above.

### L070. pc-exec-time-diff

**Prose description:** Use the host boundary to compute the signed difference between two timestamps via builtin.time operation='diff'.

**Implementation:** Bind both timestamps as typed inputs with explicit missing/null rules. Verify the real time-operation keyword/result contract and boundary cases; retain the host time primitive rather than substituting ambient Python time. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** timestamp/timestamp2: strings accepted by the retained parser. Return actual signed seconds and other declared difference fields.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = host.time(operation="diff", input=inputs["timestamp"], timestamp2=inputs["timestamp2"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-time-diff

**Usage prose draft to specialize:** “Use the host boundary to compute the signed difference between two timestamps via builtin.time operation='diff'. Supply the typed inputs described for `pc-exec-time-diff`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-time-diff`'s block above.

### L071. pc-exec-time-format

**Prose description:** Use the host boundary to format a timestamp as a human-readable string via builtin.time operation='format'.

**Implementation:** Bind timestamp, format string and optional timezone as typed inputs with explicit missing/null rules. Verify the real time-operation keyword/result contract and boundary cases; retain the host time primitive rather than substituting ambient Python time. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** timestamp/format_string/timezone: strings accepted by the retained adapter. Return the actual formatted result; formatting is not a second clock read.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = host.time(operation="format", input=inputs["timestamp"], format_string=inputs["format_string"], timezone=inputs["timezone"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-time-format

**Usage prose draft to specialize:** “Use the host boundary to format a timestamp as a human-readable string via builtin.time operation='format'. Supply the typed inputs described for `pc-exec-time-format`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-time-format`'s block above.

### L072. pc-exec-json-query

**Audit correction:** The host reparses string data as JSON text. Do not claim an arbitrary recursively union-typed input/result or change that behavior silently. Dynamic paths with incompatible result types need separately reviewed profiles or explicit supported schema work; never add an invented any field.

**Prose description:** Use the host boundary for json query operation.

**Implementation:** Bind structured data and a typed selector to the actual host.json query operation. Preserve its missing-path failure separately from present null values; the typed parse example in tools.md does not establish a replacement query executor. Reuse its contract pattern without duplicating parsing or pure extraction. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association. **Duplicate review:** newer section-2 entries 6 overlap this functionality. Compare purpose, inputs/results and identity before converting; reuse compatible code and preserve distinct contracts.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** data: one exact supported object/list schema, or a distinct JSON-text input profile; path: actual host dot/bracket query string. Result: one exact per-usage selected-value schema, nullable only when allowed by that profile. Missing field/index is an InputEncode error; presentnull remains null.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = host.json(operation="query", data=inputs["data"], path=inputs["path"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-json-query

**Usage prose draft to specialize:** “Use the host boundary for json query operation. Supply the typed inputs described for `pc-exec-json-query`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-json-query`'s block above.

### L073. pc-exec-json-stringify

**Audit correction:** Actual builtin.json stringify reparses any string input as JSON text. Passing raw hello does not serialize the string hello. This text-in profile preserves that actual behavior; typed-object profiles may reuse code only with an exact object/list shape. Pure literal-string serialization can use reviewed Monty json.dumps(..., allow_nan=False); it is a different zero-Tool contract, not this Skill.

**Prose description:** Use the host boundary for json stringify or parse.

**Implementation:** Fix stringify as one operation and reuse the typed parse usage separately. The current host reparses string data as JSON text; the concrete text-in stringify profile returns pretty JSON text. Typed-object/list profiles require exact finite schemas; a raw string must not be misrepresented as literal-string serialization. Do not coerce arbitrary objects to strings or assume parsing proves schema validity. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association. **Duplicate review:** newer section-2 entries 6 overlap this functionality. Compare purpose, inputs/results and identity before converting; reuse compatible code and preserve distinct contracts.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** json_text: valid JSON text string; result: pretty-printed JSON string. Parsing/finite/size/depth semantics must match the retained Rust serde_json adapter.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = host.json(operation="stringify", data=inputs["json_text"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-json-stringify

**Usage prose draft to specialize:** “Use the host boundary for json stringify or parse. Supply the typed inputs described for `pc-exec-json-stringify`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-json-stringify`'s block above.

### L074. pc-exec-json-validate

**Prose description:** Use the host boundary to validate a JSON string.

**Implementation:** Bind JSON text as one string and keep syntax validity separate from recursive component-schema approval. Specify invalid JSON versus host failure and nonfinite-number behavior; use the actual host validator result fields. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association. **Duplicate review:** newer section-2 entries 6 overlap this functionality. Compare purpose, inputs/results and identity before converting; reuse compatible code and preserve distinct contracts.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** json_text: bounded string. Return the actual syntax-validity object; this does not validate a component contract or establish approval.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = host.json(operation="validate", data=inputs["json_text"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-json-validate

**Usage prose draft to specialize:** “Use the host boundary to validate a JSON string. Supply the typed inputs described for `pc-exec-json-validate`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-exec-json-validate`'s block above.

### L075. pc-exec-echo

**Prose description:** Use the host boundary for builtin.echo (diagnostic passthrough).

**Implementation:** Bind the diagnostic message as data and add its explicit usage/result association. Keep echo diagnostic-only; final task replies use the packaged reply component. **Duplicate review:** newer section-2 entries 1 overlap this functionality. Compare purpose, inputs/results and identity before converting; reuse compatible code and preserve distinct contracts.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** message: string. Return actual diagnostic echo result. It does not publish the task reply.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = host.echo(message=inputs["message"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

### L076. pc-github-list-issues

**Prose description:** GET /repos/{owner}/{repo}/issues?state=open.

**Implementation:** Resolve host.http_fetch to an actually registered retained adapter rather than assuming an alias of host.http. Build the URL from typed owner/repository values with segment encoding. Validate the HTTP envelope and endpoint payload, and document pagination, authentication and failure behavior. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Split owner/repo encoding and endpoint construction from canonical HTTP GET; verify http_fetch versus http adapter identity rather than assuming aliases.

**Typed inputs/result:** relative_path: output of proposed-api-relative-path with segments repos/<encoded owner>/<encoded repo>/issues and reviewed state=open,sort=created,direction=desc,per_page=30 query pairs. Output full URL string.

**Recipe wiring:** `pc-url-encode` → `proposed-api-relative-path` → `pc-exec-http-get`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = "https://api.github.com" + inputs["relative_path"]
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-github-list-issues

**Usage prose draft to specialize:** “GET /repos/{owner}/{repo}/issues?state=open. Supply the typed inputs described for `pc-github-list-issues`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** the local pure/preparation snippet is not this Skill's executor. Associate this Skill only with the relevant single-Tool retrieval/write/removal usage in the Recipe above; pure portions stay separately referenced PythonCode. A pure-only historical wrapper becomes PythonCode documentation, not a zero-Tool Skill. For multi-Tool historical prose, retain task behavior in the Recipe and author separate usage Skills.

### L077. pc-github-list-prs

**Prose description:** GET /repos/{owner}/{repo}/pulls?state=open.

**Implementation:** Resolve host.http_fetch to an actually registered retained adapter rather than assuming an alias of host.http. Build the URL from typed owner/repository values with segment encoding. Validate the HTTP envelope and endpoint payload, and document pagination, authentication and failure behavior. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Split owner/repo encoding and endpoint construction from canonical HTTP GET; verify http_fetch versus http adapter identity rather than assuming aliases.

**Typed inputs/result:** relative_path: output of proposed-api-relative-path with segments repos/<encoded owner>/<encoded repo>/pulls and reviewed state=open,sort=created,direction=desc,per_page=30 query pairs. Output full URL string.

**Recipe wiring:** `pc-url-encode` → `proposed-api-relative-path` → `pc-exec-http-get`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = "https://api.github.com" + inputs["relative_path"]
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-github-list-prs

**Usage prose draft to specialize:** “GET /repos/{owner}/{repo}/pulls?state=open. Supply the typed inputs described for `pc-github-list-prs`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** the local pure/preparation snippet is not this Skill's executor. Associate this Skill only with the relevant single-Tool retrieval/write/removal usage in the Recipe above; pure portions stay separately referenced PythonCode. A pure-only historical wrapper becomes PythonCode documentation, not a zero-Tool Skill. For multi-Tool historical prose, retain task behavior in the Recipe and author separate usage Skills.

### L078. pc-github-get-authenticated-user

**Prose description:** GET /user — returns login, id, name, email.

**Implementation:** Resolve host.http_fetch to an actually registered retained adapter rather than assuming an alias of host.http. Retain the fixed /user endpoint and protected authentication. Validate the HTTP envelope and endpoint payload, and document pagination, authentication and failure behavior. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** No URL inputs; authentication through approved adapter/secret contract. Return actual HTTP envelope.

**Concrete caveat:** Reuse canonical HTTP GET with fixed URL only after its credential/header/auth and result semantics match the old http_fetch usage; not an unconditional alias substitution.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = host.http(method="GET", url="https://api.github.com/user")
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-github-get-authenticated-user

**Usage prose draft to specialize:** “GET /user — returns login, id, name, email. Supply the typed inputs described for `pc-github-get-authenticated-user`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-github-get-authenticated-user`'s block above.

### L079. pc-github-search-issues

**Prose description:** GET /search/issues?q={slot0}. slot0 = URL-encoded query.

**Implementation:** Resolve host.http_fetch to an actually registered retained adapter rather than assuming an alias of host.http. Bind and encode the query as data, separating query text from pagination parameters. Validate the HTTP envelope and endpoint payload, and document pagination, authentication and failure behavior. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Split shared value encoding and endpoint assembly from canonical HTTP GET.

**Typed inputs/result:** relative_path: output of proposed-api-relative-path using segments search/issues and query q=<encoded original query>, per_page=<reviewed count>. Output URL string.

**Concrete caveat:** Reject pre-encoded query fragments smuggling &per_page; encode the original query exactly once.

**Recipe wiring:** `pc-url-encode` → `proposed-api-relative-path` → `pc-exec-http-get`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = "https://api.github.com" + inputs["relative_path"]
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-github-search-issues

**Usage prose draft to specialize:** “GET /search/issues?q={slot0}. slot0 = URL-encoded query. Supply the typed inputs described for `pc-github-search-issues`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** the local pure/preparation snippet is not this Skill's executor. Associate this Skill only with the relevant single-Tool retrieval/write/removal usage in the Recipe above; pure portions stay separately referenced PythonCode. A pure-only historical wrapper becomes PythonCode documentation, not a zero-Tool Skill. For multi-Tool historical prose, retain task behavior in the Recipe and author separate usage Skills.

### L080. pc-git-diff-unstaged

**Prose description:** git diff (unstaged changes).

**Implementation:** Retain the exact fixed diff command and its real exit/output contract. Remove the old Tier-0 claim and attach the approved shell usage: all consuming shell Recipes are Tier 1.

**Split/reuse:** Reuse one shell dispatcher with this reviewed fixed command as typed Recipe data; retain distinct usage/effect contracts.

**Typed inputs/result:** No dynamic command arguments. Recipe constant command='git diff'; return actual shell output/exit_code/success/sandboxed object.

**Recipe wiring:** `proposed-shell-dispatch`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.shell(command='git diff')
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

### L081. pc-git-diff-staged

**Prose description:** git diff --cached (staged changes).

**Implementation:** Retain the exact fixed diff command and its real exit/output contract. Remove the old Tier-0 claim and attach the approved shell usage: all consuming shell Recipes are Tier 1.

**Split/reuse:** Reuse one shell dispatcher with this reviewed fixed command as typed Recipe data; retain distinct usage/effect contracts.

**Typed inputs/result:** No dynamic command arguments. Recipe constant command='git diff --cached'; return actual shell output/exit_code/success/sandboxed object.

**Recipe wiring:** `proposed-shell-dispatch`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.shell(command='git diff --cached')
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

### L082. pc-git-diff-head

**Prose description:** git diff HEAD~1 (last commit).

**Implementation:** Retain the exact fixed diff command and its real exit/output contract. HEAD~1 needs a defined first-commit/missing-parent failure. Remove the old Tier-0 claim and attach the approved shell usage: all consuming shell Recipes are Tier 1.

**Split/reuse:** Reuse one shell dispatcher with this reviewed fixed command as typed Recipe data; retain distinct usage/effect contracts.

**Typed inputs/result:** No dynamic command arguments. Recipe constant command='git diff HEAD~1'; return actual shell output/exit_code/success/sandboxed object.

**Recipe wiring:** `proposed-shell-dispatch`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.shell(command='git diff HEAD~1')
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

### L083. pc-grep-fn-tests

**Prose description:** grep pattern=slot0 in *test* files — find tests for a function.

**Implementation:** Bind the function pattern as data and replace the stale include keyword with the actual supported glob filter. Define regex versus literal-name matching and validate real grep results; finding names is not proof that behavior is tested.

**Split/reuse:** Reuse canonical grep execution with reviewed pattern/glob constants or prepared regex; keep the distinct heuristic usage and result contract, not another executor.

**Typed inputs/result:** pattern/path: strings; glob fixed *test*. Verify actual adapter glob matching scope. Output actual grep result.

**Concrete caveat:** A function name is not automatically a literal regex: escape metacharacters with the real Tool regex contract or declare this as a regex usage. Replace unsupported include keyword.

**Recipe wiring:** `pc-exec-grep`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.grep(pattern=inputs["pattern"], path=inputs["path"], glob="*test*")
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

### L084. pc-grep-hardcoded-secrets

**Prose description:** grep for hardcoded secret patterns (password/api_key/token).

**Implementation:** Keep the fixed pattern as reviewed source and verify its semantics in the actual grep engine. Declare actual match/truncation fields and sensitive-output handling; do not equate pattern matches or no matches with a complete security verdict.

**Split/reuse:** Reuse canonical grep execution with reviewed pattern/glob constants or prepared regex; keep the distinct heuristic usage and result contract, not another executor.

**Typed inputs/result:** path: typed root string; reviewed fixed heuristic pattern; output actual grep matches/truncation with sensitive-output handling.

**Concrete caveat:** Verify regex with the Rust grep engine. No matches does not establish security; avoid leaking matched secrets into reply/log history.

**Recipe wiring:** `pc-exec-grep`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.grep(pattern=r"(?i)(password|api_key|secret|token|credential)\s*[:=]\s*['\"][^'\"]{8,}", path=inputs["path"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

### L085. pc-grep-injection-patterns

**Prose description:** grep for injection-risk patterns (eval/exec/subprocess/sql format).

**Implementation:** Verify the literal regex and escaped parentheses against the Tool engine, distinct from Monty re. Add typed root/result contracts and document heuristic coverage; source spelling alone does not prove correct detection.

**Split/reuse:** Reuse canonical grep execution with reviewed pattern/glob constants or prepared regex; keep the distinct heuristic usage and result contract, not another executor.

**Typed inputs/result:** path: typed root string; reviewed fixed heuristic pattern; output actual grep matches/truncation.

**Concrete caveat:** Verify regex escapes in Tool engine; this is candidate discovery, not a security verdict.

**Recipe wiring:** `pc-exec-grep`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.grep(pattern=r"(?i)(eval\(|exec\(|subprocess|shell_exec|format!.*sql|query.*format)", path=inputs["path"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

### L086. pc-plan-create

**Prose description:** write plan document to plans/{slug}.md via memory_write.

**Implementation:** Build a validated memory-relative plan target from a typed slug, then pass the prepared content to memory_write using its real target/append contract. Remove the unsupported write path keyword and source substitution; plan composition remains an explicit Tier-1 step.

**Split/reuse:** Split reusable slug-to-target preparation from pc-exec-memory-write. Creative plan content/marker decisions stay in explicit Tier-1 Recipe work.

**Typed inputs/result:** slug: string, 1..64 restricted lowercase ASCII slug; output: memory-relative string. No absolute path, slash or traversal.

**Recipe wiring:** `proposed-plan-target` → `pc-exec-memory-write`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
import re
slug = inputs["slug"]
if re.fullmatch(r"[a-z0-9][a-z0-9_-]{0,63}", slug) is None:
    raise ValueError("invalid plan slug")
result = "plans/" + slug + ".md"
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

### L087. pc-plan-read

**Prose description:** read plan document from plans/{slug}.md via memory_read.

**Implementation:** Construct plans/<slug>.md from a typed validated slug and pass it to the actual read path argument. Distinguish missing, empty and malformed plan content, and reuse the generic read usage where compatible.

**Split/reuse:** Split reusable slug-to-target preparation from pc-exec-memory-read. Creative plan content/marker decisions stay in explicit Tier-1 Recipe work.

**Typed inputs/result:** slug: string, 1..64 restricted lowercase ASCII slug; output: memory-relative string. No absolute path, slash or traversal.

**Recipe wiring:** `proposed-plan-target` → `pc-exec-memory-read`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
import re
slug = inputs["slug"]
if re.fullmatch(r"[a-z0-9][a-z0-9_-]{0,63}", slug) is None:
    raise ValueError("invalid plan slug")
result = "plans/" + slug + ".md"
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

### L088. pc-plan-search

**Prose description:** memory_search query='plan_id:' — find all plan documents.

**Implementation:** Retain the fixed plan_id: search query but add the exact search association/result contract. Treat pagination and matching metadata explicitly; no matches is not a failed lookup.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** No dynamic inputs. Return the actual search result, including empty-match and truncation behavior.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
result = host.memory_search(query="plan_id:")
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

### L089. pc-plan-status-update

**Counterfactual overlap review:** slug preparation alone does not implement status updates. Complete the exact marker selection described under C02, then reuse C02 and L030; do not preserve another patch dispatcher or claim arbitrary source replacement is a status transition.

**Prose description:** patch a step marker in a plan doc via memory_write patch mode.

**Implementation:** Bind slug and exact old/new markers as data and use memory_write target, not path. Validate which marker is changed and the patch acknowledgement; do not replay the completed update or silently rewrite a different plan.

**Split/reuse:** Split reusable slug-to-target preparation from pc-exec-memory-patch. Creative plan content/marker decisions stay in explicit Tier-1 Recipe work.

**Typed inputs/result:** source_text:string from a complete memory-read, step_number:positive integer, from_status:string, to_status:string. Result exact object {old_string:string,new_string:string,replace_all:boolean fixed false}. Slug-to-target is the separate proposed-plan-target dependency, not this result.

**Canonical marker profile:** physical lines are LF or qualified CRLF; Unicode separators inside a description never create another Markdown row. The patch target omits the line terminator so the original CRLF remains intact. The document contains exactly one `## Steps` section. Its nonblank lines are `- [ ] N. description`, `- [-] N. description` or `- [x] N. description`, with unique positive decimal N. Another level-two heading ends the section. Preserve descriptions and all surrounding text. This is an explicit successor grammar, not a claim that every old document already follows it; migrate incompatible documents through the separate reviewed revise workflow. Allowed progress transitions are pending -> in_progress -> done; resetting failed/completed steps belongs to plan-revise. The body rejects malformed/duplicate rows before selecting a target.

**Recipe wiring:** proposed-plan-target -> separately bound L031 complete memory read -> this pure marker selection -> C02 precondition -> separately bound L030 patch. Retain actual source/target identity and freshness requirements. Do not seed the former slug-only monolith alongside this split Recipe.

```python
import re
markers = {"pending": " ", "in_progress": "-", "done": "x"}
old_status = inputs["from_status"]
new_status = inputs["to_status"]
if [old_status, new_status] not in [["pending", "in_progress"], ["in_progress", "done"]]:
    raise ValueError("unsupported_plan_transition")
number = inputs["step_number"]
if isinstance(number, bool) or not isinstance(number, int) or number < 1:
    raise ValueError("invalid_step_number")
seen = []
section_count = 0
in_steps = False
target = None
for physical_line in inputs["source_text"].split("\n"):
    line = physical_line[:-1] if physical_line.endswith("\r") else physical_line
    if line == "## Steps":
        section_count += 1
        in_steps = True
    elif line.startswith("## "):
        in_steps = False
    elif in_steps and line.strip():
        match = re.fullmatch(r"- \[([ x-])\] ([1-9][0-9]*)\. (.+)", line)
        if match is None or not match.group(2) or not match.group(3).strip():
            raise ValueError("invalid_plan_step_row")
        step_number = int(match.group(2))
        if step_number in seen:
            raise ValueError("duplicate_plan_step")
        seen.append(step_number)
        if step_number == number:
            if match.group(1) != markers[old_status]:
                raise ValueError("plan_status_precondition_failed")
            target = line
if section_count != 1 or target is None:
    raise ValueError("missing_or_ambiguous_plan_steps")
result = {"old_string": target,
          "new_string": "- [" + markers[new_status] + "]" + target[5:],
          "replace_all": False}
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

### L090. pc-hash-changed

**Prose description:** compare two SHA-256 hex strings and return whether they differ.

**Implementation:** Bind two validated SHA-256 strings or an explicitly allowed missing prior hash. Preserve exact comparison and declare the boolean result; comparison does not establish provenance or authorize activation. Reclassify the historical pure-logic Skill wrapper as documentation for this PythonCode; zero-Tool logic is not a one-Tool-usage Skill. **Duplicate review:** newer section-2 entries 19 overlap this functionality. Compare purpose, inputs/results and identity before converting; reuse compatible code and preserve distinct contracts.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** new_hash: lowercase 64-hex string; prior_hash: same or explicitly permitted null for no prior artifact. Output changed: boolean. This compares supplied hashes; trusted hashing/provenance is separate.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
import re
new = inputs["new_hash"]
prior = inputs["prior_hash"]
if re.fullmatch(r"[0-9a-f]{64}", new) is None:
    raise ValueError("invalid new SHA-256")
if prior is not None and re.fullmatch(r"[0-9a-f]{64}", prior) is None:
    raise ValueError("invalid prior SHA-256")
result = {"changed": prior != new}
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### hash-compare

**Documentation prose to retain with PythonCode:** “Use this pure transformation with the exact typed inputs/result described above. It executes no Tool, establishes no provenance/approval and needs no binding. The consuming Recipe provides trusted inputs and decides completion.”

**Association disposition:** the local pure/preparation snippet is not this Skill's executor. Associate this Skill only with the relevant single-Tool retrieval/write/removal usage in the Recipe above; pure portions stay separately referenced PythonCode. A pure-only historical wrapper becomes PythonCode documentation, not a zero-Tool Skill. For multi-Tool historical prose, retain task behavior in the Recipe and author separate usage Skills.

### L091. pc-format-component-header

**Audit correction:** The inspected legacy Rust renderer inserts a quoted name verbatim; it does not prove JSON-escape parsing compatibility. Keep exact two-space/header spelling and use the restricted profile below. A name outside it fails or uses a separately qualified encoding shared with the compiler; never silently adopt json.dumps as a legacy format upgrade.

**Prose description:** render the base-prompt component header line `## CC:UID LABEL "name"` as used by do_reassemble.

**Implementation:** Bind class, prompt UID, label and name as typed data with explicit string conversion/escaping. Validate the rendered header grammar and newline/quote handling, and align it with the existing prefix compiler format without treating a prompt UID as component identity. Reclassify the historical pure-logic Skill wrapper as documentation for this PythonCode; zero-Tool logic is not a one-Tool-usage Skill. **Duplicate review:** newer section-2 entries 18, 19 overlap this functionality. Compare purpose, inputs/results and identity before converting; reuse compatible code and preserve distinct contracts.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** class_code: supported integer class, not bool; prompt_uid: nonnegative integer within u32 transport, not bool; label: exact trusted class-label mapping; name: string in the explicit no-quote/backslash/control profile. Output header:string.

**Concrete caveat:** The restricted profile above preserves inspected legacy rendering. A wider name profile must be jointly specified and qualified with the retained native compiler before activation.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
name = inputs["name"]
label = inputs["label"]
for char in name:
    if char in ('"', '\\', '\n', '\r', '\x00') or ord(char) < 32 or ord(char) == 127:
        raise ValueError("name needs a separately qualified escape profile")
if not label:
    raise ValueError("invalid class label")
for char in label:
    if char.isspace() or char in ('"', '\\') or ord(char) < 32 or ord(char) == 127:
        raise ValueError("invalid class label")
result = {"header": "## " + str(inputs["class_code"]) + ":" + str(inputs["prompt_uid"])
          + '  ' + label + '  "' + name + '"'}
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### component-header-render

**Documentation prose to retain with PythonCode:** “Use this pure transformation with the exact typed inputs/result described above. It executes no Tool, establishes no provenance/approval and needs no binding. The consuming Recipe provides trusted inputs and decides completion.”

**Association disposition:** the local pure/preparation snippet is not this Skill's executor. Associate this Skill only with the relevant single-Tool retrieval/write/removal usage in the Recipe above; pure portions stay separately referenced PythonCode. A pure-only historical wrapper becomes PythonCode documentation, not a zero-Tool Skill. For multi-Tool historical prose, retain task behavior in the Recipe and author separate usage Skills.

### L092. pc-zencoder-validate-uuid

**Prose description:** validates slot0 as a UUID. Returns {valid, error?}. No host call.

**Implementation:** Receive the UUID as a typed string and use full-string matching in the pinned Monty re implementation. Define canonical spelling/nil rules per actual endpoint contract; keep invalid-input outcomes separate from successful API results and reuse this helper where internal composition is supported.

**Split/reuse:** Keep atomic; its necessary local checks belong inside this coherent operation.

**Typed inputs/result:** uuid: string; result: canonical lowercase UUID string. Decide nil/version restrictions against actual endpoint contract; this lexical shape check does not establish existence.

**Recipe wiring:** pure logic gets one executable step; Tool usage gets its matching binding then executable. Any prerequisite/output formatting/reply belongs in other steps.

```python
import re
value = inputs["uuid"]
if re.fullmatch(r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}", value) is None:
    raise ValueError("invalid UUID")
result = value.lower()
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

### L093. pc-zencoder-build-task-summary

**Prose description:** merges task JSON (slot0) and plan JSON (slot1) into a summary dict. No host call.

**Implementation:** Consume successful task/plan objects directly instead of serializing them back into quoted JSON slots. Validate nested step fields, optional plan absence, status spelling and counts; generator expressions currently materialize in documented Monty, so use bounded explicit data processing.

**Split/reuse:** Extract reusable bounded plan-step counts; keep the small domain summary formatter atomic.

**Typed inputs/result:** task: validated object with required status and explicitly nullable branch; steps: validated name/status list; counts: proposed-plan-step-counts result. Output exact task_status/branch/progress/plan_steps object.

**Concrete caveat:** Normalize the actual success envelope before this step. A missing plan has an explicit Recipe branch; do not default failed/malformed plan to empty steps.

**Recipe wiring:** `proposed-plan-step-counts`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
counts = inputs["counts"]
result = {"task_status": inputs["task"]["status"], "branch": inputs["task"]["branch"],
          "progress": str(counts["completed"]) + " of " + str(counts["total"]) + " steps completed",
          "plan_steps": inputs["steps"]}
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

### L094. pc-zencoder-list-projects

**Prose description:** GET /projects via host.zencoder_api.

**Implementation:** The fixed call needs no placeholder rewrite, but it still needs immutable input/result contracts and an approved zencoder_api binding/association. Verify response envelopes, pagination, authentication and failure semantics before catalogue admission. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Share endpoint construction and GET/write transport; preserve the endpoint usage contract. Do not create one new executor per endpoint.

**Typed inputs/result:** path: prepared restricted relative API path; result: retained API response envelope including pagination and classified failures.

**Concrete caveat:** Example is the shared transport body, not a complete endpoint-specific implementation. Resolve zencoder_api registration and retained authentication/result adapter before activation. Exact endpoint layout: segments=[projects]; no query. API body remains typed JSON text where the existing adapter requires it; explicit stringify precedes dispatch.

**Recipe wiring:** `proposed-api-relative-path` → `proposed-zencoder-get`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.zencoder_api(method="GET", path=inputs["path"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-zencoder-list-projects

**Usage prose draft to specialize:** “GET /projects via host.zencoder_api. Supply the typed inputs described for `pc-zencoder-list-projects`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-zencoder-list-projects`'s block above.

### L095. pc-zencoder-list-tasks

**Prose description:** GET /projects/{pid}/tasks[?status&limit] via host.zencoder_api.

**Implementation:** Bind project UUID, optional status and bounded integer limit as typed data. Reject invalid supplied filters instead of silently dropping them, build the path/query explicitly, and validate the successful list/envelope contract. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Share endpoint construction and GET/write transport; preserve the endpoint usage contract. Do not create one new executor per endpoint.

**Typed inputs/result:** path: prepared restricted relative API path; result: retained API response envelope including pagination and classified failures.

**Concrete caveat:** Example is the shared transport body, not a complete endpoint-specific implementation. Resolve zencoder_api registration and retained authentication/result adapter before activation. Exact endpoint layout: segments=[projects, project UUID, tasks]; optional status in todo/inprogress/inreview/done/cancelled and positive bounded integer limit. API body remains typed JSON text where the existing adapter requires it; explicit stringify precedes dispatch.

**Recipe wiring:** `pc-zencoder-validate-uuid` → `proposed-api-relative-path` → `proposed-zencoder-get`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.zencoder_api(method="GET", path=inputs["path"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-zencoder-list-tasks

**Usage prose draft to specialize:** “GET /projects/{pid}/tasks[?status&limit] via host.zencoder_api. Supply the typed inputs described for `pc-zencoder-list-tasks`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-zencoder-list-tasks`'s block above.

### L096. pc-zencoder-get-task

**Prose description:** GET /projects/{pid}/tasks/{tid} via host.zencoder_api.

**Implementation:** Bind project/task UUIDs as data, share the approved UUID checks and construct the fixed endpoint path in Python. Declare the actual response envelope and do not return success-shaped UUID errors. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Share endpoint construction and GET/write transport; preserve the endpoint usage contract. Do not create one new executor per endpoint.

**Typed inputs/result:** path: prepared restricted relative API path; result: retained API response envelope including pagination and classified failures.

**Concrete caveat:** Example is the shared transport body, not a complete endpoint-specific implementation. Resolve zencoder_api registration and retained authentication/result adapter before activation. Exact endpoint layout: segments=[projects, project UUID, tasks, task UUID]. API body remains typed JSON text where the existing adapter requires it; explicit stringify precedes dispatch.

**Recipe wiring:** `pc-zencoder-validate-uuid` → `proposed-api-relative-path` → `proposed-zencoder-get`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.zencoder_api(method="GET", path=inputs["path"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-zencoder-get-task

**Usage prose draft to specialize:** “GET /projects/{pid}/tasks/{tid} via host.zencoder_api. Supply the typed inputs described for `pc-zencoder-get-task`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-zencoder-get-task`'s block above.

### L097. pc-zencoder-get-plan

**Prose description:** GET /projects/{pid}/tasks/{tid}/plan via host.zencoder_api.

**Implementation:** Bind project/task UUIDs as data, share the approved UUID checks and construct the fixed endpoint path in Python. Classify plan-not-created separately from task absence and other HTTP failures; declare the actual response envelope and do not return success-shaped UUID errors. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Share endpoint construction and GET/write transport; preserve the endpoint usage contract. Do not create one new executor per endpoint.

**Typed inputs/result:** path: prepared restricted relative API path; result: retained API response envelope including pagination and classified failures.

**Concrete caveat:** Example is the shared transport body, not a complete endpoint-specific implementation. Resolve zencoder_api registration and retained authentication/result adapter before activation. Exact endpoint layout: segments=[projects, project UUID, tasks, task UUID, plan]; plan-not-created differs from task-not-found. API body remains typed JSON text where the existing adapter requires it; explicit stringify precedes dispatch.

**Recipe wiring:** `pc-zencoder-validate-uuid` → `proposed-api-relative-path` → `proposed-zencoder-get`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.zencoder_api(method="GET", path=inputs["path"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-zencoder-get-plan

**Usage prose draft to specialize:** “GET /projects/{pid}/tasks/{tid}/plan via host.zencoder_api. Supply the typed inputs described for `pc-zencoder-get-plan`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-zencoder-get-plan`'s block above.

### L098. pc-zencoder-create-task

**Prose description:** POST /projects/{pid}/tasks via host.zencoder_api.

**Implementation:** Bind a validated project UUID and prepared task body as data using the API adapter’s actual body representation. Keep LLM composition/confirmation in separate Recipe stages; declare mutation acknowledgements and never infer safe POST replay from a timeout. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Share endpoint construction and GET/write transport; preserve the endpoint usage contract. Do not create one new executor per endpoint.

**Typed inputs/result:** method: string restricted to reviewed POST/PATCH selectors by preflight/code, with an exact usage association; path: prepared relative endpoint; body: actual adapter JSON-text representation. Output: actual mutation acknowledgement.

**Concrete caveat:** Example is the shared transport body, not a complete endpoint-specific implementation. Resolve zencoder_api registration and retained authentication/result adapter before activation. Exact endpoint layout: segments=[projects, project UUID, tasks]; method=POST; separately validated complete task JSON body. API body remains typed JSON text where the existing adapter requires it; explicit stringify precedes dispatch.

**Recipe wiring:** `pc-zencoder-validate-uuid` → `proposed-api-relative-path` → `proposed-zencoder-write`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
method = inputs["method"]
if method not in ("POST", "PATCH"):
    raise ValueError("unsupported write method")
result = host.zencoder_api(method=method, path=inputs["path"], body=inputs["body"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-zencoder-create-task

**Usage prose draft to specialize:** “POST /projects/{pid}/tasks via host.zencoder_api. Supply the typed inputs described for `pc-zencoder-create-task`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-zencoder-create-task`'s block above.

### L099. pc-zencoder-patch-task

**Prose description:** PATCH /projects/{pid}/tasks/{tid} via host.zencoder_api.

**Implementation:** Bind UUIDs and the prepared patch body as data. Keep any prerequisite GET and description replacement decision in separate steps; validate the mutation result and retain unknown-completion state without blind PATCH retries. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Share endpoint construction and GET/write transport; preserve the endpoint usage contract. Do not create one new executor per endpoint.

**Typed inputs/result:** method: string restricted to reviewed POST/PATCH selectors by preflight/code, with an exact usage association; path: prepared relative endpoint; body: actual adapter JSON-text representation. Output: actual mutation acknowledgement.

**Concrete caveat:** Example is the shared transport body, not a complete endpoint-specific implementation. Resolve zencoder_api registration and retained authentication/result adapter before activation. Exact endpoint layout: segments=[projects, project UUID, tasks, task UUID]; method=PATCH; separately validated patch body. API body remains typed JSON text where the existing adapter requires it; explicit stringify precedes dispatch.

**Recipe wiring:** `pc-zencoder-validate-uuid` → `proposed-api-relative-path` → `proposed-zencoder-write`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
method = inputs["method"]
if method not in ("POST", "PATCH"):
    raise ValueError("unsupported write method")
result = host.zencoder_api(method=method, path=inputs["path"], body=inputs["body"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-zencoder-patch-task

**Usage prose draft to specialize:** “PATCH /projects/{pid}/tasks/{tid} via host.zencoder_api. Supply the typed inputs described for `pc-zencoder-patch-task`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-zencoder-patch-task`'s block above.

### L100. pc-zencoder-list-automations

**Prose description:** GET /automations[?enabled] via host.zencoder_api.

**Implementation:** Bind a nullable/optional boolean filter with exact omission semantics instead of accepting true/false/empty strings. Reject invalid values, construct the supported query representation and validate real list results. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Share endpoint construction and GET/write transport; preserve the endpoint usage contract. Do not create one new executor per endpoint.

**Typed inputs/result:** path: prepared restricted relative API path; result: retained API response envelope including pagination and classified failures.

**Concrete caveat:** Example is the shared transport body, not a complete endpoint-specific implementation. Resolve zencoder_api registration and retained authentication/result adapter before activation. Exact endpoint layout: segments=[automations]; missing enabled means omit query; supplied boolean maps exactly to true/false, null only if explicitly declared. API body remains typed JSON text where the existing adapter requires it; explicit stringify precedes dispatch.

**Recipe wiring:** `proposed-api-relative-path` → `proposed-zencoder-get`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
result = host.zencoder_api(method="GET", path=inputs["path"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-zencoder-list-automations

**Usage prose draft to specialize:** “GET /automations[?enabled] via host.zencoder_api. Supply the typed inputs described for `pc-zencoder-list-automations`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-zencoder-list-automations`'s block above.

### L101. pc-zencoder-create-automation

**Prose description:** POST /automations via host.zencoder_api.

**Implementation:** Bind prepared body data and validate the complete required shape before the POST. If the adapter requires JSON text, serialize explicitly with supported Monty json options; reject nonfinite/invalid values and keep confirmation and unknown-effect reconciliation outside this one usage. Review the companion prose against this exact usage, replace slot/range/result claims, move prerequisite or subsequent Tool calls into Recipe steps, and record its explicit reviewed association.

**Split/reuse:** Share endpoint construction and GET/write transport; preserve the endpoint usage contract. Do not create one new executor per endpoint.

**Typed inputs/result:** method: string restricted to reviewed POST/PATCH selectors by preflight/code, with an exact usage association; path: prepared relative endpoint; body: actual adapter JSON-text representation. Output: actual mutation acknowledgement.

**Concrete caveat:** Example is the shared transport body, not a complete endpoint-specific implementation. Resolve zencoder_api registration and retained authentication/result adapter before activation. Exact endpoint layout: segments=[automations]; method=POST; complete validated body, including nonblank name. API body remains typed JSON text where the existing adapter requires it; explicit stringify precedes dispatch.

**Recipe wiring:** `proposed-api-relative-path` → `proposed-zencoder-write`. Insert each required ToolSkill immediately before its executable. Pass successful preceding fields through typed layouts. The snippet below illustrates the relevant local/shared body; do not seed the original monolith alongside its replacement Recipe.

```python
method = inputs["method"]
if method not in ("POST", "PATCH"):
    raise ValueError("unsupported write method")
result = host.zencoder_api(method=method, path=inputs["path"], body=inputs["body"])
```

**Acceptance focus:** verify the concrete edge cases in the change above, hostile quote/newline/marker data, exact result validation and pre-effect failure. For an effectful usage, retain confirmed/unresolved completion so a later failure cannot replay it.

#### skill-zencoder-create-automation

**Usage prose draft to specialize:** “POST /automations via host.zencoder_api. Supply the typed inputs described for `pc-zencoder-create-automation`; apply defaults only to missing consumer inputs. Preconditions are a qualified retained Tool/binding and its supported parameter/result profile. Execute exactly this one Tool usage through the associated canonical PythonCode; preparation, additional Tools and final reply are Recipe steps. Return the actual validated result. Stop on classified errors or unknown completion; no retry without exact verified evidence.”

**Association disposition:** resolve the canonical executable/profile above and record this usage's exact parameters, result, failure metadata and ToolSkill/Tool association. Several compatible usages can share code with separately reviewed associations; no second body is needed. The code example is `pc-zencoder-create-automation`'s block above.

## Newer entries

### N01. Packaged reply: reply:code / reply:skill

**Prose description and implementation:** Retain exact packaged successor and association. The host resolves ownership/finalization; formatting alone is not proof. Do not replay publication after history failure.

**Typed inputs/result:** answer: nonblank string; result: actual nonempty task-owned msg reference.

**Split/reuse:** Keep atomic; no separately reusable internal part established.

```python
result = host.post_reply(answer=inputs["answer"])
```

**Recipe/qualification instructions:** For a concrete Tool usage, pair its exact ToolSkill with the executable and associate a one-Tool Skill. Pure checking/formatting has no artificial binding or Skill. For blocked host roles, first implement/qualify the supported adapter; preparation alone cannot satisfy the Recipe step. Pin all result/evidence/artifact identities in the selected combination.

**Associated Skill prose draft to specialize:** “Packaged reply: reply:code / reply:skill. Use only the typed parameters and supported primitive operation described here. Require the exact qualified binding and owned input/evidence references. Return the actual declared primitive result; classify errors/waits and preserve effect uncertainty. Do not perform other Tool operations or infer approval. The Recipe handles preparation, checks and completion.” Resolve the actual Skill identity before storing; the guide/planned roles have no allocated identity in this specification.

### N02. Packaged history writer: history:code / history:skill

**Prose description and implementation:** Keep append default and fixed target as the retained package defines them. Broader memory writes keep their own contracts.

**Typed inputs/result:** content: nonblank string; result: exact status:string/path:string/append:boolean/content_length:integer object.

**Split/reuse:** Keep atomic; no separately reusable internal part established.

```python
result = host.memory_write(content=inputs["content"], target="daily_log")
```

**Recipe/qualification instructions:** For a concrete Tool usage, pair its exact ToolSkill with the executable and associate a one-Tool Skill. Pure checking/formatting has no artificial binding or Skill. For blocked host roles, first implement/qualify the supported adapter; preparation alone cannot satisfy the Recipe step. Pin all result/evidence/artifact identities in the selected combination.

**Associated Skill prose draft to specialize:** “Packaged history writer: history:code / history:skill. Use only the typed parameters and supported primitive operation described here. Require the exact qualified binding and owned input/evidence references. Return the actual declared primitive result; classify errors/waits and preserve effect uncertainty. Do not perform other Tool operations or infer approval. The Recipe handles preparation, checks and completion.” Resolve the actual Skill identity before storing; the guide/planned roles have no allocated identity in this specification.

### N03. Packaged history formatter: history:formatter

**Prose description and implementation:** Preserve exact packaged source and task-owned finalized reply data. Do not split three label concatenations into tiny library components.

**Typed inputs/result:** user_input/answer/reply_ref: strings; result: string.

**Split/reuse:** Keep atomic; no separately reusable internal part established.

```python
result = 'User: ' + inputs['user_input'] + '\nAssistant: ' + inputs['answer'] + '\nReply: ' + inputs['reply_ref']
```

**Recipe/qualification instructions:** For a concrete Tool usage, pair its exact ToolSkill with the executable and associate a one-Tool Skill. Pure checking/formatting has no artificial binding or Skill. For blocked host roles, first implement/qualify the supported adapter; preparation alone cannot satisfy the Recipe step. Pin all result/evidence/artifact identities in the selected combination.

### N04. Protected global root: orchestrator:global

**Prose description and implementation:** The illustrative async shape is not a replacement root. Preserve the complete source in global_mode.py, UUID 52a1c2df-fb73-4832-ab11-e47b931f0724 and installation-owned root qualification. Extend explicit-wait reconciliation, retained selections, durable effect recovery and flow checks there. Never seed this abbreviated example.

**Typed inputs/result:** Root ports and task envelopes retain their existing recursive contracts; not a child inputs/result wrapper.

**Split/reuse:** Do not split the root into fake library steps. Lifecycle/ports remain root-owned; reuse genuine validator checks through supported compilation contracts only.

```python
async def _execute_recipe(task_token, recipe_id, step_link, inputs):
    # Exact opening excerpt; retain the rest of global_mode.py unchanged here.
    composed = await host.compose_orchestrator(task_token, recipe_id, step_link, inputs)
    if not isinstance(composed, dict) or composed.get("ok") is not True:
        raise RuntimeError("recipe_composition_failed")
```

**Recipe/qualification instructions:** Retain root-owned admission/routing/continuation; do not install the shortened example or apply child usage association rules.

### N05. Typed interval usage

**Prose description and implementation:** Resolve actual UUIDs and reviewed Skill/ToolSkill association. Tail may use offset/limit profile with limit0 for empty file; either qualify that profile explicitly or branch to the already typed empty result. Do not feed an end<start interval.

**Typed inputs/result:** path: nonempty string; start_line/end_line: positive bounded integers, not bool, inclusive end>=start; result: actual read_file envelope.

**Split/reuse:** Keep atomic; no separately reusable internal part established.

```python
start = inputs["start_line"]
end = inputs["end_line"]
if type(start) is not int or type(end) is not int or start < 1 or end < start:
    raise ValueError("invalid line interval")
result = host.read_file(path=inputs["path"], offset=start, limit=end-start+1)
```

**Recipe/qualification instructions:** For a concrete Tool usage, pair its exact ToolSkill with the executable and associate a one-Tool Skill. Pure checking/formatting has no artificial binding or Skill. For blocked host roles, first implement/qualify the supported adapter; preparation alone cannot satisfy the Recipe step. Pin all result/evidence/artifact identities in the selected combination.

**Associated Skill prose draft to specialize:** “Typed interval usage. Use only the typed parameters and supported primitive operation described here. Require the exact qualified binding and owned input/evidence references. Return the actual declared primitive result; classify errors/waits and preserve effect uncertainty. Do not perform other Tool operations or infer approval. The Recipe handles preparation, checks and completion.” Resolve the actual Skill identity before storing; the guide/planned roles have no allocated identity in this specification.

### N06. Typed JSON parse usage

**Audit correction:** Actual builtin.json parse returns a bare serde_json value, not a {data:...} envelope. General arbitrary JSON results cannot be described by the current homogeneous v3 schema; bind a concrete expected shape per approved usage. serde_json parsing does not itself establish duplicate-key rejection for review/association documents.

**Prose description and implementation:** Resolve real components and association. Syntax, duplicate-key and nonfinite semantics come from actual parser, not a second Python parser.

**Typed inputs/result:** json_text: bounded string; result: one explicit finite per-usage object/list/scalar schema with recursive fields/items and no default values. Wrong-shape parse output fails validation before a consumer.

**Split/reuse:** Keep atomic; no separately reusable internal part established.

```python
result = host.json(operation="parse", data=inputs["json_text"])
```

**Recipe/qualification instructions:** For a concrete Tool usage, pair its exact ToolSkill with the executable and associate a one-Tool Skill. Pure checking/formatting has no artificial binding or Skill. For blocked host roles, first implement/qualify the supported adapter; preparation alone cannot satisfy the Recipe step. Pin all result/evidence/artifact identities in the selected combination.

**Associated Skill prose draft to specialize:** “Typed JSON parse usage. Use only the typed parameters and supported primitive operation described here. Require the exact qualified binding and owned input/evidence references. Return the actual declared primitive result; classify errors/waits and preserve effect uncertainty. Do not perform other Tool operations or infer approval. The Recipe handles preparation, checks and completion.” Resolve the actual Skill identity before storing; the guide/planned roles have no allocated identity in this specification.

### N07. Shared narrow structural compatibility check

**Prose description and implementation:** Repair only the narrow legacy nonempty predicate. This compatibility check neither proves semantic agreement nor replaces final validators.

**Typed inputs/result:** subject_ref: trusted exact immutable review subject, required on every check record; evidence_refs: required trusted durable references to this subject and predicate; metadata: exact class-specific required string fields; required_fields: trusted class-Recipe constant list; evidence_refs: trusted bounded string list; output: exact subject_ref/check_id/complete/passed/diagnostics/evidence_refs check record.

**Split/reuse:** Keep atomic; no separately reusable internal part established.

```python
diagnostics = []
for field in inputs["required_fields"]:
    value = inputs["metadata"][field]
    if type(value) is not str or not value.strip():
        diagnostics.append({"path": [field], "code": "empty_metadata"})
result = {"subject_ref": inputs["subject_ref"], "check_id": "nonempty_metadata", "complete": True, "passed": not diagnostics,
          "diagnostics": diagnostics, "evidence_refs": inputs["evidence_refs"]}
```

**Recipe/qualification instructions:** For a concrete Tool usage, pair its exact ToolSkill with the executable and associate a one-Tool Skill. Pure checking/formatting has no artificial binding or Skill. For blocked host roles, first implement/qualify the supported adapter; preparation alone cannot satisfy the Recipe step. Pin all result/evidence/artifact identities in the selected combination.

**Validator scope:** select trusted required checks in a class-specific Recipe. Parser facts, candidate metadata, behavior evidence and approval records are separate inputs with exact subject/checksum identities. Fail on incomplete/untrusted facts. The example does not constitute full validator_v3 acceptance.

### N08. Identity and metadata checker

**Prose description and implementation:** Extend with class-specific required metadata and exact trusted checksum/subject identity comparisons. Candidate claims are not integrity evidence. The example shows the stated predicate only and reports complete=False; implement every listed obligation, establish complete trusted fact coverage and qualify behavior before setting complete=True.

**Typed inputs/result:** subject_ref: trusted exact immutable review subject, required on every check record; evidence_refs: required trusted durable references to this subject and predicate; candidate_uuid/subject_uuid: canonical UUID strings; revision: positive integer; trusted subject manifest pins class/name/content checksum. All inputs have strict unknown-field policy; output is the common subject_ref/check_id/complete/passed/diagnostics/evidence_refs check record.

**Split/reuse:** Keep atomic; no separately reusable internal part established.

```python
diagnostics = []
if inputs["candidate_uuid"] != inputs["subject_uuid"]:
    diagnostics.append({"path": ["uuid"], "code": "subject_mismatch"})
if type(inputs["revision"]) is not int or inputs["revision"] < 1:
    diagnostics.append({"path": ["revision"], "code": "invalid_revision"})
result = {"subject_ref": inputs["subject_ref"], "check_id": 'identity_and_metadata', "complete": False, "passed": False,
          "diagnostics": diagnostics, "evidence_refs": inputs["evidence_refs"]}
```

**Recipe/qualification instructions:** For a concrete Tool usage, pair its exact ToolSkill with the executable and associate a one-Tool Skill. Pure checking/formatting has no artificial binding or Skill. For blocked host roles, first implement/qualify the supported adapter; preparation alone cannot satisfy the Recipe step. Pin all result/evidence/artifact identities in the selected combination.

**Validator scope:** select trusted required checks in a class-specific Recipe. Parser facts, candidate metadata, behavior evidence and approval records are separate inputs with exact subject/checksum identities. Fail on incomplete/untrusted facts. The example does not constitute full validator_v3 acceptance.

### N09. Recursive contracts checker

**Prose description and implementation:** Implement recursive schema traversal/fact production in the supported parser owner; enforce items/fields/extra-values/null/missing/defaults/bounds and compatibility. The small policy example consumes complete facts, not a candidate-supplied pass flag. The example shows the stated predicate only and reports complete=False; implement every listed obligation, establish complete trusted fact coverage and qualify behavior before setting complete=True.

**Typed inputs/result:** subject_ref: trusted exact immutable review subject, required on every check record; evidence_refs: required trusted durable references to this subject and predicate; schema_facts: trusted complete recursively typed schema-analysis facts, including violations path/code; enforce finite bounds and nesting. All inputs have strict unknown-field policy; output is the common subject_ref/check_id/complete/passed/diagnostics/evidence_refs check record.

**Split/reuse:** Keep atomic; no separately reusable internal part established.

```python
diagnostics = []
for finding in inputs["schema_facts"]["violations"]:
    diagnostics.append({"path": finding["path"], "code": finding["code"]})
result = {"subject_ref": inputs["subject_ref"], "check_id": 'recursive_contracts', "complete": False, "passed": False,
          "diagnostics": diagnostics, "evidence_refs": inputs["evidence_refs"]}
```

**Recipe/qualification instructions:** For a concrete Tool usage, pair its exact ToolSkill with the executable and associate a one-Tool Skill. Pure checking/formatting has no artificial binding or Skill. For blocked host roles, first implement/qualify the supported adapter; preparation alone cannot satisfy the Recipe step. Pin all result/evidence/artifact identities in the selected combination.

**Validator scope:** select trusted required checks in a class-specific Recipe. Parser facts, candidate metadata, behavior evidence and approval records are separate inputs with exact subject/checksum identities. Fail on incomplete/untrusted facts. The example does not constitute full validator_v3 acceptance.

### N10. Dependency graph completeness checker

**Prose description and implementation:** Verify recursive closure, cycles, exact immutable dependencies, association approval and actual Tool artifacts. This set comparison assumes trusted complete graph extraction; incomplete graphs fail before this check. The example shows the stated predicate only and reports complete=False; implement every listed obligation, establish complete trusted fact coverage and qualify behavior before setting complete=True.

**Typed inputs/result:** subject_ref: trusted exact immutable review subject, required on every check record; evidence_refs: required trusted durable references to this subject and predicate; expected_refs/retained_refs: trusted bounded exact revision/checksum reference strings from complete traversed graphs; not just UUID names. All inputs have strict unknown-field policy; output is the common subject_ref/check_id/complete/passed/diagnostics/evidence_refs check record.

**Split/reuse:** Keep atomic; no separately reusable internal part established.

```python
diagnostics = []
if not inputs["expected_refs"] or len(set(inputs["expected_refs"])) != len(inputs["expected_refs"]) or len(set(inputs["retained_refs"])) != len(inputs["retained_refs"]):
    raise ValueError("empty or duplicate graph facts")
expected = set(inputs["expected_refs"])
retained = set(inputs["retained_refs"])
for reference in sorted(expected - retained):
    diagnostics.append({"path": ["dependencies", reference], "code": "missing_dependency"})
result = {"subject_ref": inputs["subject_ref"], "check_id": 'dependency_graph_completeness', "complete": False, "passed": False,
          "diagnostics": diagnostics, "evidence_refs": inputs["evidence_refs"]}
```

**Recipe/qualification instructions:** For a concrete Tool usage, pair its exact ToolSkill with the executable and associate a one-Tool Skill. Pure checking/formatting has no artificial binding or Skill. For blocked host roles, first implement/qualify the supported adapter; preparation alone cannot satisfy the Recipe step. Pin all result/evidence/artifact identities in the selected combination.

**Validator scope:** select trusted required checks in a class-specific Recipe. Parser facts, candidate metadata, behavior evidence and approval records are separate inputs with exact subject/checksum identities. Fail on incomplete/untrusted facts. The example does not constitute full validator_v3 acceptance.

### N11. Argument compatibility checker

**Prose description and implementation:** Compute compatibility from recursive usage, binding and callable schemas; check required/default/extra/null semantics and fixed selectors. A candidate compatible=true is never accepted as fact. The example shows the stated predicate only and reports complete=False; implement every listed obligation, establish complete trusted fact coverage and qualify behavior before setting complete=True.

**Typed inputs/result:** subject_ref: trusted exact immutable review subject, required on every check record; evidence_refs: required trusted durable references to this subject and predicate; argument_facts: trusted complete list of exact name:string/compatible:boolean and retained schema/adapter evidence references. All inputs have strict unknown-field policy; output is the common subject_ref/check_id/complete/passed/diagnostics/evidence_refs check record.

**Split/reuse:** Keep atomic; no separately reusable internal part established.

```python
diagnostics = []
for argument in inputs["argument_facts"]:
    if not argument["compatible"]:
        diagnostics.append({"path": ["arguments", argument["name"]], "code": "incompatible_argument"})
result = {"subject_ref": inputs["subject_ref"], "check_id": 'argument_compatibility', "complete": False, "passed": False,
          "diagnostics": diagnostics, "evidence_refs": inputs["evidence_refs"]}
```

**Recipe/qualification instructions:** For a concrete Tool usage, pair its exact ToolSkill with the executable and associate a one-Tool Skill. Pure checking/formatting has no artificial binding or Skill. For blocked host roles, first implement/qualify the supported adapter; preparation alone cannot satisfy the Recipe step. Pin all result/evidence/artifact identities in the selected combination.

**Validator scope:** select trusted required checks in a class-specific Recipe. Parser facts, candidate metadata, behavior evidence and approval records are separate inputs with exact subject/checksum identities. Fail on incomplete/untrusted facts. The example does not constitute full validator_v3 acceptance.

### N12. Workflow structure checker

**Prose description and implementation:** Complete binding-adjacency/Tool match, independent-call separation, typed references, selected variant step_link, completion and flow validation. Inspect source facts and exact dependent-chain coverage; one include alone is insufficient. The example shows the stated predicate only and reports complete=False; implement every listed obligation, establish complete trusted fact coverage and qualify behavior before setting complete=True.

**Typed inputs/result:** subject_ref: trusted exact immutable review subject, required on every check record; evidence_refs: required trusted durable references to this subject and predicate; steps: validated actual component-step objects with stepnumber and include UUID list; trusted flow facts separately cover order, channels, bindings and limits. All inputs have strict unknown-field policy; output is the common subject_ref/check_id/complete/passed/diagnostics/evidence_refs check record.

**Split/reuse:** Keep atomic; no separately reusable internal part established.

```python
diagnostics = []
for step in inputs["steps"]:
    if len(step["include"]) != 1:
        diagnostics.append({"path": ["steps", str(step["stepnumber"])], "code": "one_component_required"})
result = {"subject_ref": inputs["subject_ref"], "check_id": 'workflow_structure', "complete": False, "passed": False,
          "diagnostics": diagnostics, "evidence_refs": inputs["evidence_refs"]}
```

**Recipe/qualification instructions:** For a concrete Tool usage, pair its exact ToolSkill with the executable and associate a one-Tool Skill. Pure checking/formatting has no artificial binding or Skill. For blocked host roles, first implement/qualify the supported adapter; preparation alone cannot satisfy the Recipe step. Pin all result/evidence/artifact identities in the selected combination.

**Validator scope:** select trusted required checks in a class-specific Recipe. Parser facts, candidate metadata, behavior evidence and approval records are separate inputs with exact subject/checksum identities. Fail on incomplete/untrusted facts. The example does not constitute full validator_v3 acceptance.

### N13. Source facts and policy checker

**Prose description and implementation:** Reuse real retained-source inspection and pinned Monty compile facts; add unsupported syntax/import, dynamic host access and recursive dependency analysis. Never execute submitted code to discover call sites. Behavior evidence is separate. The example shows the stated predicate only and reports complete=False; implement every listed obligation, establish complete trusted fact coverage and qualify behavior before setting complete=True.

**Typed inputs/result:** subject_ref: trusted exact immutable review subject, required on every check record; evidence_refs: required trusted durable references to this subject and predicate; source_facts: trusted exact-checksum parser facts with booleans and complete static host-call/import/call-layout analysis; not substring guesses. All inputs have strict unknown-field policy; output is the common subject_ref/check_id/complete/passed/diagnostics/evidence_refs check record.

**Split/reuse:** Keep atomic; no separately reusable internal part established.

```python
diagnostics = []
facts = inputs["source_facts"]
if facts["has_runtime_source_slots"]:
    diagnostics.append({"path": ["source"], "code": "source_slots_forbidden"})
if not facts["assigns_result"]:
    diagnostics.append({"path": ["source"], "code": "missing_result"})
result = {"subject_ref": inputs["subject_ref"], "check_id": 'source_facts_and_policy', "complete": False, "passed": False,
          "diagnostics": diagnostics, "evidence_refs": inputs["evidence_refs"]}
```

**Recipe/qualification instructions:** For a concrete Tool usage, pair its exact ToolSkill with the executable and associate a one-Tool Skill. Pure checking/formatting has no artificial binding or Skill. For blocked host roles, first implement/qualify the supported adapter; preparation alone cannot satisfy the Recipe step. Pin all result/evidence/artifact identities in the selected combination.

**Validator scope:** select trusted required checks in a class-specific Recipe. Parser facts, candidate metadata, behavior evidence and approval records are separate inputs with exact subject/checksum identities. Fail on incomplete/untrusted facts. The example does not constitute full validator_v3 acceptance.

### N14. Retry declaration checker

**Prose description and implementation:** Complete eligible-outcome validation, explicit read-only/durable deduplication evidence and unknown-effect rules. The checker does not dispatch retries or grant policy. The example shows the stated predicate only and reports complete=False; implement every listed obligation, establish complete trusted fact coverage and qualify behavior before setting complete=True.

**Typed inputs/result:** subject_ref: trusted exact immutable review subject, required on every check record; evidence_refs: required trusted durable references to this subject and predicate; failure: exact skills.md failure object; retry_evidence_verified: boolean derived by a trusted evidence resolver, not a candidate flag, plus retained evidence references with subject/outcomes/idempotency evidence bound to this exact usage. All inputs have strict unknown-field policy; output is the common subject_ref/check_id/complete/passed/diagnostics/evidence_refs check record.

**Split/reuse:** Keep atomic; no separately reusable internal part established.

```python
diagnostics = []
failure = inputs["failure"]
count = failure["max_attempts"]
if type(count) is not int or count < 1:
    diagnostics.append({"path": ["failure", "max_attempts"], "code": "invalid_attempt_count"})
if type(count) is int and count > 1 and not inputs["retry_evidence_verified"]:
    diagnostics.append({"path": ["failure"], "code": "unverified_retry"})
result = {"subject_ref": inputs["subject_ref"], "check_id": 'retry_declaration', "complete": False, "passed": False,
          "diagnostics": diagnostics, "evidence_refs": inputs["evidence_refs"]}
```

**Recipe/qualification instructions:** For a concrete Tool usage, pair its exact ToolSkill with the executable and associate a one-Tool Skill. Pure checking/formatting has no artificial binding or Skill. For blocked host roles, first implement/qualify the supported adapter; preparation alone cannot satisfy the Recipe step. Pin all result/evidence/artifact identities in the selected combination.

**Validator scope:** select trusted required checks in a class-specific Recipe. Parser facts, candidate metadata, behavior evidence and approval records are separate inputs with exact subject/checksum identities. Fail on incomplete/untrusted facts. The example does not constitute full validator_v3 acceptance.

### N15. Scoped verdict aggregation

**Audit correction:** A pure equality check cannot authenticate review evidence. The trusted review owner must verify every producer/check revision, predicate scope, fact completeness and evidence record against the same subject before this component runs. Candidate-authored IDs/booleans are data, not authority. Required check IDs come from the selected approved class Recipe, never the proposal.

**Prose description and implementation:** Require same-subject/checksum evidence, supported required checks and completeness. Infrastructure errors remain errors. Verdict is review evidence, not human Q2 or activation.

**Typed inputs/result:** required_check_ids: unique trusted class-specific string list; checks: bounded exact common check-record list; subject_ref: exact reviewed manifest reference. Output subject_ref/passed/diagnostics/evidence_refs.

**Split/reuse:** Keep atomic; no separately reusable internal part established.

```python
expected = inputs["required_check_ids"]
if not expected or len(set(expected)) != len(expected):
    raise ValueError("empty or duplicate required checks")
by_id = {}
for check in inputs["checks"]:
    if check["check_id"] in by_id or check["subject_ref"] != inputs["subject_ref"]:
        raise ValueError("duplicate or wrong-subject check")
    if type(check["complete"]) is not bool or type(check["passed"]) is not bool:
        raise ValueError("invalid check status")
    if not check["complete"] or not check["evidence_refs"]:
        raise ValueError("incomplete or unevidenced check")
    if check["passed"] and check["diagnostics"]:
        raise ValueError("contradictory passing check")
    by_id[check["check_id"]] = check
if set(by_id) != set(expected):
    raise ValueError("incomplete check set")
passed = True
diagnostics = []
evidence_refs = []
for check_id in expected:
    check = by_id[check_id]
    passed = passed and check["passed"]
    diagnostics.extend(check["diagnostics"])
    evidence_refs.extend(check["evidence_refs"])
result = {"subject_ref": inputs["subject_ref"], "passed": passed,
          "diagnostics": diagnostics, "evidence_refs": evidence_refs}
```

**Recipe/qualification instructions:** For a concrete Tool usage, pair its exact ToolSkill with the executable and associate a one-Tool Skill. Pure checking/formatting has no artificial binding or Skill. For blocked host roles, first implement/qualify the supported adapter; preparation alone cannot satisfy the Recipe step. Pin all result/evidence/artifact identities in the selected combination.

**Validator scope:** select trusted required checks in a class-specific Recipe. Parser facts, candidate metadata, behavior evidence and approval records are separate inputs with exact subject/checksum identities. Fail on incomplete/untrusted facts. The example does not constitute full validator_v3 acceptance.

### N16. Immutable review submission

**Prose description and implementation:** BLOCKED host executor: Resolve the supported Rust sink/store adapter and exact host callable. Submit one exact proposed combination; Q1, human Q2 and activation are subsequent Recipe stages. The code below is concrete input preparation only, not a successful submission/export/compiler/validation/publication implementation. Replace it with the verified single direct host call after registration and recursive result contracts are implemented.

**Boundary:** this is candidate admission (validator_v3 Step2), not the Q1 verdict writer (Step9), combination approval or activation. Validators operate on an already admitted immutable subject. Their complete N15 verdict goes to the existing trusted review owner, whose exact evidence-persistence usage must be resolved separately; do not redirect that verdict through N16.

**Typed inputs/result:** subject_ref/dependency_manifest_ref: trusted immutable artifact references; result is actual immutable submission receipt after supported dispatch.

**Split/reuse:** Keep one primitive invocation atomic. Preparation remains internal unless another usage needs a separately reviewed contract.

```python
result = {"subject_ref": inputs["subject_ref"], "dependency_manifest_ref": inputs["dependency_manifest_ref"]}
```

**Recipe/qualification instructions:** For a concrete Tool usage, pair its exact ToolSkill with the executable and associate a one-Tool Skill. Pure checking/formatting has no artificial binding or Skill. For blocked host roles, first implement/qualify the supported adapter; preparation alone cannot satisfy the Recipe step. Pin all result/evidence/artifact identities in the selected combination.

**Associated Skill prose draft to specialize:** “Immutable review submission. Use only the typed parameters and supported primitive operation described here. Require the exact qualified binding and owned input/evidence references. Return the actual declared primitive result; classify errors/waits and preserve effect uncertainty. Do not perform other Tool operations or infer approval. The Recipe handles preparation, checks and completion.” Resolve the actual Skill identity before storing; the guide/planned roles have no allocated identity in this specification.

### N17. Catalogue sweep and export

**Prose description and implementation:** BLOCKED host executor: Qualify existing sweep_validated_components or its retained successor with complete coverage and no arbitrary row/token caps. Separate export from compilation. The code below is concrete input preparation only, not a successful submission/export/compiler/validation/publication implementation. Replace it with the verified single direct host call after registration and recursive result contracts are implemented.

**Typed inputs/result:** snapshot_ref: trusted coherent generation reference; result after dispatch is complete immutable export manifest/artifact references.

**Split/reuse:** Keep one primitive invocation atomic. Preparation remains internal unless another usage needs a separately reviewed contract.

```python
result = {"snapshot_ref": inputs["snapshot_ref"]}
```

**Recipe/qualification instructions:** For a concrete Tool usage, pair its exact ToolSkill with the executable and associate a one-Tool Skill. Pure checking/formatting has no artificial binding or Skill. For blocked host roles, first implement/qualify the supported adapter; preparation alone cannot satisfy the Recipe step. Pin all result/evidence/artifact identities in the selected combination.

**Associated Skill prose draft to specialize:** “Catalogue sweep and export. Use only the typed parameters and supported primitive operation described here. Require the exact qualified binding and owned input/evidence references. Return the actual declared primitive result; classify errors/waits and preserve effect uncertainty. Do not perform other Tool operations or infer approval. The Recipe handles preparation, checks and completion.” Resolve the actual Skill identity before storing; the guide/planned roles have no allocated identity in this specification.

### N18. Registered compiler worker invocation

**Prose description and implementation:** BLOCKED host executor: Resolve actual registered compiler adapter. Native CPython libraries remain in native worker; Monty only invokes one declared operation. No invented host.compile API or arbitrary shell executor. The code below is concrete input preparation only, not a successful submission/export/compiler/validation/publication implementation. Replace it with the verified single direct host call after registration and recursive result contracts are implemented.

**Typed inputs/result:** export_ref/compiler_ref: trusted immutable artifact identities; result after dispatch is bounded native-worker receipt and output artifact refs.

**Split/reuse:** Keep one primitive invocation atomic. Preparation remains internal unless another usage needs a separately reviewed contract.

```python
result = {"export_ref": inputs["export_ref"], "compiler_ref": inputs["compiler_ref"]}
```

**Recipe/qualification instructions:** For a concrete Tool usage, pair its exact ToolSkill with the executable and associate a one-Tool Skill. Pure checking/formatting has no artificial binding or Skill. For blocked host roles, first implement/qualify the supported adapter; preparation alone cannot satisfy the Recipe step. Pin all result/evidence/artifact identities in the selected combination.

**Associated Skill prose draft to specialize:** “Registered compiler worker invocation. Use only the typed parameters and supported primitive operation described here. Require the exact qualified binding and owned input/evidence references. Return the actual declared primitive result; classify errors/waits and preserve effect uncertainty. Do not perform other Tool operations or infer approval. The Recipe handles preparation, checks and completion.” Resolve the actual Skill identity before storing; the guide/planned roles have no allocated identity in this specification.

### N19. Generation validation usage

**Prose description and implementation:** BLOCKED host executor: Separate primitive parser/tokenizer facts from pure coverage policy. Pin original/derived artifact hashes, complete coverage and model technical capacity; do not equate hash comparison with qualification. The code below is concrete input preparation only, not a successful submission/export/compiler/validation/publication implementation. Replace it with the verified single direct host call after registration and recursive result contracts are implemented.

**Typed inputs/result:** manifest_ref/facts_ref: trusted complete artifact and parser/tokenizer evidence; result is actual validation receipt plus diagnostics.

**Split/reuse:** Keep one primitive invocation atomic. Split primitive facts collection from reusable pure manifest coverage policy; see proposed-prefix-coverage.

```python
result = {"manifest_ref": inputs["manifest_ref"], "facts_ref": inputs["facts_ref"]}
```

**Recipe/qualification instructions:** For a concrete Tool usage, pair its exact ToolSkill with the executable and associate a one-Tool Skill. Pure checking/formatting has no artificial binding or Skill. For blocked host roles, first implement/qualify the supported adapter; preparation alone cannot satisfy the Recipe step. Pin all result/evidence/artifact identities in the selected combination.

**Associated Skill prose draft to specialize:** “Generation validation usage. Use only the typed parameters and supported primitive operation described here. Require the exact qualified binding and owned input/evidence references. Return the actual declared primitive result; classify errors/waits and preserve effect uncertainty. Do not perform other Tool operations or infer approval. The Recipe handles preparation, checks and completion.” Resolve the actual Skill identity before storing; the guide/planned roles have no allocated identity in this specification.

### N20. Prefix generation publication

**Prose description and implementation:** BLOCKED host executor: Reuse qualified store_prefix_bundle boundary; verify immutable storage, prior generations and current publication owner. Durable unknown-effect reconciliation prevents replay on reply/cache failure. The code below is concrete input preparation only, not a successful submission/export/compiler/validation/publication implementation. Replace it with the verified single direct host call after registration and recursive result contracts are implemented.

**Typed inputs/result:** generation_ref/validation_ref: exact immutable verified generation and matching receipt; result after dispatch is real publication acknowledgement.

**Split/reuse:** Keep one primitive invocation atomic. Preparation remains internal unless another usage needs a separately reviewed contract.

```python
result = {"generation_ref": inputs["generation_ref"], "validation_ref": inputs["validation_ref"]}
```

**Recipe/qualification instructions:** For a concrete Tool usage, pair its exact ToolSkill with the executable and associate a one-Tool Skill. Pure checking/formatting has no artificial binding or Skill. For blocked host roles, first implement/qualify the supported adapter; preparation alone cannot satisfy the Recipe step. Pin all result/evidence/artifact identities in the selected combination.

**Associated Skill prose draft to specialize:** “Prefix generation publication. Use only the typed parameters and supported primitive operation described here. Require the exact qualified binding and owned input/evidence references. Return the actual declared primitive result; classify errors/waits and preserve effect uncertainty. Do not perform other Tool operations or infer approval. The Recipe handles preparation, checks and completion.” Resolve the actual Skill identity before storing; the guide/planned roles have no allocated identity in this specification.

## Proposed reusable parts

The following labels identify proposed contracts only. Search canonical catalogue/seed plans for compatible code, resolve overlaps, then allocate a stable UUID only where necessary. Each reuse consumer is named; “future” is an explicit use case, not proof that its Recipe exists. Do not create runtime source imports between rows. Internal composition needs supported retained dependency assembly, or use separate Recipe steps.

### proposed-tail-window

**Prose to store:** Calculate a final bounded line window without reading a file.

**Typed inputs/result:** total_lines: nonnegative integer; count: positive bounded integer; output offset >=1, limit >=0, both integer-not-bool. Empty file has limit 0.

**Named reuse consumers:** `pc-exec-read-file-tail`, `future log preview and paged diagnostic tail`.

```python
total = inputs["total_lines"]
count = inputs["count"]
if type(total) is not int or total < 0 or type(count) is not int or count < 1:
    raise ValueError("invalid tail bounds")
result = {"offset": max(1, total - count + 1), "limit": min(count, total)}
```

**Implementation and Recipe instructions:** Store as pure class-22 code with no ToolSkill/Skill. Supply typed input fields from captured data, reviewed constants or prior successful results. Validate bounded recursive outputs before handoff; keep helper dependencies exact. Apply the common acceptance contract and named-consumer edge cases; do not activate from this example alone.

### proposed-filter-text-lines

**Prose to store:** Select or exclude text lines by a literal substring.

**Typed inputs/result:** content/substring: strings; invert: boolean; result: bounded list of strings. This is literal substring filtering over exactly the supplied text.

**Named reuse consumers:** `pc-exec-read-then-grep`, `literal line variant of pc-exec-grep-invert`, `future log excerpts`.

```python
result = []
for line in inputs["content"].split("\n"):
    matches = inputs["substring"] in line
    if matches != inputs["invert"]:
        result.append(line)
```

**Implementation and Recipe instructions:** Store as pure class-22 code with no ToolSkill/Skill. Supply typed input fields from captured data, reviewed constants or prior successful results. Validate bounded recursive outputs before handoff; keep helper dependencies exact. Apply the common acceptance contract and named-consumer edge cases; do not activate from this example alone.

### proposed-filter-records

**Audit correction:** Choose a concrete homogeneous row/value contract per usage; this is not a generic any/union schema. Field and mode are reviewed constants or guarded inputs, and are validated even for an empty list. Python True==1 must not create an equality match across types.

**Structured-directory adapter gap:** current list_dir returns display strings, not name/type records. This example requires a qualified structured result profile exposing actual identity fields; do not feed display strings to it or infer file type/name by parsing suffixes. Reuse existing primitive internals through a supported adapter upgrade where possible. Trigger records can use this helper independently once their schema is qualified.

**Prose to store:** Filter typed records by one reviewed equality or substring predicate.

**Typed inputs/result:** rows: bounded homogeneous object list; field: reviewed allowed field; mode: equals or contains; value: declared comparable type, string for contains. Require field/type compatibility in the exact usage contract. Output retains every row field.

**Named reuse consumers:** `pc-exec-list-then-grep`, `pc-exec-list-filter-by-type`, `pc-exec-trigger-resolve-and-remove`.

```python
mode = inputs["mode"]
if mode not in ("equals", "contains"):
    raise ValueError("unsupported predicate")
result = []
for row in inputs["rows"]:
    value = row[inputs["field"]]
    if mode == "equals":
        matched = type(value) is type(inputs["value"]) and value == inputs["value"]
    else:
        if type(value) is not str or type(inputs["value"]) is not str:
            raise ValueError("contains requires strings")
        matched = inputs["value"] in value
    if matched:
        result.append(row)
```

**Implementation and Recipe instructions:** Store as pure class-22 code with no ToolSkill/Skill. Supply typed input fields from captured data, reviewed constants or prior successful results. Validate bounded recursive outputs before handoff; keep helper dependencies exact. Apply the common acceptance contract and named-consumer edge cases; do not activate from this example alone.

### proposed-require-one-record

**Audit correction:** Name uniqueness is not proved by a capped list containing one row. The actual trigger list has no completeness field; never supply constanttrue merely because fewer than100 rows returned. Distinct source errors remain errors. ValueError labels require supported classification before they are exposed as not_found/ambiguous outcomes.

**Prose to store:** Require one unambiguous selected record before a dependent operation.

**Typed inputs/result:** rows: bounded homogeneous object list with a concrete per-usage schema; complete: boolean from trusted enumeration/lookup evidence for the same query/snapshot. Output: that exact record schema.

**Named reuse consumers:** `pc-exec-trigger-resolve-and-remove`, `future exact project/task lookup`.

```python
rows = inputs["rows"]
if not inputs["complete"]:
    raise ValueError("incomplete lookup")
if len(rows) == 0:
    raise ValueError("lookup_not_found")
if len(rows) != 1:
    raise ValueError("lookup_ambiguous")
result = rows[0]
```

**Implementation and Recipe instructions:** Store as pure class-22 code with no ToolSkill/Skill. Supply typed input fields from captured data, reviewed constants or prior successful results. Validate bounded recursive outputs before handoff; keep helper dependencies exact. Apply the common acceptance contract and named-consumer edge cases; do not activate from this example alone.

### proposed-trigger-remove

**Audit correction:** Use the actual trigger_id parameter. The selected ID fixes the object identity despite renames/name reuse. removed=False is an actual absent outcome, not confirmed removal. Qualify current instance-wide dispatch/policy identity through the v3 authorization cutover; the inspected scoped backend is legacy support, not a new v3 role requirement.

**Prose to store:** Remove one explicitly selected trigger through the existing primitive.

**Typed inputs/result:** trigger_id: selected canonical non-nil UUID string; result: exact removed:boolean and trigger:nullable object {trigger_id:string,name:string}.

**Named reuse consumers:** `pc-exec-trigger-resolve-and-remove`, `future direct named-trigger removal Recipe`.

```python
result = host.trigger_remove(trigger_id=inputs["trigger_id"])
```

**Implementation and Recipe instructions:** Qualify the real existing Tool callable and its exact registration/adapter/result contract; associate one-Tool prose and bind immediately before execution. Retain actual acknowledgements and live policy/effect fencing. Apply the common acceptance contract and named-consumer edge cases; do not activate from this example alone.

### proposed-plan-target

**Prose to store:** Create one validated plan-document memory target.

**Typed inputs/result:** slug: string, 1..64 restricted lowercase ASCII slug; output: memory-relative string. No absolute path, slash or traversal.

**Named reuse consumers:** `pc-plan-create`, `pc-plan-read`, `pc-plan-status-update`.

```python
import re
slug = inputs["slug"]
if re.fullmatch(r"[a-z0-9][a-z0-9_-]{0,63}", slug) is None:
    raise ValueError("invalid plan slug")
result = "plans/" + slug + ".md"
```

**Implementation and Recipe instructions:** Store as pure class-22 code with no ToolSkill/Skill. Supply typed input fields from captured data, reviewed constants or prior successful results. Validate bounded recursive outputs before handoff; keep helper dependencies exact. Apply the common acceptance contract and named-consumer edge cases; do not activate from this example alone.

### proposed-shell-quote-arguments

**Prose to store:** Preserve literal argument boundaries for approved POSIX command construction.

**Typed inputs/result:** arguments: bounded list of strings without NUL; result: equally sized string list. POSIX-shell quoting only; no command validation, option filtering or sandbox proof.

**Named reuse consumers:** `pc-exec-shell-git-add`, `pc-exec-shell-git-commit`, `pc-exec-shell-git-push`, `pc-exec-shell-git-pull`, `pc-exec-shell-wc-l`.

```python
result = []
for argument in inputs["arguments"]:
    if "\x00" in argument:
        raise ValueError("NUL in shell argument")
    result.append("'" + argument.replace("'", "'\"'\"'") + "'")
```

**Implementation and Recipe instructions:** Store as pure class-22 code with no ToolSkill/Skill. Supply typed input fields from captured data, reviewed constants or prior successful results. Validate bounded recursive outputs before handoff; keep helper dependencies exact. Apply the common acceptance contract and named-consumer edge cases; do not activate from this example alone.

### proposed-shell-dispatch

**Prose to store:** Execute one approved prepared shell command; do not plan subsequent commands.

**Typed inputs/result:** command: bounded string constructed by an approved command-specific usage; result: actual output:string/exit_code:integer/success:boolean/sandboxed:boolean object. All consuming Recipes Tier 1.

**Named reuse consumers:** `all listed shell usages`, `pc-git-diff-unstaged/staged/head`.

```python
result = host.shell(command=inputs["command"])
```

**Implementation and Recipe instructions:** Qualify the real existing Tool callable and its exact registration/adapter/result contract; associate one-Tool prose and bind immediately before execution. Retain actual acknowledgements and live policy/effect fencing. Apply the common acceptance contract and named-consumer edge cases; do not activate from this example alone.

### proposed-api-relative-path

**Audit correction:** Require uppercase two-digit percent escapes, reject raw delimiters/control characters and literal dot segments, and preserve supplied ordering. Endpoint-specific owner/repo/UUID/status contracts also validate decoded values before encoding; percent escaping does not establish endpoint authorization, prevent decoded traversal or replace kernel egress/redirect/secret checks.

**Prose to store:** Assemble one relative API path and query from separately encoded values.

**Typed inputs/result:** encoded_segments: bounded list of nonempty strings produced by approved encoder/UUID validator or reviewed constants; encoded_query: bounded list of exact key/value string objects. Preserve order. Empty segment list selects /.

**Named reuse consumers:** `GitHub list/search usages`, `Zencoder task and automation usages`.

```python
import re
encoded = r"(?:[A-Za-z0-9._~-]|%[0-9A-F]{2})*"
segments = inputs["encoded_segments"]
for segment in segments:
    if not segment or re.fullmatch(encoded, segment) is None or segment in (".", ".."):
        raise ValueError("invalid encoded path segment")
query = []
for pair in inputs["encoded_query"]:
    if not pair["key"] or re.fullmatch(encoded, pair["key"]) is None or re.fullmatch(encoded, pair["value"]) is None:
        raise ValueError("invalid encoded query pair")
    query.append(pair["key"] + "=" + pair["value"])
result = "/" + "/".join(segments)
if query:
    result += "?" + "&".join(query)
```

**Implementation and Recipe instructions:** Store as pure class-22 code with no ToolSkill/Skill. Supply typed input fields from captured data, reviewed constants or prior successful results. Validate bounded recursive outputs before handoff; keep helper dependencies exact. Apply the common acceptance contract and named-consumer edge cases; do not activate from this example alone.

### proposed-zencoder-get

**Prose to store:** Fetch one prepared Zencoder endpoint.

**Typed inputs/result:** path: prepared restricted relative API path; result: retained API response envelope including pagination and classified failures.

**Named reuse consumers:** `pc-zencoder-list-projects`, `pc-zencoder-list-tasks`, `pc-zencoder-get-task`, `pc-zencoder-get-plan`, `pc-zencoder-list-automations`.

```python
result = host.zencoder_api(method="GET", path=inputs["path"])
```

**Implementation and Recipe instructions:** Qualify the real existing Tool callable and its exact registration/adapter/result contract; associate one-Tool prose and bind immediately before execution. Retain actual acknowledgements and live policy/effect fencing. Apply the common acceptance contract and named-consumer edge cases; do not activate from this example alone.

### proposed-zencoder-write

**Prose to store:** Perform one prepared Zencoder API mutation.

**Typed inputs/result:** method: string restricted to reviewed POST/PATCH selectors by preflight/code, with an exact usage association; path: prepared relative endpoint; body: actual adapter JSON-text representation. Output: actual mutation acknowledgement.

**Named reuse consumers:** `pc-zencoder-create-task`, `pc-zencoder-patch-task`, `pc-zencoder-create-automation`.

```python
method = inputs["method"]
if method not in ("POST", "PATCH"):
    raise ValueError("unsupported write method")
result = host.zencoder_api(method=method, path=inputs["path"], body=inputs["body"])
```

**Implementation and Recipe instructions:** Qualify the real existing Tool callable and its exact registration/adapter/result contract; associate one-Tool prose and bind immediately before execution. Retain actual acknowledgements and live policy/effect fencing. Apply the common acceptance contract and named-consumer edge cases; do not activate from this example alone.

### proposed-plan-step-counts

**Prose to store:** Count completion in a validated task-plan step list.

**Typed inputs/result:** steps: bounded list of exact name/status objects with required fields; status: validated API enum. Output completed/total integer counts.

**Named reuse consumers:** `pc-zencoder-build-task-summary`, `future multi-task progress dashboard`.

```python
completed = 0
for step in inputs["steps"]:
    if step["status"] == "Completed":
        completed += 1
result = {"completed": completed, "total": len(inputs["steps"])}
```

**Implementation and Recipe instructions:** Store as pure class-22 code with no ToolSkill/Skill. Supply typed input fields from captured data, reviewed constants or prior successful results. Validate bounded recursive outputs before handoff; keep helper dependencies exact. Apply the common acceptance contract and named-consumer edge cases; do not activate from this example alone.

### proposed-prefix-coverage

**Prose to store:** Check exact source-reference coverage without compiling or publishing a prefix.

**Typed inputs/result:** subject_ref: trusted immutable snapshot/build subject; facts_complete:boolean from verified export/build facts; expected_source_refs/covered_source_refs: homogeneous exact revision/checksum reference lists from that subject. Output subject_ref/passed/missing/unexpected.

**Named reuse consumers:** `newer-19 generation validation`, `future full-library export integrity review`.

```python
expected_refs = inputs["expected_source_refs"]
covered_refs = inputs["covered_source_refs"]
if not expected_refs or not inputs["facts_complete"]:
    raise ValueError("incomplete coverage subject")
expected = set(expected_refs)
covered = set(covered_refs)
if len(expected) != len(expected_refs) or len(covered) != len(covered_refs):
    raise ValueError("duplicate source reference")
result = {"subject_ref": inputs["subject_ref"], "passed": expected == covered,
          "missing": sorted(expected - covered), "unexpected": sorted(covered - expected)}
```

**Implementation and Recipe instructions:** Store as pure class-22 code with no ToolSkill/Skill. Supply typed input fields from captured data, reviewed constants or prior successful results. Validate bounded recursive outputs before handoff; keep helper dependencies exact. Apply the common acceptance contract and named-consumer edge cases; do not activate from this example alone.

### proposed-line-reply-formatter

**Audit correction:** Do not report no matches when a nonempty selection contains blank/whitespace lines. Preserve substantive matched text exactly; the explicit whitespace message is a display policy, not the result list.

**Prose to store:** Render a bounded line-selection report for reply publication.

**Typed inputs/result:** lines: bounded list of strings; result: nonblank string. If joined content is whitespace-only, use an explicit nonblank empty-display message before publication.

**Named reuse consumers:** `read-and-select literal lines Recipe`, `tail preview`, `future bounded diagnostic excerpts`.

```python
text = "\n".join(inputs["lines"])
if not inputs["lines"]:
    result = "No matching lines."
elif not text.strip():
    result = "Matching lines contain only whitespace."
else:
    result = text
```

**Implementation and Recipe instructions:** Store as pure class-22 code with no ToolSkill/Skill. Supply typed input fields from captured data, reviewed constants or prior successful results. Validate bounded recursive outputs before handoff; keep helper dependencies exact. Apply the common acceptance contract and named-consumer edge cases; do not activate from this example alone.

## Inspected implementation references and verification boundary

The retained layout grammar is implemented in [retained_inputs.rs](../../crates/brassclaw_engine/src/memory/retained_inputs.rs) and used by [global_bootstrap_components.rs](../../crates/brassclaw_skills/src/global_bootstrap_components.rs). File read/list/patch results come from [file.rs](../../crates/brassclaw_first_party_extensions/src/coding/file.rs); directory display strings are not structured records. Allowed grep modes are content/files_with_matches/count in [grep_tool.rs](../../crates/brassclaw_first_party_extensions/src/coding/grep_tool.rs); filter argument is glob, not include. Memory target/append/patch behavior comes from [memory.rs](../../crates/brassclaw_host_runtime/src/first_party_tools/memory.rs). The full protected root is [global_mode.py](../../crates/brassclaw_engine/orchestrator/global_mode.py).

Monty review follows [language/parser support](https://pydantic.dev/docs/monty/limitations/language/), [host-function execution](https://pydantic.dev/docs/monty/concepts/host-functions/) and [host-boundary values](https://pydantic.dev/docs/monty/limitations/host-values/), then the exact pinned interpreter/BrassClaw adapter. The documentation task checks inventory coverage, links and Python example syntax; it does not establish Monty execution, production effects, full validator behavior or catalogue activation. Partial validator examples deliberately report incomplete, and blocked host usages expose preparation only.


## Recipe-derived dependencies

These R entries supplement the earlier L/N/proposed implementation sections after tracing the actual Recipes. Apply the common exact authoring/approval/schema/retention contract above. Examples are draft code, not acceptance or insertion payloads. Where a prerequisite is blocked, only the pure preparation shown exists as a design; the separately specified primitive usage still must be implemented or reused and qualified before the Recipe is complete. Prefer direct supported input layout over needless identity preparers. No newly guessed host callable is authorized by these instructions.

### R01. proposed-http-body-text

**Prose description:** Extract a complete inline text response for JSON parsing or provider-specific search extraction.

**Implementation:** Validate actual body_text, status and truncation fields before returning text; reject saved/binary/missing/partial bodies. Reuse L024 for endpoint-specific status policy and N06 for parsing.

**Typed inputs/result:** response is the actual qualified HTTP object: required status:integer, headers:list of name/value:string header objects (optional truncated:boolean), request_bytes/response_bytes:nonnegative integers and redaction_applied:boolean; optional body_text:string, body_base64:string, saved_body:{path:string,bytes_written:nonnegative integer}, body_truncated/headers_truncated:boolean, body_bytes_returned:nonnegative integer, body_truncation_hint:string and truncation:{body:boolean,headers:boolean, optional bytes_returned:nonnegative integer,reason:string,next_step:string}. Declare nested required fields and unknown-field rules using the supported schema. No nullable substitute for absent optional fields. Result:string, including valid empty text. This strict successful-2xx text profile deliberately rejects binary/saved results; header truncation is a separate endpoint-specific concern.

```python
response = inputs["response"]
if response["status"] < 200 or response["status"] >= 300:
    raise ValueError("http_status_not_success")
if response.get("body_truncated", False) or response.get("truncation", {}).get("body", False):
    raise ValueError("http_body_incomplete")
if "saved_body" in response or "body_base64" in response or "body_text" not in response:
    raise ValueError("inline_text_body_required")
result = response["body_text"]
```

**Recipe wiring/reuse:** L016/L017/etc -> L024 where an exact status rule is needed -> R01 -> separately bound N06 or a reviewed text extractor. Consumers: http-get-json, web-search and GitHub JSON responses. No Skill/ToolSkill for this pure extraction. Bind the entire validated envelope, not just body_text, so truncation cannot disappear. Test empty text, missing/binary/saved body, both truncation markers, non-2xx and malformed metadata. Sanitized or redacted content is not necessarily byte-identical original JSON: endpoint profiles must reject redaction when it invalidates required original-data semantics. No silent second GET after an effectful request.


### R02. proposed-model-request-data

**Prose description:** Prepare typed task/request/source data for an explicit model usage.

**Implementation:** Separate deterministic packet preparation from model execution. Reuse the existing model primitive/entry point after retained Tier1 qualification; never turn skipped text into an implicit model call.

**Typed inputs/result:** request:string and source_text:string, both required with checks, no defaults. Result exact object {request:string,source_text:string}, both fields required, no extras. The approved consuming usage fixes purpose, trusted instruction envelope and result profile; this packet contains untrusted data and is not an authority-bearing system prompt or a public model API.

```python
result = {"request": inputs["request"], "source_text": inputs["source_text"]}
```

**Recipe wiring/reuse:** retrieval/typed user inputs -> R02 -> exact model ToolSkill -> requalified existing model PythonCode -> R03 or the actual qualified result-profile checker. Consumers: file-write/patch, HTTP mutation drafting, web-search summaries, shell command authoring, code/QA/security review, plan creation/revision, document compression, Zencoder task/automation proposals and prefix redesign. Different usages have separately approved output/meaning contracts. The current host.kohai_complete expects prompt, and its port reads chat_history/user_query/prefix_placeholder; this R02 request/source_text object is not that prompt schema. Author the exact finite consuming prompt profile and a supported typed mapping/preparer; do not forward this object unchanged or invent acceptance of its fields. The legacy placeholder is a marker rather than the retained full prefix: qualification must replace latest/minimal-prefix fallback with the task-pinned complete prefix and accurate history/provider selection. If an actual mapping cannot construct the nested profile, add a reusable reviewed prompt preparer for that demonstrated contract rather than letting runtime values become source. Do not add another generic Kohai executor if the existing pc-host-kohai-complete UUID can supply it. Its current state[previous_result] implementation and root stream_model ports are not accepted retained child APIs; actual adapter/task/prefix/association/failure support must be implemented and qualified first. Do not expose root-only model ports to arbitrary children. If packet forwarding is supported directly by the input layout, omit this identity step instead of adding needless code. Keep it atomic; no separate formatting helper until a consumer demonstrates reuse. Test quote/newline/marker data survives as data and no source substitution occurs.

### R03. proposed-model-answer-text

**Prose description:** Validate and extract answer text from the qualified existing Kohai completion envelope.

**Implementation:** Stop on an error or invalid answer. Keep structured draft parsing and domain validation as later separate steps; answer text is neither executable code nor an effect confirmation.

**Typed inputs/result:** response: the concrete current Kohai success/error envelope, with required ok:boolean; optional answer:string, error:string and usage:{input_tokens:nonnegative integer,output_tokens:nonnegative integer,cost_usd:nonnegative number}, nested fields declared and extra fields disallowed. This is one finite schema with conditional presence guards in code, not an invented union. Result:string. Producer schema plus trusted bridge checks occur before this component. A bridge with a different envelope needs its actual qualified profile; no alias-by-name conversion.

```python
response = inputs["response"]
if response["ok"] is not True:
    raise ValueError("model_call_failed")
if "answer" not in response or not isinstance(response["answer"], str) or "error" in response:
    raise ValueError("model_answer_invalid")
result = response["answer"]
```

**Recipe wiring/reuse:** qualified existing model usage -> R03 -> formatter, separately bound N06 for structured JSON, or exact domain draft validator -> effects. Consumers share R02's explicit model usages. If the requalified executable already returns this validated string, reuse that result directly and omit R03 rather than executing duplicate validation. No Skill/ToolSkill for the pure checker. Test okFalse, missing/invalid answer, contradictory error field, valid empty answer and structured text that remains data. Keep atomic. Never substitute this envelope for the protected root's task-bound stream outcome contract.

### R04. proposed-trigger-create

**Prose description:** Create one scheduled trigger from an already prepared name, prompt, cron and policy.

**Implementation:** Add the missing executable for trigger-create, using the actual primitive parameters and a strict selector guard. Listing/preparation is separate; uncertain persistence or rollback never permits automatic repeat.

**Typed inputs/result:** name/prompt/cron/completion_policy:required strings with checks. The code permits only recurring or complete_after_first_fire. Validate exact cron/timezone behavior in the actual scheduler profile; do not infer it from prose. Result is the actual {trigger:...} object from trigger_management::trigger_output: declare trigger_id,name,prompt,source,state,completion_policy,created_at and nullable agent_id/project_id/next_run_at/last_run_at/last_status plus the actual recursive schedule and is_active:boolean. Resolve serialized enum/schedule shapes from their concrete types and observations before qualification; no unconstrained object or success default.

```python
policy = inputs["completion_policy"]
if policy not in ["recurring", "complete_after_first_fire"]:
    raise ValueError("unsupported_completion_policy")
result = host.trigger_create(name=inputs["name"], prompt=inputs["prompt"], cron=inputs["cron"], completion_policy=policy)
```

**Skill prose draft:** “Create exactly one scheduled trigger with the supplied validated cron, name, prompt and explicit completion policy. The primitive checks schedule validity and returns its persisted trigger receipt. Listing, model preparation and subsequent task execution are independent Recipe steps. A create-hook/rollback error can leave an unresolved effect; stop and reconcile by the retained attempt rather than creating another trigger.” Associate exact code/ToolSkill/Tool/parameters/result/failure and evidence; current Tool policy is independent. Consumers: trigger-create and reusable scheduled workflow admission. Default stop/max_attempts1; no verified deduplication was established. No invocation approval lease. Test selector rejection before dispatch, cron failure, confirmed ID, hook failure and rollback uncertainty. Keep one primitive call atomic.

### R05. proposed-component-content-hash

**Prose description:** Compute a deterministic SHA256 hash of supplied complete document text.

**Implementation:** Add the missing one-operation component_db compute_hash usage. Hashing must consume faithful complete text, not numbered/capped read display or a model summary.

**Typed inputs/result:** text:required string; result exact object {hash:required string}, no extras. Verify lowercase64hex in the actual producer/consumer profile with supported checks or code; preserve UTF8/newline semantics. A complete read/raw document export is a prerequisite for doc-sync, not supplied by this hash helper.

```python
result = host.component_db(op="compute_hash", text=inputs["text"])
```

**Skill prose draft:** “Compute SHA256 over the UTF8 bytes of the supplied text using component_db's compute_hash primitive and return its lowercase hash. This does not read, write or validate a component. The Recipe obtains the exact complete source and compares stored state separately.” Pin actual registered component_db Tool/callable/adapter and its exact fixed op evidence; the Rust handler exists but this new retained usage still needs binding/association/behavior qualification. Consumers: doc-sync replacement, source/derived document preparation and document deduplication. Do not use text hashing as a substitute for the different exact retained-package/artifact checksum procedure. Test empty/Unicode/newline known vectors and complete versus truncated source. Keep atomic.

### R06. proposed-document-hash-request

**Prose description:** Prepare the typed document-name lookup needed for stored-hash comparison.

**Implementation:** Add the missing reusable preparation; separately qualify the existing component_db read_hash usage under final-v3 host-derived routing. The current legacy scope argument cannot be invented away or exposed as operator roles.

**Typed inputs/result:** name:required nonempty string with checks; result exact object {op:string,name:string}, required fields/no extras. Fixed op=read_hash belongs to this preparer's behavior. This is internal request data, not an executable host binding or a successful lookup.

```python
result = {"op": "read_hash", "name": inputs["name"]}
```

**Required execution step (blocked adapter profile):** existing component_db currently requires scope:{user_id,project_id} and returns {hash:nullable string}; missing backend fails. Final-v3 routing/instance migration must be defined by the host using admitted identity, without adding feature roles or source-supplied authority. Then implement one fixed-read_hash class22 call through its actual supported callable, ToolSkill and associated one-Tool Skill. Do not seed the dictionary above as the lookup executor or invent a scope-free host API. Recipe: R05 new hash -> actual read_hash binding/executable -> L090 comparison -> supported conditional conversion. Consumers: doc-sync and incremental document/prefix refresh. If a supported binding maps name directly, omit the preparer and create only the required one-operation usage. Skill prose must explain nullable absent stored hash versus errors, exact lookup identity and read-only evidence. Test no row/null hash, changed/unchanged complete text, backend absence and task identity. Keep atomic; no permanent separate payload component if input mapping suffices.

### R07. proposed-pending-document-fields

**Prose description:** Prepare exact typed source or derived documentation fields for pending storage.

**Implementation:** Separate document rendering from storage and approval. Reuse the existing pending storage primitive only if its mutable documentation contract fits; it must never write or activate immutable approved executable components.

**Typed inputs/result:** name/description/content/content_hash:required strings, name nonempty; similarity_parent_id/replaces_id:required nullable strings; consumer_tags:required list of strings. Result exact object with those fields plus source:string fixed to operator; declare recursive schemas/no extras and validate content_hash against R05 for this content before dispatch. The example deliberately cannot claim system_seed provenance. Packaged first-party document installation has its distinct verified installation-owner path; it cannot be obtained by accepting a source input.

```python
result = {"name": inputs["name"], "description": inputs["description"],
          "content": inputs["content"], "content_hash": inputs["content_hash"],
          "source": "operator", "similarity_parent_id": inputs["similarity_parent_id"],
          "replaces_id": inputs["replaces_id"], "consumer_tags": inputs["consumer_tags"]}
```

**Required execution step (blocked final-v3 storage profile):** component_db currently supports upsert with fields and legacy scope and always sets the documentation row pending, returning id/content_hash/is_new. Its backend updates mutable reborn_docus rows; that is not retained class22/Skill publication. Resolve actual source-label/storage semantics and host-derived routing, immutable source/derived retention where consumers need it, distinct exact review evidence and durable deduplication/effect reconciliation before qualifying a fixed one-operation pending-document-store Skill/code/ToolSkill. If the v3 submission sink N16 already supports this document artifact profile, reuse N16 and do not create a duplicate writer. Each source and derived write is a separate step/receipt; link derived to confirmed source ID. Example prepares fields only and must never count as a persisted result. Consumers: doc-convert both variants, doc-sync replacement and quarantined documentation ingestion. Reuse L091 rendering separately. Test forged seed labels are impossible, content/hash mismatch, null parent versus missing, write uncertainty and concurrent updates. Keep field preparation atomic; rendering/hash remain reusable separate components.

### R08. proposed-prefix-invalidation-request

**Prose description:** Prepare the explicit invalidation request after a confirmed document change.

**Implementation:** Make the hidden doc-sync mark-stale action a separate usage. Reuse current publication/generation invalidation when it covers the same effect; never pretend an acknowledged best-effort mark guarantees fresh content.

**Typed inputs/result:** no user fields; result exact object {op:string} with required op and fixed mark_stale. This is request preparation only.

```python
result = {"op": "mark_stale"}
```

**Required execution step (blocked adapter/evidence profile):** component_db mark_stale currently requires legacy scope and returns {ok:true}; the backend contract is best-effort and may log failures. Inspect the actual implementation/observation contract before relying on success. Qualify a host-derived instance routing profile and one-operation class22/binding/Skill only if N20 generation publication/invalidation does not already cover it. No duplicate component or redundant invalidation when the canonical publication transaction provides the effect. Return confirmed stale-generation evidence or explicit incomplete/unresolved state; do not invent a currently unsupported receipt field/API. Consumers: doc-sync replacement, incremental documentation/prefix refresh. A prepared op object is not invalidation. Omit this preparer if supported fixed binding/code already supplies op. Test stored-write failure skips invalidation, stale marking failure is visible, and repeated confirmed writes do not force unsafe effect replay. Keep atomic.

### R09. proposed-child-task-request

**Prose description:** Prepare reusable typed goal/context for a separately admitted child task.

**Implementation:** Add the missing preparation and explicit child usage requirement behind subagent Recipes. Verify actual scheduler/admission/wait support; the existing builtin.spawn_subagent authorized:true result is not evidence that a child ran.

**Typed inputs/result:** goal:string required/nonempty, context:string required; result exact object {goal:string,context:string}, both required/no extras. This is internal data, not a scheduler API or an authorized/completed run.

```python
result = {"goal": inputs["goal"], "context": inputs["context"]}
```

**Required execution step (blocked child adapter profile):** reuse existing actual supported child admission/dispatch primitive and scheduler when they implement the final-v3 global service contract. Determine real callable, parameters, parent/run/attempt identity, inherited retained snapshot, result/wait handles, cancellation/budget and effect semantics before writing the one-call executor/associated Skill/ToolSkill. Do not invent host.spawn_child, treat authorization as completion, or start a second Monty/per-chat VM. The actual primitive body in first_party_tools/spawn_subagent.rs returns only authorized:true; registration plus that result is insufficient. Explicit wait/result handling belongs to Recipe/global continuation support; runner errors never become empty successful child results. Consumers: all five seeded subagent Recipes, research/review child jobs and future reusable task decomposition. Always Tier1. If direct typed mapping suffices, omit the preparer and keep only the qualified admission usage. Test identical goal/context transport, old-snapshot retention across child/wait, stale attempt/cancel fencing and missing/unknown child outcome. Keep atomic; no domain-specific child Rust workflow Tool.

### Coverage acceptance for these dependencies

Use [the Recipe ledger](monty.composition.recipes.md) as the component/variant closure checklist. For each Tool usage produce the specialized Skill prose, exact skill-association/1 arguments (including fixed op/selectors), recursive inputs/result/failure, trusted approval of exact revisions and actual retained Tool implementation. The pure dictionary examples are internal data contracts, never inserted as executor success or passed as new unsupported API fields. Data-source checks, conditions and foreach/child waits must use supported retained flow/input contracts, not prose or automatic source concatenation.

For doc-sync/doc-convert, faithful complete raw source acquisition and immutable source/derived linkage are independent prerequisites: current numbered read text and generic mutable documentation upsert do not establish them. Storage and invalidation effects each have their own retained receipt. For model usages and child execution, requalify existing supported primitives rather than reviving root control ports or treating authorized:true as completed work. Test every selected Recipe variant and hostile typed data through exact pinned Monty and actual production adapters during implementation; CPython checks below only validate this document's examples.

### R10. proposed-complete-document-text

**Prose description:** Validate and extract a complete faithful UTF8 document snapshot before hashing, extraction, storage or full-source prefix teaching.

**Implementation:** Add the missing raw-source check shared by doc-convert/doc-sync and complete-source export. A qualified raw acquisition/export operation supplies an internally defined typed snapshot with immutable source_ref:string, content:string, complete:boolean and representation:string. Every field is required, no extras; representation must equal raw_utf8_preserved by guard. This internal result profile is a target adapter contract, not a currently supported read_file field/API. The trusted host verifies actual acquisition, limits, encoding/newlines and source identity before exposing it; user/model flags cannot establish completeness or provenance.

```python
snapshot = inputs["snapshot"]
if snapshot["complete"] is not True or snapshot["representation"] != "raw_utf8_preserved":
    raise ValueError("complete_raw_document_required")
if not snapshot["source_ref"]:
    raise ValueError("document_source_identity_required")
result = snapshot["content"]
```

**Recipe wiring/reuse:** actual supported raw snapshot/export binding and executor (blocked until its registered profile is identified or supported) -> R10 -> R05 and L033/L091/R07 in separate reusable steps. Ordinary L001 read_file uses decoded lines, removes newline details and adds numbered display prefixes; stripping them cannot recover original source bytes. Reuse an existing complete-source export primitive if its exact semantics agree; otherwise extend the existing filesystem primitive's technical raw/snapshot profile instead of adding a Rust doc-conversion workflow. Do not invent host.read_file(raw=True) or concatenate display chunks and declare them byte-faithful. UTF8-only documents preserve the exact text hashed by R05; other encodings need an explicit different canonicalization contract. No silent CRLF/trailing-newline normalization. Test absent/malformed identity, incomplete text, numbered-display representation, empty valid document, Unicode, CRLF and terminal newline, size rejection and changing source. Preserve the snapshot ref alongside text through all consumers. This is a pure checker with no Skill/ToolSkill; the separate acquisition is a one-Tool Skill usage. Keep atomic; consumers share this check but do not gain authority from its booleans.

### Existing model usage requalification — pc-host-kohai-complete

**Purpose/disposition:** Preserve the existing canonical identity for review as an explicit Tier1 model usage if its actual primitive can satisfy the retained child task contract. It remains retired/superseded for protected root routing and No-Match behavior. This requirement does not create a duplicate model component or restore old per-chat execution.

**Example successor source (blocked until qualification):**

```python
result = host.kohai_complete(prompt=inputs["prompt"])
```

**Typed contract and implementation gate:** Current bridge consumes prompt.user_query:string, chat_history:list of role/content:string objects and prefix_placeholder:string, with permissive empty/default parsing and per-scope minimal-prefix fallback. Those behaviors are not the final-v3 child model contract. Before this body can be retained as a one-Tool usage, implement/qualify strict admitted-task prompt mapping, complete approved pinned prefix/source identity, model/provider/capability mapping, no silent history omission/fallback, global policy/technical checks, task compute account, failure/wait/recovery and exact association/approval. Inputs/output must match the actual supported revised bridge; use R03 only if the revised envelope is its documented ok/answer/usage shape. R02 is internal packet preparation, not a prompt object this current bridge accepts. A separate qualified mapping step or direct input layout must convert the reviewed request/source data to actual user_query/history/prefix references without source interpolation or elevating untrusted content. Do not manufacture a guessed new host.model callable or expose root stream_model authority. The same stopped/incomplete behavior applies to provider-assisted validator/prefix workflows. Test missing/failed model, context/identity mismatch, unsupported prefix/capacity, typed hostile input and incomplete result before any subsequent effect. Semantic review/structured parsing remains explicit later Recipe code; arbitrary model Python is never executed.


## MCP consumers of these components

The accepted [Skill-facing MCP target](monty.composition.md#16-mcp-exposes-skills-and-submits-python-to-the-global-orchestrator) consumes the same reusable Python/Tool usage library through the existing global orchestrator's isolated child execution. It does not require a second executor or duplicate class22 primitives. Keep raw Tool result validation before semantic output formatting; reuse proposed-line-reply-formatter only for its actual list-of-lines contract and other exact formatters where appropriate. Rust encodes the formatted result in the MCP response, rather than selecting the workflow/rewriting the result. N01/N02 ordinary chat publication/history are not automatically MCP completion steps. R09's general child-request/admission prerequisite can share actual supported global child machinery, but its dictionary example is not an MCP server, admitted program or child executor. External submitted Python needs the separate implemented transient-source admission contract in section16, not an invented Skill UUID -> Recipe compose call or automatic publication of the received source. No additional Python component is needed solely to serialize an MCP protocol envelope.


## Counterfactual Recipe dependencies C01–C05

These additions assume correct composition, json-parse and both doc-convert variants. They are pure class22 roles, never executors, host bindings or new Skill types. Apply the appendix's common finite recursive schema, immutable revision, dependency and qualification contracts. Exact object fields below are required and extras disallowed; list item objects have the stated finite shape. Selector constraints not expressible by supported schema nodes are guarded in code. Every body still requires compilation and behavioral qualification with the exact retained Monty; CPython probes below are development evidence only.

### C01. proposed-api-absolute-url

**Prose description:** Combine an approved API origin with an encoded relative request path.

**Implementation:** Extract the reusable origin/path assembly missing from web-search and prose-only GitHub usages. Reuse existing encoders and path preparation; do not add another HTTP executor.

```python
origin = inputs["origin"]
relative = inputs["relative_path"]
if not origin.startswith("https://"):
    raise ValueError("https_origin_required")
authority = origin[8:]
if not authority or any(c in authority for c in "/?#@\\"):
    raise ValueError("invalid_origin")
parts = authority.split(":")
if len(parts) > 2:
    raise ValueError("unsupported_origin_profile")
name = parts[0]
for label in name.split("."):
    if not label or len(label) > 63 or label[0] == "-" or label[-1] == "-":
        raise ValueError("invalid_origin_label")
    if any(c not in "abcdefghijklmnopqrstuvwxyz0123456789-" for c in label):
        raise ValueError("invalid_origin_character")
if len(name) > 253:
    raise ValueError("invalid_origin_length")
if len(parts) == 2:
    port = parts[1]
    if not port or any(c not in "0123456789" for c in port) or not 1 <= int(port) <= 65535:
        raise ValueError("invalid_origin_port")
if not relative.startswith("/") or relative.startswith("//"):
    raise ValueError("relative_api_path_required")
if any(ord(c) <= 32 or ord(c) >= 127 or c in "#\\" for c in relative):
    raise ValueError("invalid_relative_api_path")
result = origin + relative
```

**Typed inputs/result:** origin:string and relative_path:string; result:string. This profile supports lowercase ASCII DNS-name HTTPS origins with an optional numeric port, not credentials, a base path, IPv6 or arbitrary URL normalization. Origin is selected from the admitted approved configuration; syntactic acceptance is not permission or proof that an origin is configured. relative_path comes from the exact reviewed proposed-api-relative-path dependency, which validates encoded segments/query. Declare that internal dependency; the extra checks below are not a replacement for its complete encoding contract.

**Recipe wiring:** L027 encodes each dynamic path/query value -> proposed-api-relative-path -> C01 -> matching HTTP binding -> L016/L017 as applicable -> L024/R01/N06. Web-search's configured origin/path/query profile must actually exist; otherwise stop with setup-required. GitHub usage paths/body/auth profiles remain separately reviewed. Consumers: web-search, code-review-pr, github-create-pr/add-comment, and future REST usages. Reuse existing embedded assembly in L076–L079 if it already supplies the exact target; do not force redundant preparation steps.

**Acceptance:** reject userinfo, mixed-case/Unicode authority, controls, fragments, missing/out-of-range port and network-path reference. Test encoded Unicode/query round trips and no double encoding. DNS/network/redirect policy and secrets remain host enforcement; joining two strings grants none of them.

### C02. proposed-patch-precondition

**Prose description:** Check that a proposed replacement targets the intended text without ambiguity.

**Implementation:** Add the missing reusable pre-effect check for file patches, memory patches and plan marker updates. Reuse existing patch executables; require freshness enforcement separately.

```python
source = inputs["source_text"]
old = inputs["old_string"]
new = inputs["new_string"]
replace_all = inputs["replace_all"]
if not old or not isinstance(replace_all, bool):
    raise ValueError("invalid_patch_request")
count = source.count(old)
if count == 0 or (not replace_all and count != 1):
    raise ValueError("patch_target_missing_or_ambiguous")
if old == new:
    raise ValueError("patch_has_no_change")
result = {"old_string": old, "new_string": new,
          "replace_all": replace_all, "occurrences": count}
```

**Typed inputs/result:** source_text:string, old_string:string, new_string:string, replace_all:boolean. Result exact object {old_string:string,new_string:string,replace_all:boolean,occurrences:positive integer}. Nonempty old, unique non-all match and actual change are behavior guards. Count follows the primitive's nonoverlapping literal replacement semantics; no regex or fuzzy matching.

**Recipe wiring:** qualified complete raw source acquisition -> R10 where its snapshot profile applies -> selected typed patch draft -> C02 -> L006 file patch or L030 memory patch. Plan progress uses proposed-plan-target -> L031 complete memory read -> L089 exact marker selection -> C02 -> L030. The local source receipt and actual target identity remain separately pinned through the Recipe; do not accept model-provided source text as the authoritative precondition.

**L089 completion requirement:** replace its current illustrative slug-only body with the split plan-target flow. Define one canonical plan document/marker grammar in the plan usage profile. Select the entire uniquely identified step line as old_string; preserve all text outside its status marker and build new_string with only the allowed pending/in-progress/done transition. Reject absent/duplicate step identifiers, unsupported marker states and malformed document layout before C02. A typed layout may map already validated exact old/new lines directly; arbitrary model-selected substrings are insufficient. Do not retain a second plan patch dispatcher.

**Acceptance:** no match, two matches in single mode, all-mode counts, empty old text, unchanged replacement, Unicode/CRLF and hostile strings. This check is an observation, not an atomic compare-and-set: the actual Tool must verify freshness/preconditions at dispatch if the Recipe promises them. Missing supported atomic profile blocks that promise; no guessed expected_hash parameter. Default stop after uncertain effect, never reread/reapply automatically.

### C03. proposed-finding-text

**Prose description:** Render one structured review finding with severity, explanation and proposed fix.

**Implementation:** Supply explicit reusable formatting where a Recipe selects structured findings. A model returning an already complete answer can bypass this helper; do not call formatting a missing model executor.

```python
finding = inputs["finding"]
severity = finding["severity"]
if severity not in ["Critical", "High", "Medium", "Low", "Info"]:
    raise ValueError("invalid_finding_severity")
title = finding["title"]
if not title.strip() or "\n" in title or "\r" in title:
    raise ValueError("invalid_finding_title")
if not finding["detail"].strip() or not finding["recommendation"].strip():
    raise ValueError("incomplete_finding")
result = "[" + severity + "] " + title + "\n\n" + finding["detail"] + "\n\n" + finding["recommendation"]
```

**Typed inputs/result:** finding:exact object {severity:string,title:string,detail:string,recommendation:string}; result:string. Code guards the finite severity vocabulary and nonempty text. Upstream recursive validation occurs before the body.

**Recipe wiring:** reviewed model usage -> R03 -> N06 with its matching binding if the response is JSON -> recursive finding/target validation -> retained foreach C03 -> proposed-line-reply-formatter or separately approved comment POST. Consumers: code-review-local/pr, QA/security reviews and future structured MCP results. If the usage already returns the complete specified prose answer, reuse that answer and omit C03; it is not proof that such a review lacked executable code. Retained iteration is an execution prerequisite, not implemented by a list in prose.

**Acceptance:** empty/invalid severity, multiline title, missing fix, hostile Markdown and multiple findings. Body text remains untrusted display data; safe rendering/redaction belongs to its actual consumer. Severity formatting does not prove accuracy or authorize a comment. Keep atomic; do not create separate severity/title helpers without demonstrated reuse.

### C04. proposed-text-diff-anchors

**Prose description:** Derive eligible file line and side targets from a complete ordinary unified text diff.

**Implementation:** Add conservative deterministic hunk accounting for verified line comments. Unsupported or invalid diff forms stop. A general-comment branch needs a separately qualified known-unsupported outcome; C04 does not supply that classifier.

```python
import re
base_sha = inputs["base_sha"]
head_sha = inputs["head_sha"]
for sha in [base_sha, head_sha]:
    if len(sha) not in [40, 64] or any(c not in "0123456789abcdef" for c in sha):
        raise ValueError("invalid_diff_subject")
if not inputs["source_ref"]:
    raise ValueError("missing_diff_source")
anchors = []
seen_paths = []
state = "start"
path = ""
header = ""
old_left = 0
new_left = 0
old_line = 0
new_line = 0
old_end = 0
new_end = 0
file_hunks = 0
marker_allowed = False
lines = inputs["diff_text"].split("\n")
if lines[-1] == "":
    lines = lines[:-1]
for line in lines:
    if line == "\\ No newline at end of file":
        if state != "body" or not marker_allowed:
            raise ValueError("invalid_no_newline_marker")
        marker_allowed = False
        continue
    marker_allowed = False
    if line.startswith("diff --git "):
        if old_left != 0 or new_left != 0 or (state != "start" and file_hunks == 0):
            raise ValueError("incomplete_diff_file")
        header = line
        state = "metadata"
        file_hunks = 0
        old_end = 0
        new_end = 0
    elif state == "metadata" and line.startswith("index "):
        if re.fullmatch(r"index [0-9a-f]+\.\.[0-9a-f]+(?: [0-7]{6})?", line) is None:
            raise ValueError("invalid_index_header")
        state = "old_header"
    elif state in ["metadata", "old_header"] and line.startswith("--- a/"):
        path = line[6:]
        if not path or any(ord(c) < 32 or ord(c) in [34, 92, 127] for c in path):
            raise ValueError("unsupported_diff_path")
        if any(part in ["", ".", ".."] for part in path.split("/")) or path in seen_paths:
            raise ValueError("invalid_or_duplicate_diff_path")
        seen_paths.append(path)
        state = "new_header"
    elif state == "new_header" and line == "+++ b/" + path:
        if header != "diff --git a/" + path + " b/" + path:
            raise ValueError("diff_header_mismatch")
        state = "hunk_header"
    elif state in ["hunk_header", "body"] and line.startswith("@@ "):
        if old_left != 0 or new_left != 0:
            raise ValueError("incomplete_diff_hunk")
        pieces = line[3:].split(" @@", 1)
        if len(pieces) != 2 or (pieces[1] and not pieces[1].startswith(" ")):
            raise ValueError("invalid_hunk_header")
        ranges = pieces[0].split(" ")
        if len(ranges) != 2 or not ranges[0].startswith("-") or not ranges[1].startswith("+"):
            raise ValueError("invalid_hunk_ranges")
        parsed = []
        for item in ranges:
            fields = item[1:].split(",")
            if len(fields) not in [1, 2] or any(not f or any(c not in "0123456789" for c in f) for f in fields):
                raise ValueError("invalid_hunk_number")
            start = int(fields[0])
            count = int(fields[1]) if len(fields) == 2 else 1
            if count > 0 and start < 1:
                raise ValueError("invalid_hunk_start")
            parsed.append([start, count])
        # A zero-count range is a gap after start; normalize its next coordinate.
        old_start = parsed[0][0] if parsed[0][1] else parsed[0][0] + 1
        new_start = parsed[1][0] if parsed[1][1] else parsed[1][0] + 1
        if file_hunks and (old_start < old_end or new_start < new_end):
            raise ValueError("overlapping_or_reordered_hunk")
        old_line, old_left = old_start, parsed[0][1]
        new_line, new_left = new_start, parsed[1][1]
        if old_left == 0 and new_left == 0:
            raise ValueError("empty_hunk")
        old_end = old_start + old_left
        new_end = new_start + new_left
        file_hunks += 1
        state = "body"
    elif state == "body" and line.startswith(" ") and old_left > 0 and new_left > 0:
        anchors.append({"path": path, "line": new_line, "side": "RIGHT"})
        old_line += 1
        new_line += 1
        old_left -= 1
        new_left -= 1
        marker_allowed = True
    elif state == "body" and line.startswith("-") and old_left > 0:
        anchors.append({"path": path, "line": old_line, "side": "LEFT"})
        old_line += 1
        old_left -= 1
        marker_allowed = True
    elif state == "body" and line.startswith("+") and new_left > 0:
        anchors.append({"path": path, "line": new_line, "side": "RIGHT"})
        new_line += 1
        new_left -= 1
        marker_allowed = True
    else:
        raise ValueError("unsupported_or_invalid_diff")
if old_left != 0 or new_left != 0 or (state != "start" and file_hunks == 0):
    raise ValueError("incomplete_diff")
result = {"source_ref": inputs["source_ref"], "base_sha": base_sha, "head_sha": head_sha, "anchors": anchors}
```

**Typed inputs/result:** source_ref:string, base_sha:string, head_sha:string, diff_text:string. Result exact object {source_ref:string,base_sha:string,head_sha:string,anchors:list of {path:string,line:positive integer,side:string}}. Guard SHA as lowercase 40/64 hex and side values by producer behavior. The Rust admission/retained source contract must bind source_ref and base/head SHA to the actual complete reviewed response; these strings are not self-attesting. Acquire through the exact diff GET profile and R01, with complete response/resource constraints and pinned base/head observations.

**Supported parsing profile:** ordinary same-path modified text files with diff --git/index/--- a/+++ b/@@ headers, optional function text after the closing @@, context/add/delete lines and the conventional no-final-newline marker. Empty diff yields empty anchors. Count each declared hunk exactly. Only RIGHT context/addition and LEFT deletion anchors are eligible, matching GitHub semantics. Renames, added/deleted whole files, binary/combined diffs, quoted/tab/control/backslash paths, mode changes and other extensions fail explicitly. This example implements a deliberately limited safe profile; it is not complete Git diff coverage. Before broader support, extend the parser and requalify source identity/path/count tests rather than ignoring unfamiliar lines.

**Recipe wiring:** C01 -> bound diff GET L016 -> R01 -> C04 -> independently validated model finding targets -> C05. Consumers: code-review-pr-post-comments and future local review annotations over the same diff profile. L080–L082 supply local diff data only through their actual complete output profile. No second diff Tool, no host fact parser invented. This example deliberately stops on every unsupported or invalid form. It does not implement a general-comment fallback classifier. If such a branch is desired, separately qualify a complete trusted parser outcome that distinguishes a known unsupported placement from malformed/truncated source; otherwise stop. An arbitrary ValueError must never become a general-comment success.

**Acceptance:** multiple files/hunks, additions/deletions/context, omitted counts, zero counts, blank content lines, truncated/extra hunk rows, mismatched paths, unsupported extensions and no-newline markers. Reject duplicate paths and overlapping/reordered hunks at the producer; the consumer still requires one exact matching anchor. Split only on LF: Unicode separators in added/context content are characters on that physical line. No-newline markers require an immediately preceding body row and may appear only once per row. Test optional function text containing @@, embedded U+2028/vertical-tab content and forged repeated markers. Source provenance and completeness come from the actual acquisition/retention boundary, not a boolean authored by the model.

### C05. proposed-pr-line-comment

**Prose description:** Prepare a GitHub review comment for a verified target in the reviewed commit.

**Implementation:** Validate diff identity, head freshness observation and the chosen anchor, then prepare typed API fields. Reuse the existing POST usage under its exact comment association.

```python
facts = inputs["diff_facts"]
if facts["source_ref"] != inputs["expected_source_ref"] or facts["head_sha"] != inputs["reviewed_head_sha"] or facts["base_sha"] != inputs["reviewed_base_sha"]:
    raise ValueError("wrong_review_subject")
if inputs["current_head_sha"] != inputs["reviewed_head_sha"] or inputs["current_base_sha"] != inputs["reviewed_base_sha"]:
    raise ValueError("review_head_changed")
line = inputs["line"]
side = inputs["side"]
if isinstance(line, bool) or not isinstance(line, int) or line < 1 or side not in ["LEFT", "RIGHT"]:
    raise ValueError("invalid_comment_target")
matches = 0
for anchor in facts["anchors"]:
    if anchor["path"] == inputs["path"] and anchor["line"] == line and anchor["side"] == side:
        matches += 1
if matches != 1 or not inputs["body"].strip():
    raise ValueError("unverified_comment_target_or_body")
result = {"body": inputs["body"], "commit_id": inputs["reviewed_head_sha"],
          "path": inputs["path"], "line": line, "side": side}
```

**Typed inputs/result:** diff_facts:C04's exact result object, expected_source_ref:string, reviewed_base_sha:string, reviewed_head_sha:string, current_base_sha:string, current_head_sha:string, body:string, path:string, line:positive integer, side:string. Result exact object {body:string,commit_id:string,path:string,line:positive integer,side:string}. current_base_sha/current_head_sha are acquired independently through a separately bound metadata GET, not model output. expected_source_ref/reviewed_base_sha/reviewed_head_sha come from the task's retained acquisition subject. Validate their concrete identity shapes before this body.

**Recipe wiring:** C04 trusted anchors + model's separately validated finding -> optional C03 formatting -> bound fresh PR metadata GET/R01/N06 -> C05 -> bound L017 under the exact Skill/ToolSkill GitHub review-comment profile -> L024/R01/N06 actual receipt. JSON body uses the existing supported typed HTTP adapter or separately bound L073 serialization; never pass a dictionary to an unverified string-only body API. General comments instead map the validated text to the finite {body:string} request contract; no permanent identity-dictionary helper if input mapping already does this. Consumers: code-review-pr-post-comments and future reviewed line annotations.

**GitHub contract:** [review comment endpoint](https://docs.github.com/en/rest/pulls/comments#create-a-review-comment-for-a-pull-request) specifies commit_id/path/line/side; use LEFT for deletion lines and RIGHT for added/context lines. This profile is single-line comments, not multi-line ranges or deprecated diff position. A fresh GET is an observation and cannot prevent the PR advancing between GET and POST. Preserve the reviewed commit ID, let server validation decide the target and surface rejection/outdated state. Never claim this helper supplies an atomic head lock.

**Acceptance:** wrong source/base/head, later observed base or head, positive booleans, nonexistent/duplicate anchor, invalid side and blank body all stop before POST. Retain actual comment ID and confirmed/unresolved effect state; no repeat after formatting/reply/disconnect failure. Add/specialize the existing one-Tool comment Skill draft with these exact reviewed parameter/result expectations and association; C05 itself has no Skill.


## Additional audit-derived reusable parts C06–C07

These parts fill concrete producer/consumer gaps in the Recipe designs. They remain proposed pure PythonCode, without a fabricated ToolSkill or zero-Tool Skill. Reuse an existing compatible component identity if the exact finite profile already exists; do not duplicate L091 header formatting or L073's supported typed-object serialization.

### C06. proposed-document-render

**Purpose and implementation:** combine a separately rendered single-line component header with the full extracted/compressed body. L091 only returns a header; hashing/storing that header does not preserve the document. Use this small pure component for both doc-convert variants and future rendered component documentation.

**Typed inputs/result:** header:string, body:string; result:string. Required strings distinguish missing/null; body may be empty only where the selected document profile explicitly permits it. The raw source remains a separate immutable artifact and is never replaced by the compressed or rendered string. Apply the selected finite text-size/resource constraints at admission/result validation.

```python
header = inputs["header"]
if not header.strip() or "\n" in header or "\r" in header:
    raise ValueError("invalid_document_header")
result = header + "\n\n" + inputs["body"]
```

**Exact wiring:** L091.result.header -> C06.header; L033.found=true and its non-null section_content -> C06.body, or R03 validated compression text -> C06.body. L033.found=false is a missing-section outcome and cannot bind null as body; C06.result -> R05.text -> derived pending-document R07 content/hash. Separately R10 full raw source -> R05 -> source R07; its confirmed ID supplies derived similarity_parent_id. Every hash is computed over the exact bytes/profile of the corresponding stored content. Never use the derived hash for the raw source. Inspect R07's actual adapter; prepared fields cannot stand in for writes. Reuse both variants' canonical store usages, with two separate retained dispatch receipts. Reject incomplete source/compression and content/hash/parent mismatches before storage. Test empty permitted body, CRLF/Unicode body preservation and multi-line header rejection.

### C07. proposed-typed-json-text

**Purpose and implementation:** serialize a validated typed value for a selected JSON-as-text output profile, without reparsing string values. Public structured Tool results need an explicit formatter before N01's string input. L073's current host adapter reparses strings as JSON; reuse L073 for a compatible typed-object profile, and use this pure part only where string/scalar serialization or avoiding another Tool call supplies a distinct reusable need.

**Typed inputs/result:** data uses the consumer's exact finite recursive schema, including concrete list items/object fields and allowed extra-value shapes; result:string. No invented any/union schema, arbitrary Monty object, cycle, secret-bearing authority or nonfinite number is admitted. Different reviewed finite profiles can reuse the same body. Result validation and resource constraints still apply.

```python
import json
result = json.dumps(inputs["data"], ensure_ascii=False, allow_nan=False)
```

**Exact wiring:** validated successful usage result or explicitly selected public projection -> C07.data -> string result -> the one selected reply owner. Existing text results bypass this helper; list-of-string displays can reuse the existing line formatter. Errors/unknown effects remain typed failures, never success strings. MCP's qualified output profile may reuse it for a text content block while retaining structured/error metadata and no automatic chat posting. Do not expose hidden credentials/internal evidence by serializing an entire envelope. Test strings containing quotes/JSON-looking text, finite nested objects, Unicode, nonfinite/cyclic rejection at the typed boundary and preservation of protocol failure status.


Recipe-level implementation instructions are in the [RCP001–RCP170 appendix](monty.composition.recipe-implementation.md); reuse the canonical bodies/contracts here rather than copying them into new components.


The [audit dispositions](monty.composition.md#20-correctness-security-and-v3monty-alignment-audit) apply to these examples and all per-Recipe designs. Syntax/pure-function probes do not establish acceptance by the selected retained Monty/IBS path.
