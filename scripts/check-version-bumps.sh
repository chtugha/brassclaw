#!/usr/bin/env bash
set -euo pipefail

# CI script: check that version bumps accompany WIT or extension source changes.
# Exit 0 if all checks pass, exit 1 if any version wasn't bumped.

ERRORS=0
ALLOW_SKIP_VERSION_CHECK="${ALLOW_SKIP_VERSION_CHECK:-true}"
RAW_BASE_BRANCH="${GITHUB_BASE_REF:-main}"
BASE_BRANCH="${RAW_BASE_BRANCH#refs/heads/}"

# Ensure the base branch ref is available for skip checks and diffs.
if ! git rev-parse "origin/${BASE_BRANCH}" >/dev/null 2>&1; then
    echo "Fetching origin/${BASE_BRANCH}..."
    git fetch origin "$BASE_BRANCH" --depth=1
fi

MERGE_BASE=$(git merge-base "origin/${BASE_BRANCH}" HEAD)

# --- Skip mechanism -----------------------------------------------------------

if [[ "${ALLOW_SKIP_VERSION_CHECK}" == "true" ]]; then
    if [[ "${PR_LABELS:-}" == *"skip-version-check"* ]]; then
        echo "skip-version-check label detected — skipping all version checks."
        exit 0
    fi

    # Check commit messages for [skip-version-check]
    if git log "${MERGE_BASE}..HEAD" --pretty=format:"%s %b" 2>/dev/null \
        | grep -qF '[skip-version-check]'; then
        echo "[skip-version-check] found in commit message — skipping all version checks."
        exit 0
    fi
fi

# --- Determine base branch and changed files ----------------------------------

echo "Base branch: $BASE_BRANCH"

CHANGED_FILES=$(git diff --name-only "$MERGE_BASE" HEAD)

if [[ -z "$CHANGED_FILES" ]]; then
    echo "No changed files detected. Nothing to check."
    exit 0
fi

# --- Helper functions ---------------------------------------------------------

# Extract JSON "version" field using jq
extract_json_version() {
    local file="$1"
    if [[ ! -f "$file" ]]; then
        echo ""
        return
    fi
    jq -r '.version // empty' "$file" 2>/dev/null || true
}

# Extract JSON "version" from the base branch copy of a file
extract_json_version_base() {
    local file="$1"
    git show "origin/${BASE_BRANCH}:${file}" 2>/dev/null | jq -r '.version // empty' 2>/dev/null || true
}

# Return 0 if $1 (new) is strictly greater than $2 (old) via sort -V, or old is empty.
version_was_bumped() {
    local new="$1"
    local old="$2"
    if [[ -z "$old" ]]; then
        # No prior version — treat as new, no bump required
        return 0
    fi
    if [[ -z "$new" ]]; then
        # Version was removed — that's a problem
        return 1
    fi
    if [[ "$new" == "$old" ]]; then
        return 1
    fi
    # Check new > old via sort -V
    local highest
    highest=$(printf '%s\n%s\n' "$new" "$old" | sort -V | tail -n1)
    [[ "$highest" == "$new" ]]
}

# --- 1. Tool source changes ---------------------------------------------------

TOOL_NAMES=$(echo "$CHANGED_FILES" | sed -n 's|^tools-src/\([^/]*\)/.*|\1|p' | sort -u)

if [[ -n "$TOOL_NAMES" ]]; then
    echo ""
    echo "=== Tool source changes ==="
fi

for tool in $TOOL_NAMES; do
    REGISTRY_FILE="registry/tools/${tool}.json"
    echo ""
    echo "  --- tools-src/${tool}/ changed ---"

    if [[ ! -f "$REGISTRY_FILE" ]]; then
        echo "  SKIP: ${REGISTRY_FILE} does not exist yet (new extension?)."
        continue
    fi

    NEW_VER=$(extract_json_version "$REGISTRY_FILE")
    OLD_VER=$(extract_json_version_base "$REGISTRY_FILE")

    echo "  Registry version: ${OLD_VER:-<none>} -> ${NEW_VER:-<missing>}"

    if ! version_was_bumped "${NEW_VER}" "${OLD_VER}"; then
        echo "  ERROR: ${REGISTRY_FILE} version was not bumped (${OLD_VER} -> ${NEW_VER:-<missing>}). Bump the version when changing tools-src/${tool}/."
        ERRORS=$((ERRORS + 1))
    else
        echo "  OK: version bumped."
    fi
done

# --- 2. Channel source changes ------------------------------------------------

CHANNEL_NAMES=$(echo "$CHANGED_FILES" | sed -n 's|^channels-src/\([^/]*\)/.*|\1|p' | sort -u)

if [[ -n "$CHANNEL_NAMES" ]]; then
    echo ""
    echo "=== Channel source changes ==="
fi

for channel in $CHANNEL_NAMES; do
    REGISTRY_FILE="registry/channels/${channel}.json"
    echo ""
    echo "  --- channels-src/${channel}/ changed ---"

    if [[ ! -f "$REGISTRY_FILE" ]]; then
        echo "  SKIP: ${REGISTRY_FILE} does not exist yet (new extension?)."
        continue
    fi

    NEW_VER=$(extract_json_version "$REGISTRY_FILE")
    OLD_VER=$(extract_json_version_base "$REGISTRY_FILE")

    echo "  Registry version: ${OLD_VER:-<none>} -> ${NEW_VER:-<missing>}"

    if ! version_was_bumped "${NEW_VER}" "${OLD_VER}"; then
        echo "  ERROR: ${REGISTRY_FILE} version was not bumped (${OLD_VER} -> ${NEW_VER:-<missing>}). Bump the version when changing channels-src/${channel}/."
        ERRORS=$((ERRORS + 1))
    else
        echo "  OK: version bumped."
    fi
done

# --- Summary ------------------------------------------------------------------

echo ""
if [[ $ERRORS -gt 0 ]]; then
    echo "FAILED: ${ERRORS} version check(s) did not pass. See errors above."
    exit 1
else
    echo "All version checks passed."
    exit 0
fi
