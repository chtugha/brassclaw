---
name: ibm_bob_webhooks
description: Register, inspect, and manage IBM Bob webhook subscriptions for HR event notifications.
types: [ibm_bob]
---

# IBM Bob – Webhooks

Use this skill when the user asks about subscribing to HR events, configuring IBM Bob webhooks, webhook delivery logs, event types, or debugging webhook failures.

## Capabilities

- List all configured webhook subscriptions
- Create a new webhook subscription for one or more event types
- Update an existing webhook (URL, secret, event types)
- Delete a webhook subscription
- List available event types
- Inspect webhook delivery history and retry failed deliveries

## API Patterns

Base path: `/api/v1/webhooks`

### List webhook subscriptions
```
GET /api/v1/webhooks
```

### List available event types
```
GET /api/v1/webhooks/event-types
```

### Create a webhook
```
POST /api/v1/webhooks
Body: {
  "url": "https://...",
  "events": ["employee.created", "employee.updated", "timeoff.approved"],
  "secret": "...",
  "active": true
}
```

### Update a webhook
```
PUT /api/v1/webhooks/{webhookId}
Body: { "url": "...", "events": [...], "active": true }
```

### Delete a webhook
```
DELETE /api/v1/webhooks/{webhookId}
```

### Get delivery history
```
GET /api/v1/webhooks/{webhookId}/deliveries?status={success|failed}&limit=50
```

### Retry a failed delivery
```
POST /api/v1/webhooks/{webhookId}/deliveries/{deliveryId}/retry
```

## Notes

- Webhook payloads are signed with HMAC-SHA256 using the subscription `secret`; always verify signatures on the receiving end.
- IBM Bob retries failed deliveries up to 3 times with exponential back-off (1 min, 5 min, 30 min).
- Subscriptions inactive for 90 days are automatically deactivated; renew via the `active: true` update.
- Use the `test` endpoint (`POST /api/v1/webhooks/{id}/test`) to send a synthetic payload for verification.
