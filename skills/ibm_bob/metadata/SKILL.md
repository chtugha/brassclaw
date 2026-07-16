---
name: ibm_bob_metadata
version: "1.0.0"
types: [agent, llm]
description: >
  IBM Bob Metadata API. Read and manage employee field definitions and picklist configurations.
activation:
  keywords: ["fields", "metadata", "lists", "picklist", "bob fields"]
  tags: ["ibm_bob", "hr", "metadata"]
credentials:
  - name: hibob_service_user_token
    provider: hibob
    location:
      type: basic_auth
      username: "{{secret:hibob_service_user_id}}"
    hosts: ["api.hibob.com"]
---

## IBM Bob Metadata API

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
| `bob_get_fields` | GET | `/metadata/objects/employee/fields` | All employee field definitions | `Read metadata` | No |
| `bob_get_lists` | GET | `/metadata/lists` | All picklist definitions | `Read metadata` | No |
| `bob_get_list_by_name` | GET | `/metadata/lists/{name}` | A specific picklist's items | `Read metadata` | No |
| `bob_create_field` | POST | `/metadata/objects/employee/fields` | Create custom field | `Write metadata` | **Yes** |
| `bob_update_field` | PUT | `/metadata/objects/employee/fields/{fieldName}` | Update field definition | `Write metadata` | **Yes** |
| `bob_add_list_item` | POST | `/metadata/lists/{name}/items` | Add item to picklist | `Write metadata` | **Yes** |

---

### `bob_get_fields` — GET `/metadata/objects/employee/fields`

Retrieve all employee field definitions, including built-in and custom fields. Useful for discovering available field paths before constructing search filters or update payloads.

**Response:**

```json
{
  "fields": [
    {
      "name": "work.department",
      "type": "list",
      "category": "Work",
      "humanReadable": "Department",
      "historical": false
    },
    {
      "name": "personal.dateOfBirth",
      "type": "date",
      "category": "Personal",
      "humanReadable": "Date of Birth",
      "historical": false
    }
  ]
}
```

| Response Field | Type | Description |
|----------------|------|-------------|
| `name` | string | Dot-path field identifier used in API requests |
| `type` | string | Data type: `text`, `date`, `list`, `number`, `bool`, etc. |
| `category` | string | Grouping category (e.g. `Work`, `Personal`, `Home`) |
| `humanReadable` | string | Display label shown in the Bob UI |
| `historical` | boolean | Whether the field supports historical versioning |

---

### `bob_get_lists` — GET `/metadata/lists`

Retrieve definitions for all picklists (dropdown fields) configured in the Bob instance.

**Response:**

```json
{
  "lists": [
    {
      "name": "department",
      "humanReadable": "Department"
    },
    {
      "name": "site",
      "humanReadable": "Site"
    }
  ]
}
```

---

### `bob_get_list_by_name` — GET `/metadata/lists/{name}`

Retrieve all items for a specific picklist by its internal name. Use `bob_get_lists` first to discover available list names.

**Path Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name` | string | Yes | Internal list identifier (e.g. `department`, `site`) |

**Response:**

```json
{
  "name": "department",
  "humanReadable": "Department",
  "items": [
    { "value": "engineering", "humanReadable": "Engineering" },
    { "value": "product", "humanReadable": "Product" }
  ]
}
```

---

### `bob_create_field` — POST `/metadata/objects/employee/fields`

Create a new custom employee field. ⚠️ **Requires approval before execution.** Field creation is irreversible in many configurations — confirm field name and type carefully.

**Request Body:**

```json
{
  "name": "customField1",
  "type": "text",
  "category": "Work",
  "humanReadable": "Project Code"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Internal field identifier (must be unique) |
| `type` | string | Yes | Data type: `text`, `date`, `list`, `number`, `bool` |
| `category` | string | Yes | Field category / section |
| `humanReadable` | string | Yes | Display label |

---

### `bob_update_field` — PUT `/metadata/objects/employee/fields/{fieldName}`

Update the definition of an existing employee field. ⚠️ **Requires approval before execution.**

**Path Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `fieldName` | string | Yes | Internal field name (dot-path identifier) |

**Request Body:** Subset of field definition properties to update (e.g. `humanReadable`, `category`).

---

### `bob_add_list_item` — POST `/metadata/lists/{name}/items`

Add a new item to an existing picklist. ⚠️ **Requires approval before execution.**

**Path Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name` | string | Yes | Internal list identifier |

**Request Body:**

```json
{
  "name": "data-platform",
  "value": "Data Platform"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Machine-readable key for the new item |
| `value` | string | Yes | Display label for the new item |
