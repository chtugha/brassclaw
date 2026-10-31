# BUILD028 — actual encoding prerequisites and shared chat fixture

The [machine receipt](2026-10-31-ci-encoding-chat-fixture.json) binds sources,
actual executables, native logs and fixture/runtime identities. SHA-256: `04b096b087ee20b6f9ca6ab8d29a84c1a61e1fb45ac127cd52a880866fca0b6c`.

The mandatory encoding consumer now honors the existing Python override. CI
supplies the existing independent HF oracle, CPU dependencies, six exact public
tokenizer/config files and the actual worker path. No model weights or provider
configuration are needed. Full validation explicitly runs the four existing
retained HF oracle cases; their new GitHub wiring remains unverified locally.
The helper checks existing bytes, bounds downloads by exact fixture size/hash
and publishes exclusively. Five CLI cases include a real pinned1125-byte download
and rejection of wrong bytes, symlinks, offline absence and nonfinite timeout.

Two unscoped workers over shared PostgreSQL may claim either admitted task.
The original test gave each a private one-answer replay queue, so a legitimate
claim by the other worker could exhaust its model fixture. Both now use the same
existing request-keyed replay source. All original history/identity assertions
remain, plus exact request count and foreign-content negatives.

Actual encoding, two-worker chat and both unchanged strict error-evidence cases
pass (four native cases), with affected strict Clippy and workflow lint. No case
is ignored. Root/support fixture capture was extended during compilation before
native execution:132 additional inputs all predated initial capture, matched Git
index and were independently checked unchanged at terminal. The original8250
input guard also passes; this timing limitation remains explicit in the receipt.

No production workflow, failure classification or permission boundary is changed.
GitHub033 failures remain historical failures. These local macOS results are not
full CI/platform, component/Serving/Prefixless activation, K3 or plan completion.
