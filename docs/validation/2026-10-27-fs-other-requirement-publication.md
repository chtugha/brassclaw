# Filesystem OAuth and account-choice requirement publication — 27 October

Status: scoped acceptance through actual encrypted filesystem Auth owners.

Manual, OAuth and explicit account choice now use one head-publication helper in
the existing FS Auth owner. Exact flow/body CAS retains the selected original;
the head is written next and completion last. A pending head is unusable until
its actual flow custody completes. Cancellation settles a selected publication
before answering. Expiry persists before a new choice; a chosen original retains
its original time/account instead of selecting today's metadata on retry.

OAuth pins its requirement receipt with its original account-publication decision.
A head conflict after account publication retains the actual account effect and
records Failed/AccountSelectionRequired without replay or compensation. Historical
decisions missing that optional receipt finish their original result without
backfilling a new requirement binding. No new store, registry or workflow loop.

Receipt: `target/validation/fs-other-requirements-20261027/run-1/accepted.json`.
SHA256 `08e13fbc54f3ae28730be0b48c69917b43fda132d7b510b677423882133bdf21`.
126 fresh actual tests pass:FS82/PG44, zero ignored. Tests/default strict lint,
format and whitespace pass. The four new Memory/PostgreSQL cases run through the
actual record backends and encrypted FilesystemSecretStore. They cover stage/head/
final committed-write return loss, rejected final writes, cancellation before
acknowledging a pending publication, later choices/revocation, expired choices,
competing original choices/OAuth outcomes and archive-shaped decisions. The fault
wrapper drops a real committed record-port return, not an actual TCP packet;
OAuth returned-exchange data is a fixture, not external issuer acceptance.

Actual Composition test image SHA256 `d68b1147963f0e7d03e850fa7e6c4d990e4a7207688dc3613b19183e01819e6b`.
Exact source snapshot, nine-file changes/originals, emitted worker and logs are
in the existing receipt directory. The requested worker build reused its recorded
unchanged bytes. Prefix Source6/Source2 stayed unchanged; Runner/Screen were absent
before the freeze was lifted. No broad clean, parallel Cargo or target override.

Non-manual fake transition fidelity, deliberate sharing and actual task-family/
material consumers remain open. ProductAuthRuntimeCredentialAccountSelector still
enumerates current accounts; this package does not claim original-family capture,
material-generation pinning, full K3–K7, platform/deployment or plan completion.
