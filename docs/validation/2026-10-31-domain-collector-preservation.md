# Standalone domain collector preservation

BUILD044 is accepted locally for the existing Defensive, Home Assistant and
tomedo collectors. Actual temporary Git repositories, source files and SQLite
including derived rows and FTS verify visible UTF-8/read/stat/programming errors,
rollback across repositories, closed connections, preserved working outputs,
confirmed deletion and literal CRLF. Five methods / fifteen domain subcases pass,
zero skipped, in 4.7 seconds. No Cargo run is required for these standalone files.

[Exact inputs, command and result](2026-10-31-domain-collector-preservation.json)
retain the actual Python 3.14.6 / SQLite 3.54.1 runtime and log identity.
JSON SHA256: `3565c295b62c513acd8507d1158a04764a6de5384cce7484dc471b6dfde153be`.

This is scan-failure preservation. SQLite and later filesystem publication are
not crash-atomic. Unreferenced raw originals can remain after an aborted scan.
Other collectors, native domain migration, activation and Serving remain open.
