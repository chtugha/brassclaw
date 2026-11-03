# B3-09 login-observer projection fencing

Date: 2026-11-03. Scope: browser-side transport and login caller behavior only.
This is not native Auth, issuer, PostgreSQL, browser-rendering, installed-asset
or complete B3-09 acceptance.

## Change and caller path

The existing `runCodexLogin` passed its abort/context fence through status/start
requests but called `reconcileProviderMutation` without it. That helper emitted
bootstrap notifications before the caller checked the provider/role/model
receipt. A delayed response or matching mutation ID with foreign receipt fields
could therefore affect the shared UI before the login caller rejected it.

The shared reader/reconciler now check the supplied observer fence before
dispatch and after response parsing, and synchronously validate the receipt
before changing pending state or emitting settings/bootstrap notifications.
Codex supplies its original auth/context/generation fence, AbortSignal and exact
receipt validator. NEAR supplies its existing current-observer fence and exact
mutation/operation/provider validator. Other mutation callers retain their
existing behavior; no new endpoint, persistence owner or automatic login retry
was introduced. Abandoning the browser observer does not claim cancellation or
rollback of already accepted native work. Originals remain available for an
explicit supported resume.

## Actual checks

All commands ran locally, without Cargo or remote compute:

```sh
node --test crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/codex-login.test.mjs crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/provider-login.test.mjs crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/provider-mutations.test.mjs crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/provider-bootstrap.test.mjs crates/brassclaw_webui_v2_static/static/js/pages/settings/hooks/useProviderLogin.test.mjs
```

First run: 54 selected, 52 passed, 2 failed, none skipped. The two newly added
mutation tests incorrectly expected no events at all despite the initial submit
having already emitted its legitimate pending notification. Their assertions
were corrected to compare against the captured pre-observation event list.
No implementation change was required by those failures.

```sh
node --test crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/provider-mutations.test.mjs
```

Second run: all 13 mutation tests passed, none skipped. The other 41 tests from
the first run remain applicable: their modules, implementation inputs and tests
did not change between runs. Combined evidence: 54 distinct passing cases.
Whitespace validation passed for the changed implementation and tests.

New regressions exercise the actual shared helpers and Codex caller with
controlled fetch responses: pre-dispatch abandonment, delayed response after
observer/auth changes or abort, AbortSignal propagation, foreign-provider
receipt rejection before notification, supersession from the terminal-status
callback, unchanged pending custody, and one normal exact bootstrap notification.
These controlled HTTP responses do not certify the server-side protocol.

## Build isolation and remaining work

The other chat's T5 Cargo/test process was terminal and no Cargo/compiler process
or shared-lock owner was present when these implementation edits were applied.
The implementation application held `/private/tmp/brassclaw-target-cargo.lock`
and checked compiler processes. No Rust, validator, PostgreSQL, Modal-image or
other-chat implementation file was changed by this work package.

The static crate embeds these JavaScript assets. The next coordinated native
build must package the final bytes; this receipt does not certify an older
binary's assets. B3-02/native refresh, remaining B3-09 projections and the full
plan's connected acceptance remain open. No compilation or test on the test
machine, commit, push or release was performed.
