# BUILD025 — observe the confirmed OAuth original deadline

The [machine receipt](2026-10-31-oauth-observation.json) records source/image
identities, original logs and the separately observed Node result. SHA-256:
`71e5b8facaf02bb0e3dfc4f46c34fc941f6d19ed4ce53da8cea4b176e28d02f1`.

The existing watcher now receives the checked original receipt expiry rather
than granting a new ten-minute period on every open. A popup-blocked recovery
link also starts observation. Invalid or expired deadlines never create another
window; popup close, unmount and confirmed terminal receipts clean up the timer.
No server cancellation, replacement setup or authorization replay is introduced.
The2000ms refresh interval remains a separate open control.

26 actual Node cases passed once, zero skipped; their exact unchanged seven
inputs were verified and reused. Actual static/Composition embedded images were
rebuilt239.4s because old binaries would serve old JS. All28 static packaging
cases and the existing native HTTP/PG/Chromium case passed, with static strict
Clippy `-D warnings`. The browser audit delegates actual timers unchanged and
verifies one ticking blocked-popup observer, then no active observer after its
confirmed original cancel. Earlier identity, lost-ack and unknown-cleanup
assertions remain intact. Actual workerbe0f0d6e… is unchanged and verified.

The native fixture controls issuer replies. It proves UI/HTTP/PG behavior, not
external provider health or component approval. No Rust behavior, SQL, guide or
release-reference change required repeating unchanged global/model gates.
OAuth creation/default/max/gate lifetimes, client-expiry successor, polling/
transport/ingress, K3–K7 and the full plans remain open. This is local macOS
acceptance, not new CI/platform/deployed support, release or PrefixFree approval.
