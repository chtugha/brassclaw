# HTTP response save: sensitive target checks

Status: **scoped host/filesystem repair accepted; full K3 and plan work remains open**.

The existing scoped byte-write adapter now checks the invocation's write grant,
the visible and resolved names with the shared sensitive-path rules, and actual
backend metadata. A missing leaf checks its nearest existing parent; a sensitive
target/parent or incompatible file type rejects. No metadata or read grant is
returned to a write-only caller. HTTP performs this preflight before network or
credential injection, then repeats it before writing the downloaded body.

The native opaque write still atomically rejects structured-record replacement.
Trusted Auth/Secret record owners retain their separate write APIs. This does
not turn every opaque JSON file into a structured record or introduce a blanket
virtual namespace ban. Local metadata checks assume a trusted host filesystem,
as the existing Coding path does; they do not provide descriptor-based protection
against malicious filesystem mutation during the final check/write interval.

## Actual acceptance

Receipt: `target/validation/http-sensitive-save-20261027/run-1/accepted.json`.
SHA256: `c2e22e421c074e699755a06e2a58f8311083e9f8a99554240bf6c3b53b7075ad`.
Serial background screen validation used the inherited canonical Cargo target,
with complete compiler-input hashes and immutable actual test images retained.

**266 freshly executed passing cases:** Host Runtime221, HTTP43, native file
boundary1 and native live Coding-settings1. The native file case exercises real
memory and PostgreSQL owners and both HTTP authorization and actual writes.
Five added HTTP cases prove sensitive visible/resolved names reject with zero
network/no writes; harmless aliases to actual sensitive local targets and missing
parents reject; an existing `.env.example` and new safe parents remain writable
through exact write-only grants with an empty default resolver; a real record
conversion during transport is preserved; and, on this Unix host, a local alias
changed to a sensitive symlink during transport is preserved. Post-network errors
retain actual network usage. Existing structured targets now reject before network.

Three existing Docker tests are explicitly unexecuted because Docker is absent.
The platform-neutral implementation is not Windows or Linux execution acceptance;
the additional Unix symlink case is gated accordingly. Strict affected Filesystem,
Host Runtime and Composition test/default Clippy, formatting and whitespace pass.
The actual worker was freshly built because Filesystem is in its dependency graph;
its resulting bytes are unchanged. The Composition test image was freshly built.
Prefix Source6 remained unchanged throughout; no Cargo/Screen process remained
at the terminal check. No Prefix serving/activation or full-plan acceptance is claimed.
