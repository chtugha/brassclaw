#!/bin/bash
export BRASSCLAW_REBORN_PROFILE=local-dev
export BRASSCLAW_REBORN_WEBUI_TOKEN=test-playwright-token
export BRASSCLAW_REBORN_WEBUI_USER_ID=test-playwright-user
export BRASSCLAW_GATEWAY_TOKEN=test-token

cd ../..
exec cargo run --bin brassclaw --release --features libsql -- serve --host 127.0.0.1 --port 3000

# Made with Bob
