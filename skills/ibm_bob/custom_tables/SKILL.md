---
name: ibm_bob_custom_tables
version: "1.0.0"
types: [agent, kohai]
description: >
  IBM Bob Custom Tables API. List, read, create, update, and delete entries in user-defined custom table objects.
activation:
  keywords: ["custom table", "bob table", "custom data", "extended data"]
  tags: ["ibm_bob", "hr", "custom_tables"]
credentials:
  - name: hibob_service_user_token
    provider: hibob
    location:
      type: basic_auth
      username: "{{secret:hibob_service_user_id}}"
    hosts: ["api.hibob.com"]
---

## IBM Bob Custom Tables API

### Authentication

All API calls require HTTP Basic Auth. Construct the `Authorization` header as:

```
Authorization: Basic base64({hibob_service_user_id}:{hibob_service_user_token})
```

Where `hibob_service_user_id` and `hibob_service_user_token` are loaded from the BrassClaw secrets store. **Never** log or expose these values.

**Base URL:** `https://api.hibob.com/v1`

### Rate Limiting

IBM Bob enforces per-endpoint rate limits. If you receive `HTTP 429`, back off and retry after the `Retry-After` header value (or default 60 seconds).

### Error Codes

| Code | Meaning |
|------|---------|
| 401 | Bad credentials — check `hibob_service_user_id` and `hibob_service_user_token` |
| 403 | Missing permission — the service user lacks the required scope |
| 404 | Resource not found |
| 422 | Validation error — check required parameters |
| 429 | Rate limited — back off and retry |

### Response Format

API responses use JSON. List-linked fields (dropdowns, picklists) return both `value` (machine-readable key) and `humanReadable` (display label). Set `humanReadable=true` in requests to receive display labels in responses.

---

### Endpoints

| Tool | Method | Path | Description | Permission | Approval Required |
|------|--------|------|-------------|------------|-------------------|
| `bob_list_custom_tables` | GET | `/objects/table` | List all custom table definitions | `Read custom tables` | No |
| `bob_get_custom_table_entries` | GET | `/objects/table/{tableId}/entries` | Get entries for a custom table | `Read custom tables` | No |
| `bob_create_custom_entry` | POST | `/objects/table/{tableId}/entries` | Create a new entry | `Write custom tables` | **Yes** |
| `bob_update_custom_entry` | PUT | `/objects/table/{tableId}/entries/{entryId}` | Update entry | `Write custom tables` | **Yes** |
| `bob_delete_custom_entry` | DELETE | `/objects/table/{tableId}/entries/{entryId}` | Delete entry | `Write custom tables` | **Yes** |

---

### `bob_list_custom_tables` — GET `/objects/table`

Retrieve all custom table definitions configured in the Bob instance. Use this to discover available `tableId` values before querying entries.

**Response:**

```json
{
  "tables": [
    {
      "id": "certifications",
      "humanReadable": "Certifications",
      "fields": [
        { "name": "certName", "type": "text", "humanReadable": "Certification Name" },
        { "name": "issueDate", "type": "date", "humanReadable": "Issue Date" }
      ]
    }
  ]
}
```

| Response Field | Type | Description |
|----------------|------|-------------|
| `id` | string | Table identifier used in subsequent requests |
| `humanReadable` | string | Display name of the table |
| `fields` | array | Column definitions for the table |

---

### `bob_get_custom_table_entries` — GET `/objects/table/{tableId}/entries`

Retrieve entries from a custom table. Filter by employee using the `employeeId` query parameter.

**Path Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `tableId` | string | Yes | Custom table identifier (from `bob_list_custom_tables`) |

**Query Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `employeeId` | string | No | Filter entries to a specific employee |

**Response:**

```json
{
  "entries": [
    {
      "id": "entry-001",
      "employeeId": "1234567890",
      "certName": "AWS Solutions Architect",
      "issueDate": "2024-05-01"
    }
  ]
}
```

---

### `bob_create_custom_entry` — POST `/objects/table/{tableId}/entries`

Create a new entry in a custom table. ⚠️ **Requires approval before execution.**

**Path Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `tableId` | string | Yes | Custom table identifier |

**Request Body:** An object whose keys match the field names defined for the table.

```json
{
  "employeeId": "1234567890",
  "certName": "Google Cloud Professional",
  "issueDate": "2025-01-10"
}
```

**Response:**

```json
{
  "id": "entry-002"
}
```

---

### `bob_update_custom_entry` — PUT `/objects/table/{tableId}/entries/{entryId}`

Update an existing entry in a custom table. ⚠️ **Requires approval before execution.**

**Path Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `tableId` | string | Yes | Custom table identifier |
| `entryId` | string | Yes | Entry identifier to update |

**Request Body:** Subset of entry fields to update.

```json
{
  "issueDate": "2025-02-01"
}
```

---

### `bob_delete_custom_entry` — DELETE `/objects/table/{tableId}/entries/{entryId}`

Delete an entry from a custom table. ⚠️ **Requires approval before execution.** This action is permanent.

**Path Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `tableId` | string | Yes | Custom table identifier |
| `entryId` | string | Yes | Entry identifier to delete |

**Response:** `204 No Content` on success.
