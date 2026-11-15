# Child-VM cancellation regression — 2026-11-15

Main CI on `5bf4fe8e7` passes both association-review cases on macOS and Linux,
then fails `busy_python_yields_and_cannot_catch_a_supervisor_cancellation` in
`tests/monty_control/tests/child_host.rs`. It expects `Python`, but the actual
host returns `ResourceLimit`. Jobs 132902117334 and 132902117356 preserve the
original failures. This is distinct from the repaired immutable-record trigger.

The host's `RecipeVm::accept` has deliberately separated supervisor cancellation,
terminal control and exhausted accounting from Python domain exceptions since
`2db8822d28`. Cancellation retains its `Cancelled` diagnostic, clears the
artifact and leaves the VM terminal. The test expectation had not followed that
change. Production failure classification is unchanged by this repair.

The corrected test requires `ResourceLimit` plus the original cancellation
diagnostic. Its Python handler now prints a marker if it executes; the native
test requires empty stdout and a second continuation attempt to fail with
`Terminal`. This strengthens the uncatchable/no-resume claim rather than merely
accepting another generic failure.

## Actual acceptance

`target/validation/ci-child-control-20261115/` records the exact command, complete
Cargo/native output, source snapshot, selected package graph and executable hash.
One native compilation/test command runs through the existing serial local
build owner with Rust 1.96, two jobs, the configured canonical target and offline
locked dependencies. All eleven actual `child_host` cases pass, with zero failed,
ignored or compiler-warning cases. Compilation and execution take 127.880 s.
The executable SHA256 is
`79d3beeb7aa39c4bc7673417d8b02fb51771f4e45c7b11af921e90a969878152`.

The actual Cargo graph contains no Composition dependency. Its package inputs
and the captured source remain unchanged throughout this run. The separate
startup-transfer work is not exercised or qualified here. No PostgreSQL,
provider, cloud build or full-platform CI acceptance is claimed.

The Screen ends with exit 0, no signal/error; the final IPC notification is
accepted and actually received. Its recorded private socket directory is empty.
Root informs the separately authorized startup chat that the local Cargo slot
is free. This receipt closes the scoped test repair, not the remaining Bob plans.
