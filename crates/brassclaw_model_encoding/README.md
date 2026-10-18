# Offline model encoding

This fixed utility renders a retained model chat template and measures exact token
IDs using retained tokenizer bytes. It has no source selection, model download,
Python/remote-code execution, provider transport, prefix activation or workflow
logic. Generation, layout, probes and acceptance decisions belong in reusable
PythonCode/Recipes.

The retained `monty_worker` utility protocol exposes `EncodeModel` as one
contained operation, using its existing bounded frame, allocator ceiling,
response deadline and child termination/reaping. It never starts another global
Monty service. The encoder request format is
`brassclaw-model-encoding-worker/1`; output is `brassclaw-model-encoding/1`.
Assets are original UTF-8 tokenizer.json,
tokenizer_config.json and chat_template.jinja bytes. Text probes encode exactly
as supplied. Chat probes carry the complete render context in `context_json`, a
JSON string whose object order is preserved (including nested tool declarations).
No padding, truncation or additional special tokens are inserted. Hashes of
rendered bytes and little-endian u32 token IDs permit comparison without returning
large token arrays. Common-prefix length is measured across all supplied probes;
this does not prove server cache persistence, recurrence reuse or model quality.

The renderer preserves Python JSON spacing, Unicode and ordered keys, has no
filesystem template loader, and rejects wall-clock template functions rather
than accepting nondeterministic prefix bytes. Unsupported templates fail closed.
A compatible result for one template is not universal Hugging Face equivalence.

Technical input/render ceilings are 64 MiB, response ceiling 1 MiB, 64 probes,
context depth 64, two million context nodes, five million template fuel. These
are worker safety bounds, not prefix token budgets. Production native integration must retain the utility worker with its native
implementation selection and honor the effective preparation limits. Encoding
may need a larger utility-worker memory ceiling than parsing; insufficient
capacity fails rather than changing limits or truncating content. This crate
alone does not register or activate a native Tool.

Focused tests include tokenizer padding/truncation defaults and reference JSON
rendering. `retained_ornith_matches_huggingface` is deliberately ignored by default:
set `BRASSCLAW_ENCODING_MODEL_ORIGINALS` to retained model originals and execute it
explicitly with offline Python Transformers/tokenizers installed. It compares
complete rendered hashes, token counts/IDs and common-prefix hashes against an
independent Hugging Face renderer. Tests never download model data.

Compatibility is explicitly pinned: `vendor/minijinja-compat` preserves eager
indexable sequence concatenation, and the environment corrects the iterable test
for None. Template bytes are not rewritten to conceal a renderer mismatch.
Safe typed error categories cross the worker boundary; template source, context
values and private exception strings do not appear in those errors. The native
adapter treats these classified failures as failed operations, never successful
prefix validation or a reason to fall back to the original template.

The existing worker memory backstop measures Rust allocator-backed allocations.
Native-library allocations (notably Oniguruma) are not covered by that counter;
do not represent it as an OS-enforced process RSS ceiling. Input/output bounds
and the external deadline still apply. Production activation must include the
applicable native-memory/resource containment evidence for supported model assets.
