# Fake OAuth/account-choice requirement publication — 27 October

Status: in-memory Auth test-service transition fidelity, not production-family
or external issuer acceptance.

One pure preparation helper checks actual protected original flow/custody and the
captured requirement head before manual, OAuth or account-choice mutations. Under
the existing mutex, all fallible checks precede account/flow/head/continuation
publication. A stale choice cannot leak success or account mutation. Completed
originals recover their retained result despite later heads or account revocation;
no current account lookup is used to reconstruct them. A conflicting OAuth head
keeps its already recorded returned outcome pending without publishing an account
or continuation, matching the native transaction's rollback boundary. This does
not pretend the CAS-only production backend can roll back an applied effect.

R2 receipt: `target/validation/fake-other-requirements-20261027/run-2/accepted.json`.
SHA256 `562f71dd12159ae5723a2fa9d77e4770e0308b33537d41d21c7351f51adba6d7`.
100 fresh actual tests pass (Units5/Contracts95), zero ignored. All-target strict
lint, format and whitespace pass. The two new public Choice/OAuth cases and
previous manual cases actually run. Exact two-file snapshots, test executables,
commands and hashes are retained in the existing receipt directory.

R1 rejected three new test calls that used an incorrect name for the existing
account_record port. All three were corrected after reading its actual trait;
no production API or lint was weakened. Preserve R1 failure/source and R2 correction.
No Composition rebuild or production acceptance is claimed for this fake-only
successor. Prior FS/native receipts remain tied to their own exact source images.
The terminal Runner/Screen are absent and Prefix Source6/Source2 stayed unchanged.

Deliberate sharing and actual original task-family/child/wait/material consumers
remain open. Current runtime account enumeration is not retained family selection.
This does not close K3–K7, complete plans, platform or deployment acceptance.
