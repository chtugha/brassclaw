# Deferred Screen notifications — 2026-11-15

The existing Screen helper observes tool-call completion before sending direct
Desktop IPC messages. Its internal sender thread leaves command supervision and
signal forwarding independent. One pending slot coalesces heartbeats; completion
replaces unsent heartbeats. Unknown state withholds delivery and uncertain sends
are not replayed. There is no Node dependency, chat queue or separate service.

The starter is unchanged. The helper exposes configurable quiet, IPC-operation
and final-wait intervals, including 0.3-second quiet and 600-second final-wait
defaults. Final waiting starts at actual build completion. Build outcome,
notification outcome and session exit remain separate observations.

The user-commissioned Screen chat supplied the implementation and installed the
exact reviewed pair in the agreed window. Its 55 tests passed in 18.732 seconds.
Its real live probe completed a build in 3.133 seconds while a separate foreground
command ran 7.005 seconds without interruption or signals. Five waiting heartbeats
were superseded; only completion was sent, 0.358 seconds after that command ended.
App acceptance, actual chat receipt and session exit were confirmed separately.
Its prior actual inactive-chat wake check remains applicable to unchanged IPC code.

Root's final review corrected a macOS-only `/private/tmp` default: the helper now
uses the system temporary directory, and Unix fixture sockets use short `/tmp`
paths. All 55 tests passed again against the final installed pair in 18.544
seconds. The prescribed starter used its new default successfully. The final
notification was accepted and actually received; the correct private Screen
socket directory is empty. Bash/Python syntax, whitespace, workflow YAML and
affected workflow shell syntax passed. The existing Ubuntu/macOS script job now
selects GNU Screen and runs these tests. Actual GitHub/Linux execution is pending;
this is current-host acceptance, not a cross-platform deployment claim.

The final receipt and original local logs are under
`target/validation/screen-deferred-20261115/`. The separately retained peer
production verification report is referenced by path and SHA-256 there. No Rust
compilation is required for these development-script changes. The observation/send
gap is not atomic, the IPC protocol is internal, and app restart or loss of chat
ownership can prevent delivery. Already-started heartbeat/cleanup work may outlast
the final-dispatch deadline; ordinary kernel filesystem stalls cannot be forcibly
cancelled by a Python sender thread. No complete Bob-plan claim follows.
