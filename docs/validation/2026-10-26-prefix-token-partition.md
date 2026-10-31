# Physical prefix-span token measurement — 26 October 2026

Status: scoped package passed. Full Prefix-plan completion remains open.

The existing offline encoding primitive now supports `partitioned_text`: one
complete literal rendered text and an explicit UTF-8 byte range. It encodes the
whole text once with the retained tokenizer, without saved padding/truncation or
extra BOS/EOS. Actual token offsets attribute every token to the supplied range
or the remaining prompt. Any token crossing a boundary, or an unmappable offset
with a nonempty prefix, rejects the measurement. Independent substring counts,
rounding and double counting are forbidden. Empty range `0..0` explicitly means
no prefix; every actual token then belongs to the prompt.

Worker request/report version 2 is selected only for a batch containing the new
probe kind. Existing text/chat version-1 report bytes omit the optional sidecar
and remain unchanged. Results retain whole-text hashes/counts, physical range,
prefix byte hash, partition counts and little-endian token-ID hashes. The parent
checks exact requested text/range identity, qualification and checked count sums.
Common-prefix observations remain independent of physical attribution.

This extends the existing Tool/backend and its two canonical encoding usages;
there is no new store, editable setting, queue, provider catalogue or workflow
Tool. Both existing installation-owned encoding Recipes gain an independently
specified partition result, a boundary rejection and an invalid-UTF-8 rejection.
The actual contained worker must be rebuilt, not substituted by the old image.
The bootstrap owner continues to bind exact packaged/native/worker identities,
native outcomes and the durable invocation journal.

## Observed checks

- Encoder unit tests, including real BPE boundary merges, Unicode, invalid ranges,
  unmappable offsets, disabled prefix, complete long inputs and legacy shape.
- Independent retained Ornith/Hugging Face comparison for original rendering and
  the new physical partition. Archived model originals are verified by their
  provenance hashes; Transformers stays offline and trusts no remote code.
- Actual PostgreSQL runtime readiness, both native encoding Recipes and durable
  journal reconstruction, plus rejection of altered/missing qualification data.
- Ordinary global Match/No-Match regression and strict affected/default lints,
  format, whitespace and exact protected-source integrity.

## Remaining scope

`qualification=supplied-byte-span-only` is a physical observation, not proof that
the caller selected the correct semantic prefix. No provider request is made.
The qualified actual-envelope adapter, real prefix/composite selection, canonical
policy read, output/context reservation, unconditional final pre-dispatch guard,
zero-call overflow tests and live stricter-ceiling behavior remain open. This
does not qualify image expansion, cache residency/restoration, recurrent state,
model answer quality, target-aware Generate or serving/deployment. No inference
service, Compose stack, host venv, systemd unit or remote machine was changed.

## Exact evidence

Receipt: `target/accepted-evidence/prefix-token-partition-20261026/run-1/accepted.json`;
SHA-256 `ffd04d0a20e016fbb3af508225f68cd01e22098e62d61ac4753b704328c37bc7`.

All 15 executed Rust cases passed: nine encoder cases, two independent
Ornith/Hugging Face comparisons, three Composition encoding cases (including
actual PostgreSQL startup/Recipe/bootstrap/journal validation) and the ordinary
global Match/No-Match case. Both canonical encoding usages execute their
complete installation-owned case inventories; these include the three new cases
per usage. Their internal fixture executions are not counted as additional Rust
test cases. No ignored test was silently skipped: both explicit offline oracle
cases were selected with `--ignored` and each actually passed.

`strict-encoder-skills-composition`: passed in 107.23 seconds.
`strict-default-composition`: passed in 38.07 seconds.
`format`: passed in 4.73 seconds.
Whitespace and protected-source integrity passed. The newly built actual
worker and test executables, all eight changed source files, complete source
inventory, model-original hashes, commands, raw logs and validation runner are
retained in that receipt directory. Older evidence remains intact.
