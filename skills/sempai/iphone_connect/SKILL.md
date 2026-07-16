---
name: sempai_iphone_connect
description: Sempai iPhone connection skill — establishes and monitors the Sempai provider connection from an iPhone via the BrassClaw mobile interface.
types: [sempai]
---

# Sempai iPhone Connect

This skill manages the connection lifecycle between the interceptor service and a Sempai provider accessed from an iPhone. It is activated when the interceptor transitions into or out of **rerouting state** via the mobile interface.

## Connection Lifecycle

```
User taps "Use as Sempai" in provider settings
  → Interceptor receives connection request
  → Sempai provider health-check (GET /ping or equivalent)
  → On success: SharedInterceptorMode → Rerouting
  → On failure: remain in Routing, surface error to UI

User taps "Disconnect Sempai"
  → SharedInterceptorMode → Routing
  → In-flight packets drain (no new rerouting started)
```

## Mobile Interface Contract

The BrassClaw mobile app surfaces:

| Action | Description |
|--------|-------------|
| Provider card → **Use as Sempai** | Activates the selected provider as the Sempai and switches the interceptor to rerouting state |
| Sempai status badge | Shows current mode (Routing / Rerouting) and packet throughput |
| **Disconnect** | Returns the interceptor to routing state immediately |
| **Review queue** | Opens the forensic packet list for manual Sempai review of captured prompts |

## Connection Health

While in rerouting state, the interceptor maintains a keepalive:

- Sends a lightweight `ping` prompt to the Sempai provider every 60 seconds
- If the Sempai fails to respond within the configured timeout (default: 30 s), the interceptor **automatically reverts to routing state** and surfaces a reconnect prompt
- All in-flight rerouting packets complete against the last-known Sempai response or fall back to the original Kohai prompt on timeout

## Settings

| Setting | Default | Description |
|---------|---------|-------------|
| `sempai.keepalive_interval_secs` | `60` | Seconds between health-check pings |
| `sempai.response_timeout_secs` | `30` | Maximum wait for a Sempai response before fallback |
| `sempai.max_concurrent_packets` | `4` | Maximum prompts routed to Sempai simultaneously |
| `sempai.fallback_on_timeout` | `true` | Whether to forward the original Kohai prompt on Sempai timeout |

## Security Notes

- The Sempai provider receives the full assembled Kohai prompt, including system instructions and context. Only configure a trusted provider as the Sempai.
- The interceptor stores forensic packets locally — they are not sent to any third party beyond the configured Sempai provider.
- iPhone connections use the same bearer-token authentication as the WebUI v2 API surface.
