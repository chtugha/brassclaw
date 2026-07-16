---
name: ibm_bob_people
version: "1.0.0"
types: [agent, kohai]
description: >
  IBM Bob People API. Manage employee records including search, create, update, terminate, and invite operations.
activation:
  keywords: ["employee", "people", "staff", "hire", "terminate", "bob employee"]
  tags: ["ibm_bob", "hr", "people"]
credentials:
  - name: hibob_service_user_token
    provider: hibob
    location:
      type: basic_auth
      username: "{{secret:hibob_service_user_id}}"
    hosts: ["api.hibob.com"]
---

## IBM Bob People API

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
| `bob_search_employees` | POST | `/people/search` | Search employees using filters | `Read employee data` | No |
| `bob_get_employee` | GET | `/people/{id}` | Get a single employee by id or email | `Read employee data` | No |
| `bob_create_employee` | POST | `/people` | Create new employee | `Write employee data` | **Yes** |
| `bob_update_employee` | PUT | `/people/{id}` | Update employee fields | `Write employee data` | **Yes** |
| `bob_terminate_employee` | POST | `/people/{id}/employment/terminate` | Terminate employment | `Write employee data` | **Yes** |
| `bob_invite_employee` | POST | `/people/{id}/invite` | Send onboarding invite | `Write employee data` | **Yes** |
| `bob_get_avatar` | GET | `/avatars/{id}` | Get employee avatar URL | `Read employee data` | No |

---

### `bob_search_employees` — POST `/people/search`

Search for employees using structured filters. Supports pagination and field selection.

**Request Body:**

```json
{
  "filters": [
    {
      "fieldPath": "work.department",
      "operator": "equals",
      "values": ["Engineering"]
    }
  ],
  "fields": ["firstName", "lastName", "email", "work.department"],
  "humanReadable": true
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `filters` | array | No | List of filter objects. Each filter has `fieldPath`, `operator`, and `values`. |
| `fields` | array | No | List of dot-path fields to include in response. Omit for all fields. |
| `humanReadable` | boolean | No | Set `true` to receive display labels alongside machine keys. |

**Response:**

```json
{
  "employees": [
    {
      "id": "1234567890",
      "firstName": "Jane",
      "lastName": "Doe",
      "displayName": "Jane Doe",
      "email": "jane.doe@example.com",
      "department": "Engineering"
    }
  ]
}
```

---

### `bob_get_employee` — GET `/people/{id}`

Retrieve a single employee record by their Bob employee ID or email address.

**Path Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id` | string | Yes | Employee ID or email address |

**Query Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `humanReadable` | boolean | No | Set `true` to include display labels |

**Response:** Full employee object including all profile, work, personal, and custom fields.

---

### `bob_create_employee` — POST `/people`

Create a new employee record. ⚠️ **Requires approval before execution.**

**Request Body:**

```json
{
  "firstName": "John",
  "lastName": "Smith",
  "email": "john.smith@example.com",
  "site": "New York",
  "department": "Engineering",
  "startDate": "2025-01-15"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `firstName` | string | Yes | Employee first name |
| `lastName` | string | Yes | Employee last name |
| `email` | string | Yes | Work email address |
| `site` | string | Yes | Office site / location |
| `department` | string | No | Department name |
| `startDate` | string | No | ISO 8601 date (`YYYY-MM-DD`) |

**Response:**

```json
{
  "id": "9876543210"
}
```

---

### `bob_update_employee` — PUT `/people/{id}`

Update one or more fields on an existing employee record. ⚠️ **Requires approval before execution.**

**Path Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id` | string | Yes | Employee ID |

**Request Body:** Any subset of employee fields (same schema as create). Only supplied fields are updated.

---

### `bob_terminate_employee` — POST `/people/{id}/employment/terminate`

Terminate an employee's employment. ⚠️ **Requires approval before execution.**

**Path Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id` | string | Yes | Employee ID |

**Request Body:**

```json
{
  "lastDayOfWork": "2025-03-31",
  "reasonType": "Resigned",
  "reason": "Accepted external offer"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `lastDayOfWork` | string | Yes | ISO 8601 date of last working day |
| `reasonType` | string | Yes | Termination category (e.g. `Resigned`, `Dismissed`, `Retired`) |
| `reason` | string | No | Free-text explanation |

---

### `bob_invite_employee` — POST `/people/{id}/invite`

Send an onboarding invite to an employee. ⚠️ **Requires approval before execution.**

**Path Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id` | string | Yes | Employee ID |

**Request Body:** Empty body `{}` or omit — invite is sent to the employee's registered email.

---

### `bob_get_avatar` — GET `/avatars/{id}`

Retrieve the avatar image URL for an employee.

**Path Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id` | string | Yes | Employee ID |

**Response:**

```json
{
  "url": "https://images.hibob.com/avatars/1234567890.jpg"
}
```
