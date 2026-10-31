# Google private setup original — 28 October 2026

Status: **scoped native producer/callback/refresh/cleanup acceptance; remaining caller and generation consumers are open**.

The existing Google host owner now captures the first complete client configuration
and PKCE verifier in one immutable encrypted write. Safe context pins the initial
flow, scope, usage, account snapshot, timestamps and challenge. A failed first flow
write leaves the preparation recoverable; neither retry nor a changed current client
can substitute its bytes. No second registry, task loop, VM or Tool was added.

The ordinary Google provider factory uses this same owner for callback and refresh
client material. New Google starts require original flow/invocation IDs and use the
host's private store. The dedicated Google callback uses that store instead of the
route's process-local cache. The older raw Google setup endpoint rejects before
creating a challenge without a client original; unrelated raw-provider compatibility
is not migrated by this packet. Gate identities retain owner/run/gate and sorted
predecessors, without a process mutex or a wall-clock latest-row assumption.

Retained private aliases use conditional generation leases. The stable use marker
survives retirement and fences missing-flow reconstruction. Refresh bindings keep
client-only material and original/use lineage; a rotated binding owns its own copy.
Supported confirmed terminal cleanup erases raw PKCE through the existing identity
closure primitive, including reserving absent aliases against late output creation.
Uncertain issuer effects stay retained. Current account lifecycle receipts capture
the private client's two handles alongside the selected refresh secret. Old receipts
keep their absent material list and original context; no historical receipt is
silently reinterpreted.

The initial Extension account lookup also now passes its already captured requester.
Previously that lookup could reject a valid Extension-owned selected account before
the explicit authorization comparison. This does not broaden backend permissions.

## Actual evidence

Receipt: `target/validation/google-private-original-20261028/run-2/accepted.json`,
SHA256 `50e9b6a463f2b2f7d0e7e823cd91157a398c0a3c4d1e83c47ba1c17edbdd39ec`.
Owned15 manifest SHA256:
`b980283c1c56922dfc941e5f82ef284c0da976f1ced99a4d6386cf91f8009759`.
Whole compiler-input inventory SHA256:
`caf6f341796c53788805103ba17f28ef384a9c5f7da3b24b40d02245a9e19c91`.
Originals/proposals/manifests/continuation:
`/private/tmp/brassclaw-google-private-original-20261028-iz83ftep/`.

**531 actual Rust tests passed, zero ignored**: Auth119, Product207, native PG51,
record-filesystem88, Google gates6, provider composition7, public OAuth routes19,
DCR32, global Monty1 and architecture boundary1. The route suite executes actual
Chromium. Formatting, strict connected/default-feature lib/tests (`-D warnings`),
actual Auth/Product/Composition/worker builds and whitespace pass. No warning was
suppressed. R1 failed on a new test's wrong account-port getter; its complete source
and diagnostics remain preserved. R2 fixes only that fixture getter and runs the
complete scoped batch; it does not count the failed compile as acceptance.

Both actual native owners deliberately reject the first flow insert after encrypted
preparation. A recreated owner with a different current client restores that same
original. A freshly mounted public HTTP callback with no route cache performs the
actual HostOAuth transport against controlled issuer replies; replay makes no second
issuer request. Refresh/rotation uses the first client, completed PKCE is retired,
and ordinary Uninstall closes the account's current private client bindings. The
cases use a real Extension-owned account and the actual requester-preparation port.
Additional cases cover independent gate owners, cancellation before the first use
alias, closed absent refresh bindings and legacy-no-client rejection with zero HTTP.

Actual Composition image SHA256:
`67138fe5b05f7b29307769202653322f9a7f12624b127274fb69d24d0c8304ef`;
worker SHA256:
`a4387d629652101c90de222af962218f4ca3e153408198ca3be9c21f20311143`.
Runner59045/Screen59044 and observed native test61126 are verified absent; the
shared Cargo lock was independently acquired/released. Prefix Source30 and the
final Exact documentation input record were captured unchanged. The recorded
embedded/compiler inputs are checked separately from unembedded plan observations.
Fresh Auth/global checks do not qualify every changed Teaching/Q1/Serving subject.
Both owners received the terminal receipt and free serial window.

## Remaining before broader completion

- The standalone Google start response still assumes a challenge URL; align its
  terminal/elapsed original projection with the existing Extension response.
  Older external UI caller fixtures also need their real original owner and explicit
  IDs/target/requirement contract. They were compiled, but their external test target
  was not executed by this scoped batch. This receipt is not complete standalone or
  full WebUI caller acceptance.
- The common raw refresh-token consumer still uses handle-only `lease_once` and
  the refresh intent does not yet retain its encrypted generation. Repair that
  preparation/native-claim/dispatch/conditional-consume handoff in the existing
  owner; immutable client lineage alone does not pin a mutable raw token.
- Old Google flows/tokens without captured client bytes cannot establish equivalence
  to today's configuration. Preserve their metadata and require explicit fresh
  authorization; never manufacture historical private provenance. Upgrade/recovery,
  original task-family/material consumers, pending failed/unknown effects and older
  retained-value lifetimes remain with their existing owners.

No live Google issuer, installed upgrade, platform-wide, real Prefix tokenizer/design/
serving or complete-plan acceptance is claimed. No commit, tag, push, deployment,
SSH operation or target cleanup was performed.
