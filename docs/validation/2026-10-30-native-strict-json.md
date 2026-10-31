# Native strict JSON prerequisite — 30 October 2026

[The machine receipt](2026-10-30-native-strict-json.json) binds four changed
HostRuntime files, the Exact001 proposal, Root's test repair, actual images,
worker/source inventories and both attempts. Root integrated and executed the
checks through the permanent central Cargo queue.

`builtin.json` gains explicit `parse_strict` using the existing common bounded
reader with duplicate rejection at every object depth. Inputs are exactly
operation plus actual JSON text. Existing parse/query/stringify/validate/hash
operations preserve their original semantics. The exact v1 schema remains
resolvable; a disjoint v2 schema exposes the new operation. One current-schema
helper serves declaration, native registration and normalization. No parser,
Tool, executor, dependency, migration or settings registry is added.

Run2 compiles and passes11 fresh distinct cases/0ignored, rustfmt and connected/
default strict Clippy with warnings denied. The enhanced real native retention
case verifies the current implementation/schema, retains the exact original
handle across mutable registry replacement, and observes allow/block/re-enable
policy revisions. It accepts nested distinct objects/Unicode, rejects nested and
Unicode-escaped duplicate keys, malformed/trailing JSON and unsupported argument
shapes, and enforces captured byte/depth bounds. Legacy parse remains last-wins;
legacy validate preserves its original duplicate-valid behavior. Existing JSON
large-input/output, hash and schema/surface consumers remain green.

Run1 failed compilation on a test-only attempt to read a CapabilityManifest field
from CapabilityDescriptor. Root corrected the test through the existing
`native_input_schema()` and `native_identity()` APIs; no production accessor or
guard was added or weakened. The failed run and complete diagnostic are retained.

The actual native Tool implementation is the new application/test artifact,
verified through `retain_native`; worker227a4b… has unchanged dependency inputs
and is reused without rebuilding. Old native references are never relabeled as
the successor. This does not qualify every old-native upgrade/loading route.

No new Skill/Recipe is activated or approved. Exact association/independent-pure/
workflow and Free root/child/review qualification remain their existing owners.
Strict format parsing alone cannot establish provenance, permit prefill import,
approve a model design or complete B1/B2/I1/I2. BUILD-005's earlier images retain
their exact prior HostRuntime scope; later combined qualification must use this
current native artifact. Platform/deployment/release and the full plans stay open.
