# BUILD021 — native Chromium result-limit editing

The [machine receipt](2026-10-31-native-browser-limit-clear.json) records actual
attempts and inputs. SHA-256: `c14fdf2ffc2c244b251da24c745f3e85dcab34389bf89c9d2a9d55c020c0ff4d`.

Both original CI browser consumers passed against the real router, facade,
PostgreSQL and global Monty: **two cases, zero ignored**. Tool-policy checks retain
exact large values, invalid-input rejection, allow/block, revision409, lost commit
acknowledgement and no automatic replay. Clearing writes exactly one null limit
under the original revision. Resource settings and EN/DE help also passed through
their unchanged original script.

Read-only observations reproduced lost pointer focus: Home/Shift+End/Backspace
left the value unchanged. Explicit focus worked, but native Meta+A did not select
all, so Backspace removed only the final digit. The final test uses nonempty
invalid-space fill, verifies its unsaveable React state, then removes it through
real Backspace. No DOM values/events or server replies are fabricated. The pinned
native CLI has a configurable bounded test deadline; R3 exercised its timeout.
R1 never executed the browser; R2/R3 failures and real cleanup are preserved.
A requested SIGTERM found the old wait already exited and terminated nothing.

The actual Composition image8c7550fa… and8,190 matching inputs were reused.
This JS-only repair needed no Cargo. Evidence is local macOS/Node25/Chrome149;
Linux CI and installed/platform acceptance remain separate. Full K3–K7,
Pure/S/function review, Prefix and plan completion remain open.
