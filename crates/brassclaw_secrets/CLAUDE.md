# brassclaw_secrets guardrails

- Own scoped secret metadata, storage, and one-shot lease mechanics only; runtime injection is not enforced here.
- Never expose raw secret material through metadata, errors, debug output, audit records, events, or dispatch results.
- Preserve tenant/user/agent/project isolation; no global handle lookup unless an explicit admin-scoped API is introduced later.
- Do not implement authorization, approval, run-state, runtime injection, network access, process lifecycle, or product workflow semantics here.
- Keep raw secret access explicit through `SecretStore::consume(...)`; consumers must request a scoped lease first.
- Treat `SecretStore::put(...)` as a trusted setup/composition/storage-code primitive, not a runtime/plugin API.


## Trusted setup generation primitives

`SecretStore::record_reference` authenticates the selected encrypted generation
and projects an opaque owner/backend-bound reference without material, a lease,
locking a PG secret row or migrating legacy bytes. `matches` compares inside the
same owner using fixed-size in-memory digests, never persisting or returning a
plaintext fingerprint. Neither method proves runtime eligibility or authority;
comparison can identify an expired key while ordinary use still fails expiry.

`put_if_record_reference` atomically checks the captured generation and writes;
`expected: None` protects absence, including concurrent insertions. A mismatch
returns None without a secret write. An error is an unconfirmed acknowledgement,
not evidence of no effect. Actual PG transaction ports remain the native owner;
FS uses the actual backend record Version/Absent CAS and the legacy memory owner
uses its real write lock. Thin wrappers must forward these operations and their
fault/observation behavior rather than emulate CAS with get plus unconditional put.

These primitives alone do not make a mutable handle an immutable original,
retain a whole publication workflow or fence a lease across later replacement.
The caller still owns durable original input/result/cleanup custody; K3 step4
owns authenticated generation capture and final lease-use fencing. Runtime raw
material continues to cross only the scoped one-shot consume boundary.


## Immutable original and retained values

`put_original` stores the first full scope, typed trusted context and encrypted
raw input under one immutable identity. Exact retries return its actual stored
metadata; a changed input/context/full scope or occupied ordinary/retired identity
returns no write. Context contains nonsecret setup data, never token material.
`custody` authenticates the actual original/retained/retired record without
returning raw material. Original and retired values are unavailable to runtime
metadata/leases. `put_retained_from_original` selects an exact live original
inside its owner and creates one immutable runtime output; it accepts no caller
replacement material. Neither metadata nor a copied reference is authority.

All ordinary put/CAS/delete writers authenticate selected state and cannot
repurpose a custody identity. Delete retires a custody record, zeroizes the raw
material and leaves its original/source references occupied; it never resets the
identity for a later input. Ordinary FS removal uses native conditional single-
entry deletion with the version and encrypted-body checksum from the same read.
Missing/stale/already-retired means no mutation, not proof of current absence.
Errors may follow a committed effect and require original disposition readback.
Unsupported conditional storage never falls back to unconditional mutation.

Memory uses its actual owner lock; native PG exposes the same operations within
the caller transaction and orders its multi-record writer locks. CAS-only FS
selects authenticated source data at its read, not an atomic multi-record grant.
Original retirement does not revoke an already issued output. AuthProduct still
owns account/flow/result/browser publication and reconciliation; the storage
ports alone do not implement that cutover or K3 step4 generation-fenced leases
for ordinary mutable values. A raw retry after original retirement is a conflict;
a payload-free lookup of a confirmed original is a distinct operation.


`PgSecretStore::put_original_and_retained_in_transaction` acquires the source and
output writer locks once in their shared sorted order, then uses the same private
original/retained implementations as the individual transaction ports. It copies
only the exact authenticated original; it returns no raw material. The caller
must abort on `None` or error and owns commit/rollback and all domain publication.
The primitive neither commits nor marks an AuthProduct account/flow complete.

## Selected generations and issuer lifetime

Lease issuance pins the actual authenticated encrypted generation. Consumption
checks that same selection before releasing material; a later replacement is
never read through an old handle-only lease. Native PG keeps its row lock and
lock order; FS authenticates the complete lease selection/state and uses body
plus version CAS. Old unproven leases remain revocable but cannot read latest.
These lease guards do not establish a complete original task-family auth grant.

`lease_once_if_record_reference` checks a caller-selected encrypted generation
inside the actual owner and returns no lease on missing/mismatched selection.
It cannot be emulated by reference reads around an unconstrained `lease_once`.
Native PostgreSQL checks under its actual row lock before any legacy upgrade,
then pins the canonical result in the lease without reversing advisory-lock
order. FS pins the same authenticated snapshot it compared; this is not a
multi-record transaction, and later replacement is rejected at consume. The
legacy owner selects encrypted metadata under its own lock and retains its
existing UUID/usage guard at consumption. Delegating/fault wrappers preserve the
selected operation. Scope, issuer lifetime and revocation remain independent;
this primitive alone does not capture task families or prove refresh lineage.

`put_with_expiry` is a trusted setup write for the exact optional issuer deadline.
Finite deadlines use an explicitly selected authenticated encryption purpose,
not raw-token JSON sniffing or an unprotected metadata field. Unsupported owners
reject finite expiry before writing; delegating wrappers preserve actual owner
semantics. None clears a prior deadline for a newly written token and preserves
legacy unknown-lifetime encoding. Comparison/reference projection can identify
an expired generation without making it runtime-eligible.

Public leases carry the selected `secret_expires_at` for the existing host
staging handoff. Issuance/consume recheck actual expiry after awaited storage work;
expired material never returns merely because it was valid before a wait. Native
transaction helpers leave commit/rollback with the caller; the ordinary committed
consume keeps its consumed disposition even when the acknowledgement arrives
after expiry. Host runtime must retain the selected issuer deadline through final
injection, separately from lease/session/VM and staging cleanup budgets.
