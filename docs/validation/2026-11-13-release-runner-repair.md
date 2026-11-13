# Release runner repair — 2026-11-13

The actual rc.5 release run [43347099426](https://github.com/chtugha/brassclaw/actions/runs/43347099426)
checks out immutable tag `v1.7.0-rc.5`, source `e466ed46c69a77ae73e4502c68101c3d3f57a86d`.
Its Intel job `131628974774` is cancelled before any step: `macos-14` resolves to
an unsupported retired configuration. This is not a Rust compilation failure.
The Linux/ARM jobs were still running at diagnosis; they are not marked passed.

GitHub's [official runner inventory](https://github.com/actions/runner-images/blob/main/README.md)
records retirement of all GitHub macOS 14 images on November 2, 2026. The
[runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
assigns `macos-15-intel` to x86_64 and `macos-15` to arm64.

Package W-01 (`01a1d29a-065e-7ee0-b5ee-1b7e3c9a4ffd`, isolated `34d4` worktree)
commits `17951f9f4a450d64a948303708c88b9f71240936` on exact `e466ed46c`.
Root reviews and fast-forwards the two-line patch. Only the two macOS labels
change: Intel `macos-14` → `macos-15-intel`, ARM `macos-latest` → `macos-15`.
Linux, targets, asset names, checksums, action pins, permissions and publisher
remain byte-identical. All eight active workflows are scanned; no other retired
macOS 13/14 references exist. Existing `macos-latest` test jobs stay unchanged.

Actual package checks pass: actionlint 1.7.12, whitespace, PyYAML 6.0.3 parsing
of all eight workflows, and a structural comparison permitting exactly those
two values. Root verifies the complete diff and identical integrated blob.
These checks establish CI configuration, not successful hosted compilation.
No local Cargo or Modal execution is needed for this CI-only repair.

Root prepares `1.7.0-rc.6`: root/CLI/path dependency, two lock entries, README pin
and a separate changelog section. The existing rc.5 tag is preserved, never
retagged/forced. The new release will build the actual new version/embedded
release bytes on the supported native runners. Earlier rc.5 native/CLI receipts
remain scoped to their original source/artifacts; unchanged Rust implementation
logic is reused, not claimed as new rc.6 exact-byte runtime qualification.
Actual hosted run [43348645122](https://github.com/chtugha/brassclaw/actions/runs/43348645122)
now passes all three native Build jobs, their uploads and Create Release. The
published [rc.6 prerelease](https://github.com/chtugha/brassclaw/releases/tag/v1.7.0-rc.6)
has all twelve application/worker/checksum assets. The superseded rc.5 run is
cancelled deliberately; it is not relabelled a successful compilation.

The previous push reports absent required contexts `build`/`test`. With the
user's explicit approval, Root replaces only those obsolete required contexts
with `Tests (Reborn)` and `Code Style (fmt + clippy)`, both bound to the verified
GitHub Actions app ID 15368. Fresh REST readback verifies the exact pair and
unchanged `strict: false`; complete before/after protection comparison verifies
all other rules unchanged. No green status is fabricated. Correct check names
are not evidence that their current runs passed.

The successful hosted builds use Rust 1.100.0, whereas the original local strict
repair receipts use 1.98.1. ANSI-normalized actual logs from all three rc.6 Build
jobs contain the same 93 manifest causes: fifty missing workspace-lint
inheritances, forty-two redundant homepage declarations and one unused Root
build dependency. Including 59 summaries, each log has 152 warning lines.
An initial raw-log scan missed warnings separated by ANSI escapes; Root corrects
that observation and preserves the actual logs. Empty/incomplete CLI log output
is not warning-free evidence. W-02 and subsequent repairs are being validated
for rc.7; its full-workspace diagnostic compilation is not yet strict/native
acceptance. See [the current handoff](../development-handoff.md).
