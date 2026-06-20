---
title: Model Context Protocol (MCP)
description: Connect your agent to Model Context Protocol (MCP) servers
---

BrassClaw connects to [Model Context Protocol](https://modelcontextprotocol.io/) servers, giving your agent access to external tools and data sources without writing custom integrations.

---

## Quickstart

```bash
# Add an HTTP server
brassclaw mcp add notion https://mcp.notion.com/mcp --client-id <your-client-id>
```

```bash
# Add a stdio server (spawns a local process)
brassclaw mcp add docs --transport stdio \
  --command npx --arg @mintlify/mcp --arg=--docs --arg https://docs.brassclaw.com/mcp
```

```bash
# Add a Unix socket server
brassclaw mcp add myserver --transport unix --socket /tmp/mcp.sock
```

```bash
# Test connectivity
brassclaw mcp test notion
```

```bash
# List configured servers
brassclaw mcp list
```

```bash
# Remove a server
brassclaw mcp remove notion
```

```bash
# Toggle a server on/off
brassclaw mcp toggle notion

# Explicitly disable or enable
brassclaw mcp toggle notion --disable
brassclaw mcp toggle notion --enable
```

---

## Transports

| Transport          | Use case                                                          | Example                                                                     |
|--------------------|-------------------------------------------------------------------|-----------------------------------------------------------------------------|
| **HTTP** (default) | **Connects** to a remote server over HTTP(S)                      | `brassclaw mcp add name https://mcp.example.com`                             |
| **stdio**          | **Spawns** a local server and **connects** to it via stdin/stdout | `brassclaw mcp add docs --transport stdio --command npx --arg @mintlify/mcp` |
| **Unix**           | **Connects** to a server on a Unix domain socket                  | `brassclaw mcp add name --transport unix --socket /tmp/mcp.sock`             |

### HTTP with OAuth

Many hosted MCP servers require OAuth 2.1 authentication. BrassClaw implements the [MCP Authorization spec](https://spec.modelcontextprotocol.io/specification/2025-03-26/basic/authorization/) with PKCE:

```bash
# Add with OAuth credentials
brassclaw mcp add notion https://mcp.notion.com/mcp \
  --client-id YOUR_CLIENT_ID \
  --scopes "read,write"

# Authenticate (opens browser for consent)
brassclaw mcp auth notion
```

OAuth tokens are stored securely via BrassClaw's secrets store and refreshed automatically.

### stdio with Environment Variables

Stdio servers often need API keys or configuration via environment variables:

```bash
brassclaw mcp add docs --transport stdio \
  --command npx --arg @mintlify/mcp \
  --env MINTLIFY_API_KEY=your_api_key
```

---

## Configuration File

Server configs are stored in `~/.brassclaw/mcp-servers.json`:

```json
{
  "schema_version": 1,
  "servers": [
    {
      "name": "docs",
      "url": "",
      "transport": {
        "transport": "stdio",
        "command": "npx",
        "args": ["@mintlify/mcp", "--docs", "https://docs.brassclaw.com/mcp"]
      },
      "enabled": true,
      "description": "BrassClaw docs search"
    },
    {
      "name": "notion",
      "url": "https://mcp.notion.com/mcp",
      "oauth": {
        "client_id": "your-client-id",
        "scopes": ["read", "write"]
      },
      "enabled": true
    }
  ]
}
```

You can edit this file directly, or use `brassclaw mcp add` / `brassclaw mcp remove` to manage it.

---

## Custom Headers

For servers that use API key authentication instead of OAuth:

```bash
brassclaw mcp add myapi https://api.example.com/mcp \
  --header "Authorization:Bearer sk-your-key" \
  --header "X-Custom:value"
```

---

## Built-In Servers

BrassClaw ships with a built-in registry of hosted MCP servers. A few examples:

| Server                                         | Transport | What it does                              |
|------------------------------------------------|-----------|-------------------------------------------|
| [Asana](https://mcp.asana.com/v2/mcp)          | HTTP      | Task management, projects, and team coordination  |
| [Cloudflare](https://mcp.cloudflare.com/mcp)   | HTTP      | DNS, Workers, KV, and infrastructure management   |
| [Intercom](https://mcp.intercom.com/mcp)       | HTTP      | Customer messaging, support, and engagement       |
| [Linear](https://mcp.linear.app/sse)           | HTTP      | Issue tracking and project management             |
| [NEAR AI](https://private.near.ai/mcp)         | HTTP      | Built-in tools like web search                    |
| [Notion](https://mcp.notion.com/mcp)           | HTTP      | Pages, databases, and comments                    |
| [Sentry](https://mcp.sentry.dev/mcp)           | HTTP      | Error tracking and performance monitoring         |
| [Stripe](https://mcp.stripe.com)               | HTTP      | Payments, subscriptions, and invoices             |

Browse more servers at:
- [MCP Server Registry](https://github.com/modelcontextprotocol/servers) (official)
- [awesome-mcp-servers](https://github.com/punkpeye/awesome-mcp-servers) (community)

---

## Troubleshooting

```bash
# Check server health
brassclaw mcp test <server-name>

# Re-authenticate an OAuth server
brassclaw mcp auth <server-name>

# Disable without removing
brassclaw mcp toggle <server-name> --disable

# Debug logging
RUST_LOG=brassclaw::tools::mcp=debug brassclaw mcp test <server-name>
```
