# Filesystem manual requirement/account publication

Status: **CAS-only publisher prerequisite accepted; full K3 remains open**.

The existing filesystem Auth owner now retains the actual original Flow custody
in encrypted `filesystem-manual-token-original/2` context. New accounts retain
requested scope metadata; bound updates retain their existing declared scopes.
These are not proofs of external issuer privileges. Immutable `/1` originals
keep their historical scope/result semantics and gain no missing requirement
binding during recovery.

The owner stages its exact selection in the original flow, publishes the existing
requirement head through absence or version-and-body CAS, and only then marks
the flow Completed. The ordinary selection reader requires completed original
custody, so the head's temporary presence is not a usable selection or grant.
Lost acknowledgements read back the original staged/head/completed records.
Completed originals never republish an old head after a deliberate replacement.
Bare interaction submission cannot publish a new requirement independently.

Unlike the native multi-record transaction, a CAS-only loser may already have
published its account/material. Those actual effects and receipts remain intact;
the original records Failed/AccountSelectionRequired, without a successful
continuation, false rollback, cleanup or repeated credential publication. Unknown
backend outcomes remain at the original operation until readable reconciliation.

## Actual acceptance

Common receipt: `target/validation/fs-requirement-publication-20261027/run-1/accepted.json`.
SHA256: `b4cdeedd8d22b9a862ff0284c57018b9315567bfec8bffa8868d7b0c94cf04f5`.
The full common checkout was frozen with both owners, built serially in screen
using the inherited target and verified before/after checks. Exact Source5
originals/current bytes, compiler-input manifest and actual images are retained.

**119 fresh Auth passes**, zero ignored: all78 filesystem-owner cases, including
three new cases, and all41 native PostgreSQL-owner regression cases. The new
Memory/PostgreSQL sequences use actual encrypted FilesystemSecretStore records,
drop Auth record-port acknowledgements after real writes at stage/head/final
boundaries, reject a final write before mutation, reopen original owners, and
verify the incomplete head remains unusable until completion. Independent owners
capture absent and existing heads; the losing original and partial account stay
explicit. Deliberate bound updates retain scopes, later head replacement leaves
original recovery unchanged, and changed raw input rejects. The third case uses
actual encrypted archive-shaped `/1` context, not an old executable; relabeling
it `/2` without its required custody rejects.

Strict Composition tests Clippy (`-D warnings`,79.1s), fresh test build (214.1s),
default Clippy (16.2s), formatting and whitespace pass. Actual Composition image:
`ef2e30cafdd2843d9add0fb3b4b38ac345edc9a1c5590675a501bede5ce616db`.
The worker build returned its unchanged actual artifact. The Prefix owner's
separate image-only check additionally passed native3/shape2/operator-Pair2,
with its own scoped receipt; these seven passes are not counted again as Auth.
Source6/Prefix Source2 stayed unchanged; no build process remained at completion.

## Remaining work

Fake transitions, OAuth/account-choice publishers, deliberate sharing and original
task-family/child/wait credential selection/material-generation consumers remain
open. Runtime still has legacy compatible-account enumeration; these publisher
checks do not qualify the complete WebUI/API/Recipe consumer path or live grants.
No Prefix serving, full K3–K7, full-plan, platform, release or deployment acceptance
is claimed.
