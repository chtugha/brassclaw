# Verified leading prefix in rendered chats — 26 October 2026

> **Operator architecture correction, 26 October:** the Sempai global template
> override was removed. Production prefixes must already be fully formatted
> per provider/model at generation. Generator, Kohai and same-pair Sempai use
> the same stored accepted design revision. Kohai first formats only the dynamic
> Orchestrator prompt; connected/enabled Sempai may correct it using its own
> generated reviewer prefix/target-pair design and returns the final dynamic
> prompt. Kohai then adds the intended prefix unchanged exactly once before it,
> without formatting the return again,
> under the target joining/default-format contract. The
> six-context 2,179-token result below uses the historical custom
> `inject_reference` template; it is not default-template/request-owned serving
> qualification. Generic byte/span/context-check primitives retain their exact
> mechanical scope. Follow the [current binding plan](../plans/prefix_v3_upgrade.md#binding-operator-correction--26-october-2026)
> for the required migration; preserve the original receipt and observed facts.

Status: this bounded package passed. Final request guards and full Prefix-plan
completion remain open.

The existing encoding Tool and its two canonical usages now accept
`leading_prefix_chat` with exact ordered `context_json` and `leading_prefix`.
The contained worker renders the complete context using the selected retained
effective template. It requires the supplied literal prefix at byte zero, then
encodes the complete rendered chat once and partitions actual token offsets.
It never searches for a later occurrence, tokenizes substrings independently,
inserts measuring sentinels, shortens evidence or infers a boundary from an LCP.
Mismatched leading bytes, unsupported render contexts or tokens crossing the
boundary reject the measurement.

The version-2 sidecar retains exact prefix bytes/token-ID hashes and counts,
remaining prompt counts/hash, the exact ordered render-context digest and
`qualification=verified-leading-render-bytes-only`. The parent checks the requested
prefix and context identity, range, label and checked count sum. An empty expected
prefix explicitly charges all tokens to the prompt. Existing version-1 text/chat
and version-2 literal-span observations keep their old serialized shapes.

## Executed validation

The retained Ornith originals were checked against their archived provenance.
All three native/Hugging Face comparisons ran explicitly, including the existing
renderer and literal-span regressions. The new comparison uses the actual shared
`prefix_template.py` injection adapter, not a copied template implementation.
Six contexts cover missing/system-string/system-null/system-list roles, null and
empty tools, an actual tool schema, thinking settings, Unicode and a repeated
reference in dynamic history. Prefix token IDs remain identical: 2,179 tokens;
dynamic prompt counts vary, including the repeated reference. All bytes remain
measured. These are offline tokenizer/render facts, not current server/cache facts.
The 2,179 tokens belong to this diagnostic fixture: they are not a configured
prefix maximum, a 100,000-token capacity test, a cache-hit count or proof of
KV/recurrent-state reuse. These legacy custom-template cases do not prescribe
runtime whole-prefix formatting or a dynamic-tools-before-prefix serving layout.

Actual PostgreSQL startup executes both packaged encoding Recipes, including
leading match/mismatch, boundary failure and Unicode cases. Independent fixed
fixture answers and durable native invocation journals are reconstructed; altered
context labels/hashes and cross-kind provenance fail checks. Ordinary global
Match/No-Match also passes. No general pure approval exception was extended.

The actual worker was built anew from the current common tree. Fresh current test
builds execute historical Prefix regression cases for canonical export
capacity, registered-source/HTTP/classification/storage/preload, budget planning,
archived originals, graph/captured selection and retained composition. These
observations supersede the incomplete 530-file reuse justification for precisely
those executed current cases. Original receipts and their limits are preserved.
This does not qualify every historical package, independent Python/browser work,
unexecuted native consumer or remote deployment.

## Remaining work

The caller-supplied expected prefix is byte-verified, not authenticated as an
accepted serving artifact. This primitive does not resolve current canonical
provider policy or prove that this render context equals the actual transformed
provider wire request. The qualified provider-envelope adapter, accepted
prefix/composite and artifact binding, live policy/output/context ceilings and
unconditional final pre-dispatch guard remain open. Production must generate the
finished provider/model-specific prefix first; Kohai formats only the Orchestrator
prompt (including dynamic tools/history), then adds the exact unchanged prefix
before that formatted prompt. No later whole-request formatting may reorder
dynamic parts ahead of it or mutate its characters. An incompatible transport/
default-template path remains blocked pending a correct adapter; a non-leading
diagnostic span is not an alternative serving contract. The old version-1 capacity
calculator is unchanged and is not a version-2 two-cap guard. Missing policy or
identity must not silently become estimated fit or a compatibility fallback.

Remaining production integration must also resolve the same accepted, pinned
provider/model design for prefix generation and Kohai's dynamic-prompt formatting
and joining. Kohai first formats the dynamic Orchestrator prompt for the final
target. Optional connected Sempai corrects that already formatted prompt and returns
the final dynamic prompt; Kohai then inserts the intended prefix unchanged exactly
once, without formatting the return again. The final prefix is not review data.
Sempai's review request uses its selected pair's pinned design, separately generated
reviewer prefix and complete-request guard. When the pair is shared with Kohai,
the design revision is shared too; only the role content/prefix differs. Pair limits
are manually editable through the canonical provider settings owner, not copied
into role-specific designs. These are required contracts, not behavior qualified by
this encoding evidence.

The plan's §5.3 source-only caller audit records the outstanding concrete
integration: the installed catalogue currently supplies a retained task prefix,
the interceptor reviews an already assembled request, and the design catalogue
reports design generation unavailable. These are existing consumers to migrate
through their owners, not proof of the required target-design-aware serving order.

No model setting/store, queue, provider catalogue or alternate submission path
was added. No inference service, Compose/venv/systemd, model host, prefix serving
pointer or cache configuration was changed.

## Exact evidence

Receipt: `target/accepted-evidence/prefix-leading-chat-20261026/run-1/accepted.json`;
SHA-256 `025b9ab2c52ddb9cb30eb4a5386fcecd97713acf9b507783d2e51ea457610049`.

33 fresh Rust cases and seven Python checks pass. All three ignored-by-default oracle cases were explicitly selected
and passed; no zero-case result was accepted.

- `actual-encoder-unit`: 11 passed.
- `retained_ornith_matches_huggingface`: 1 passed.
- `retained_ornith_partition_matches_huggingface`: 1 passed.
- `retained_ornith_injected_leading_chat_matches_huggingface`: 1 passed.
- `native-encoding-bootstrap-and-journal`: 4 passed.
- `current-export-capacity`: 2 passed.
- `current-registered-source-candidate`: 2 passed.
- `current-http-classification`: 3 passed.
- `current-source-preload`: 1 passed.
- `current-pg-source-storage`: 1 passed.
- `current-budget-planning`: 1 passed.
- `current-original-graph-captured-selection`: 4 passed.
- `ordinary-global-match-no-match`: 1 passed.

`strict-encoder-skills-composition`: passed in 85.8 seconds.
`strict-default-composition`: passed in 18.39 seconds.
`format`: passed in 4.69 seconds.
Whitespace and protected-source checks pass. Exact current source, all eight
changed files, actual worker/test executables, commands, logs, model provenance
and runner are retained with the receipt.

`worker-source-applicability.json` binds the newly built worker to 9,714 captured
repository files as a conservative whole-repository inventory. It is explicitly
not a minimal normal/build dependency closure and does not use the historical
530-entry list as proof. All twelve accepted conditional-delete source/test/
document files remain byte-identical after this Prefix package.
