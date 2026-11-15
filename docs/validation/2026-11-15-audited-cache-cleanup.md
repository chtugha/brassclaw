# Audited cache cleanup — 2026-11-15

The operator requested deletion of unnecessary caches on both volumes rather than
further archival. The four operations below completed successfully.

| Removed category | Volume | Observed free-space increase | Receipt |
| --- | --- | ---: | --- |
| Obsolete cold compiler archives | SSDE | 60.84 GiB | `cold-archives-deleted.json` |
| Unreferenced native compiler objects | System | 20.37 GiB | `orphan-objects-deleted.json` |
| Hash-identical test execution copies | SSDE | 45.01 GiB | `redundant-test-images-deleted.json` |
| Unused duplicate Python virtualenv | System | 1.14 GiB | `unused-oracle-deleted.json` |

The operations freed approximately 127.37 GiB combined.
Immediately afterwards the system volume had 45.01 GiB free and
SSDE had 156.78 GiB free. These are observed filesystem deltas,
not a promise of exact physical space attributable to each removed byte.

The orphan-object audit protected all external object references in 120 current
Mach-O program debug maps, checked its parser against Apple's `dsymutil`, and
retained library-associated and multiply linked incremental objects. It removed
25,703 single-link orphan objects only.

The execution-copy audit matched all 150 selected generated files by SHA-256
against retained original artifact manifests and primary binaries. The files
were not open at deletion. Primary binaries, manifests, historical outcomes,
source snapshots, runners, logs, database directories and private worktrees remain.
The unused native-target Python environment was replaced by the already accepted
Oracle environment on SSDE; current tests already select that environment.

Machine-readable plans and operation receipts are retained under
`target/validation/disk-cleanup-20261115/`. Historical test receipts retain their
original meaning; removing disposable execution copies does not turn a failed
run into a pass or qualify new product behavior. No Cargo rebuild was requested
for cleanup. The missing Reborn resource-test compilation resumes separately,
reusing only the genuinely successful nine-program check from Run104.
