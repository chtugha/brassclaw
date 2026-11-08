# E-01 Native Run 18 test length correction

Root Run 18 was terminal before this correction. Read its complete diagnostic at
`/Volumes/SSDE/brassclaw/target/validation/bob-native-repair-20261105/run-18-composition/compile-1.log`.
The Composition test compile reported two E0308 errors: the serialized claim
length was `usize`, but `MontyExecutionLimits.max_value_bytes` is `u64`.

The only implementation change converts the actual serialized Vec length once
using `u64::try_from(...).expect(...)` before the existing reduction and strict
comparison. Both operations therefore use the same checked byte count and the
actual settings type. No `as` cast, synthetic limit, reduced coverage or changed
assertion. Execution behavior and native selectors remain unchanged.

Concrete DTO audit of the new test source:

- `MontyExecutionLimits.max_value_bytes` is `u64`; `max_value_depth` is `u32`.
  The latter already uses checked `usize` addition and checked `u32::try_from`
  from `REVISION_LIMITS.max_depth`. Runtime contract depths/bytes are `usize`
  and remain behind the existing captured-preparation conversion.
- `MontyVmSettings.revision` is `u64`; the update's `expected_revision` is
  `Option<u64>`. Both JSON updates preserve the current revision's actual type,
  and their execution-limits payload uses the complete existing DTO.
- `ComponentRevisionRef.version`, allocation-head version and the `stage`
  expected version are `u64`; the authored-head comparisons use those values
  directly without a cast.
- Fixture/vector/component counts and pool maximum size are `usize`.
  The first-claim race explicitly converts its closed suite count to `i64`
  with `try_from` before comparing the actual PostgreSQL `i64` counts.
  Terminal-record retention uses the existing `usize` field with zero.
- ACK-cut rule IDs are `u8`; `cut_count` is `usize`. Existing literals at
  these calls/comparisons are inferred in the actual API types.

No additional concrete mismatch was found by this source audit; this is not a
compiler result. Ran rustfmt edition 2024 with `skip_children=true`,
`git diff --check` and sibling SHA-256 manifest generation/readback. No Cargo,
DB, tests or Modal. Root owns the next connected compile/native run.

Rootmain was `0c929ede58c68ae6ee101a2bb4dcc9ce054097b8`. The separately requested
unused-getter removal is already committed as
`8d8cb33aab0dafdb2b483b6cdb619bf68b7d6fb0` and has not been integrated by this
package. Integrate that commit and this follow-up only, not this branch's older
base. The sibling manifest supersedes only the checker-test source entry from
the Run 17 manifest; earlier manifests remain historical source evidence.
