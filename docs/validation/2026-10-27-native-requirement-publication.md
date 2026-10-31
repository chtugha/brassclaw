# Native manual requirement/account publication

Status: **native prerequisite accepted; the complete K3 vertical remains open**.

The existing PostgreSQL Auth owner now publishes the exact requirement/account
selection in the same transaction as the authenticated original/retained
credential, account, interaction and completed original flow. Its existing
requirement-head table is reused. An existing head is checked against its actual
original flow custody, the captured previous selection and native revision/data
CAS. The unique key serializes absent-head competition. A losing or rejected
publication rolls back the whole transaction; no advisory-lock registry or
alternative credential store is added.

The exact immutable selection is retained in original flow custody after later
head replacements. Raw repeats prove the original material, and metadata-only
continuation verifies actual original completion. Neither path rereads today's
account to replace the result or republishes an old head. A bare interaction
cannot publish this requirement independently of its whole-flow owner.

New native manual originals use `native-manual-token-publication/2`. New accounts
retain requested scope metadata from their captured usage; a bound update
preserves its declared scope metadata. This is not evidence of external issuer
privileges. The reader still verifies `/1` originals with their historical
scope-clearing rule, retaining original results. A legacy missing selection does
not become a grant or a new binding during recovery.

## Actual acceptance

Receipt: `target/validation/requirement-publication-20261027/run-1/accepted.json`,
SHA256 `51a550d2aa857aac9c32446ebd20c08c0ac34c2923863f91d6dfa66d37e3fad4`.
The current common checkout was built serially in background screen using the
inherited canonical Cargo target. Exact four-path source/previous-byte manifests,
compiler inputs and the actual executable are retained with the receipt.

**116 freshly executed passing cases**, zero ignored:

- All41 PostgreSQL Auth-owner cases, including four added cases: actual lost
  COMMIT acknowledgement with later replacement and original recovery; two
  independent absent-head publishers; actual final-flow SQL rejection and whole
  rollback; and real encrypted archive-shaped `/1` original recovery. The archive
  fixture does not claim execution of an older application image.
- All75 existing filesystem Auth-owner regression cases, including real memory/
  PostgreSQL owners, boundary rejection and lost-write acknowledgement cases.
  They retain their existing supported scope; they do not implement or certify
  filesystem requirement publication.

Strict Composition library/tests Clippy (`-D warnings`,98.3s), the actual native
test build (221.4s), default-library Clippy, workspace formatting and whitespace
passed. The test executable SHA256 is
`14942dcfe5001ab445dd338fd11b3fa19978d0479f6147028eecd3e4be8aca62`.
The existing R6 Monty worker is explicitly reused from its immutable receipt:
the four changed Composition Auth files are outside its dependency graph. New
Auth code was freshly compiled/executed. No validation process remained at the
terminal check. No permitted unembedded peer document changed during this run.

## Remaining work

Filesystem and fake publishers, OAuth/account-choice publication, deliberate
sharing, original task-family/child/wait selection and material-generation
consumers remain open in K3. This does not qualify the new complete WebUI/API/
Recipe consumer path, general live credential use, Prefix serving or the full
Simplified/Validator/Composition plans. Selection metadata and content references
remain independent of actual credential eligibility and live Tool authority.
