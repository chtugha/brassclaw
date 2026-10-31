# BUILD031 — existing offline Prefix validation hardened

The [machine receipt](2026-10-31-prefix-response-hardening.json) binds current
sources, both original attempts and actual interpreter/runtime identities.
SHA-256: `86c02191e6f95363ce88ec8f6d24a8423145d5260cec3312fdda0000cb808db0`.

Generated and saved JSON reject duplicate fields and non-JSON constants. Saved
answers enforce the already declared answer-schema boundaries. The standalone
compiler wrapper verifies the actual local text and configured file against the
same evidence generation before its provider callback. No new response format,
resource/token cap, provider configuration or execution authority is introduced.

All13 canonical validator cases now pass without skips, including the formerly
skipped real Home Assistant2026.2.3 schema/safe-YAML checks. The actual three domain
compiler imports/citation subcases pass in their existing compiler interpreter.
The first isolated HA interpreter lacked those separate compiler dependencies;
its failed caller run remains recorded and its exact13 successes are reused.

HA runs only static schemas: runtime/device behavior remains `not_checked`.
Quotation provenance/lexical checks do not prove semantic entailment. These
results do not establish native Recipe integration or qualified Serving.

The exact historical HA environment built lru-dict/PyRIC and emitted upstream
packaging deprecations concerning license metadata/universal wheels. Their full
warnings remain in the receipt/install log, without filtering or overrides.
They are not warnings from the changed validator and are not declared resolved.
