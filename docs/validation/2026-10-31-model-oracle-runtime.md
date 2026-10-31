# Offline model oracle runtime and existing native consumers

BUILD-017 and batched BUILD-016 passed. The [receipt](2026-10-31-model-oracle-runtime.json)
records nine reviewed source paths, exact binaries, interpreter/dependency lock,
model files, failed attempts and evidence reuse.

Four normally ignored offline Rust/Hugging Face cases were explicitly executed;
all four passed with zero ignored. The actual PostgreSQL/global native consumer
also passed, including base/Sempai measurements and rejection of oversized context.
The supported isolated Python runtime loads the original typed Qwen configuration.
Physical counting uses the tokenizer backend without padding or truncation;
original model limits, full measurement inputs and model/template bytes remain
unchanged. The observed configuration/backend/model-input wrapper warnings are
absent without filtering or suppression.

Compilation and connected strict Clippy passed in R1/R3. R4 changed only an
externally loaded Python measurement script, verified unchanged Rust and literal
embedded inputs, and reused those exact binaries and checks. R1's disk guard and
R2's private runner preflight failure remain recorded. Only required disposable
Composition package caches were cleaned after preserving the actual test image.

After the original processes exited, Root independently verified the source
inventory, archived images, worker companions, interpreter lock, model assets and
available common lock. The inventory contains 8,177 named inputs; it is not a
claim to enumerate every possible generated or dynamic input. Root retains Cargo
ownership; the source freeze for this accepted batch is released.

This acceptance covers offline rendering/tokenization and existing native
consumers. It establishes no real inference, Linux/Windows deployment, accepted
model design/prefix, new Q1/Q2 or functional approval, Free capability, or full-plan
completion. General A own/joint approvals and noncircular S qualification remain
implementation work.
