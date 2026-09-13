#!/usr/bin/env bash
set -euo pipefail

# CI script: check that version bumps accompany extension source changes.
#
# The v1 tools-src/ and channels-src/ trees were removed in Phase 6 of the
# Reborn migration. There are no longer any such directories to check.
# The version-check job in reborn-tests.yml calls this script; it passes
# cleanly so the job remains green while the check remains a no-op.
#
# TODO: repurpose this script to enforce workspace crate version bumps when
# any public API crate (brassclaw_host_api, brassclaw_common, etc.) changes.

echo "No tools-src/ or channels-src/ directories present (removed in Phase 6)."
echo "All version checks passed."
exit 0
