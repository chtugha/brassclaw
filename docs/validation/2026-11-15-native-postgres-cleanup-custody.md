# Native PostgreSQL fixture cleanup custody — 2026-11-15

Both existing Reborn and Composition test-fixture owners now retain their own
cluster directory when an orderly stop fails. The supervised cleanup thread
borrows its server owner instead of moving it: a failed stop, thread panic or
thread-start error must not invoke the server's immediate-stop destructor and
then delete potentially live data. Failure stays visible. Ordinary successful
cleanup still removes the stopped cluster.

Each owner has one real regression in its existing parent test program. It hides
only that fixture's control executable, observes the failing destructor and
retained live cluster, restores the executable, performs an orderly `pg_ctl`
stop, confirms exit before removing files, and checks normal cleanup separately.
The tests use exclusively their own temporary databases. No production database,
model, simulated shutdown, forced termination or global setting is involved.

| Check | Actual result |
| --- | --- |
| Run103, Rust 1.100.0 strict relevant all-targets lint | Passed, no warnings; 129.45 s |
| Run104, Rust 1.98.1 warm nine-program first check | Passed, no warnings; 725.24 s |
| Run104, missing Reborn program second check | Failed at the disk gate before Cargo; original failed receipt retained |
| Run105, missing Reborn resource program | Passed, no warnings; 185.92 s |
| Composition native cleanup regression | One passed, zero failed/ignored; 4.00 s |
| Reborn native cleanup regression | One passed, zero failed/ignored; 3.16 s |

Run105 reuses only Run104's successful first check after verifying identical
source, compiler, environment and original artifact hashes. It does not rebuild
those nine programs or relabel the failed overall Run104. Native cases execute
the retained primary binaries directly without another staging copy. After the
cases, no associated PostgreSQL/test process remains; Screen completion,
notification acceptance, actual chat receipt and session closure were confirmed.

The combined scoped acceptance is
`target/validation/b1-native-cleanup-20261115/accepted.json`; underlying runs are
under `target/validation/bob-native-repair-20261105/`. The four changed Rust files
are test-only. This receipt does not qualify production shutdown, fix a production
ownership cycle or close any complete Bob plan. The separate
[cache cleanup](2026-11-15-audited-cache-cleanup.md) restores disk headroom without
discarding required original artifacts.
