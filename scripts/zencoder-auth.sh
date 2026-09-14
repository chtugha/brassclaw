#!/usr/bin/env bash
# zencoder-auth.sh — obtain a Zencoder/Zenflow JWT and print it to stdout.
#
# Usage:
#   bash scripts/zencoder-auth.sh
#
# After running, copy the JWT from the output and store it:
#   brassclaw secret set zencoder_access_token <jwt>
#
# The token is valid for ~24 hours.  Re-run this script when you see a 401.
#
# Requirements:
#   - curl
#   - ZENCODER_EMAIL and ZENCODER_PASSWORD environment variables set, OR
#     the script will prompt interactively.

set -euo pipefail

AUTH_URL="https://auth.zencoder.ai/auth/realms/zencoder/protocol/openid-connect/token"
CLIENT_ID="zencoder-public"

# Resolve credentials — env vars take precedence; fall back to interactive prompt.
if [[ -z "${ZENCODER_EMAIL:-}" ]]; then
    read -rp "Zencoder email: " ZENCODER_EMAIL
fi

if [[ -z "${ZENCODER_PASSWORD:-}" ]]; then
    read -rsp "Zencoder password: " ZENCODER_PASSWORD
    echo
fi

RESPONSE=$(curl -s -X POST "$AUTH_URL" \
    -H "Content-Type: application/x-www-form-urlencoded" \
    -d "client_id=${CLIENT_ID}" \
    -d "grant_type=password" \
    -d "username=${ZENCODER_EMAIL}" \
    -d "password=${ZENCODER_PASSWORD}")

# Extract access_token using sed (no jq dependency required).
TOKEN=$(printf '%s' "$RESPONSE" | sed -n 's/.*"access_token":"\([^"]*\)".*/\1/p')

if [[ -z "$TOKEN" ]]; then
    echo "ERROR: Failed to obtain token. Response was:" >&2
    echo "$RESPONSE" >&2
    exit 1
fi

echo ""
echo "=== Zencoder JWT ==="
echo "$TOKEN"
echo "===================="
echo ""
echo "Run the following command to store it in BrassClaw:"
echo "  brassclaw secret set zencoder_access_token $TOKEN"
