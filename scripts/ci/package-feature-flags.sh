#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "usage: $0 <package>" >&2
  exit 2
fi

case "$1" in
  brassclaw_reborn_cli)
    printf '%s\n' "--features slack-v2-host-beta"
    ;;
  brassclaw_product_adapters)
    printf '%s\n' "--features test-support,host-auth-mint"
    ;;
  brassclaw_product_workflow)
    printf '%s\n' "--features test-support"
    ;;
  brassclaw_product_workflow_storage)
    printf '%s\n' "--features libsql"
    ;;
  brassclaw_reborn_composition)
    printf '%s\n' "--features test-support,slack-v2-host-beta,libsql"
    ;;
  brassclaw_reborn)
    printf '%s\n' "--features root-llm-provider,libsql-secrets,libsql-restart-tests,webui-user-store"
    ;;
  brassclaw_reborn_event_store)
    printf '%s\n' "--features libsql"
    ;;
  brassclaw_reborn_webui_ingress)
    printf '%s\n' "--features dev-in-memory-session"
    ;;
  *)
    ;;
esac
