# Global Monty lifetime — scoped acceptance, 2026-11-15

The runtime retains the actual global driver independently of the turn worker
until worker, private I/O, VM and review/settings shutdown stages have all been
attempted. Errors remain explicit. The catalogue's validator bridge holds a weak
attachment and upgrades once for an operation, including its asynchronous wait.
An expired bridge fails before admission; it never selects another owner or VM.

The native regression exposed a second strong cycle after the first was repaired:
Factory -> catalogue -> child runtime -> turn store -> registered Factory.
The composition owner now registers a weak private-claim participant. The public
Turn-State interface and retained reservation objects remain unchanged. Closing
private admission and retaining a reservation share the existing task-registry
lock, so closure prevents another reservation or custody acquisition.

## Actual checks

Source basis: `6f4c6f4e040d93d1b9960e6e44547db00d156ee5` plus the captured eight
pending Rust files in Composition. Exact per-file hashes and commands are in
`target/validation/bob-native-repair-20261105/` at the named runs.

| Check | Result |
| --- | --- |
| Run110, Rust 1.100.0 relevant all-targets strict lint, including production/test feature graph | Passed, no warnings; 123.81 s |
| Run111, Rust 1.98.1 established nine-program native feature union | Passed, no warnings; 719.56 s |
| Real runtime/bridge shutdown, graph release and expired-call refusal | One passed, none ignored; 332.58 s |
| Original and late-waiter native Q1 delivery | One passed, none ignored; 310.25 s |
| Interrupted-worker unresolved delivery without replay | One passed, none ignored; 300.61 s |
| Acknowledged I/O, pre-start fence and closed-admission refusal | One passed, none ignored; 956.83 s |

The four libtest cases use six independently owned temporary PostgreSQL clusters.
The last case contains three actual fixture executions, not three separately
selected tests. Unchanged programs stay warm; the normal worker is reused only
after complete dependency-source/configuration/compiler/environment/hash checks.
Only the changed Composition program and its adjacent companion are staged for
execution; other original programs are referenced without duplicate copies.
No external provider, production database, Modal job or simulated result is used.

The lifecycle case attaches the production-shaped bridge explicitly because
unit runtimes normally omit it. It stops the turn worker, confirms bridge access
while the runtime remains live, then performs real runtime shutdown. With the
external bridge still held, weak observations prove release of driver, Factory,
catalogue and native application image. Late calls fail and create no Work row.
The production field/attachment is separately source-reviewed and compiled;
the libtest's additional control handles do not prove exclusive production
ownership on their own. General crash recovery is outside this acceptance.

The earlier Run109 compilation passed, but its actual lifecycle case failed
because the Factory remained retained. That failed receipt is preserved; its
assertion was not removed or weakened. Intermediate Runs106–109 do not replace
the final Run110/111 acceptance.

Final receipt: `run-111-b1-validator-lifetime-native-r2/accepted.json`, with native
details in `native-lifetime-acceptance/`. Source and actual artifacts remained
unchanged throughout execution. Both Screen sessions ended normally; their final
notifications were accepted and actually received. Process inspection confirms
no associated PostgreSQL, Monty, compiler or test process remains.

Final reporting/CI/Changelog changes follow the tested image. They do not establish
an exact final release package, general authored approval, complete Q1, successor
publication, durable recovery, another platform or completion of any Bob plan.
