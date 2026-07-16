---
name: ibm_bob_attendance
version: "1.0.0"
types: [agent, kohai]
description: >
  IBM Bob Attendance API. Import punches, fetch summaries and daily breakdowns, search entries, create or update entries, clock in/out, and delete entries.
activation:
  keywords: ["attendance", "clock in", "clock out", "punch", "hours", "time tracking"]
  tags: ["ibm_bob", "hr", "attendance"]
credentials:
  - name: hibob_service_user_token
    provider: hibob
    location:
      type: basic_auth
      username: "{{secret:hibob_service_user_id}}"
    hosts: ["api.hibob.com"]
---

## IBM Bob Attendance API

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
| `bob_import_punches` | POST | `/attendance/importPunches` | Import attendance punches | `Write attendance` | **Yes** |
| `bob_fetch_summaries` | POST | `/attendance/reports/summaries` | Fetch attendance summaries | `Read attendance` | No |
| `bob_fetch_daily_breakdown` | POST | `/attendance/reports/daily` | Fetch daily breakdown | `Read attendance` | No |
| `bob_search_entries` | POST | `/attendance/search/entries` | Search attendance entries | `Read attendance` | No |
| `bob_create_entries` | POST | `/attendance/entries` | Create attendance entries | `Write attendance` | **Yes** |
| `bob_update_entries` | PUT | `/attendance/entries` | Update attendance entries | `Write attendance` | **Yes** |
| `bob_clock_in` | POST | `/attendance/clock-in` | Clock in an employee | `Write attendance` | **Yes** |
| `bob_clock_out` | POST | `/attendance/clock-out` | Clock out an employee | `Write attendance` | **Yes** |
| `bob_delete_entry` | DELETE | `/attendance/entries/{entryId}` | Delete attendance entry | `Write attendance` | **Yes** |

---

### `bob_import_punches` — POST `/attendance/importPunches`

Bulk-import raw attendance punches (clock-in / clock-out events) from an external time-tracking system. ⚠️ **Requires approval before execution.**

**Request Body:** Array of punch objects.

```json
[
  {
    "dateTime": "2025-07-14T09:00:00Z",
    "type": "clock-in",
    "employeeId": "1234567890"
  },
  {
    "dateTime": "2025-07-14T17:30:00Z",
    "type": "clock-out",
    "employeeId": "1234567890"
  }
]
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `dateTime` | string | Yes | ISO 8601 datetime of the punch |
| `type` | string | Yes | `clock-in` or `clock-out` |
| `employeeId` | string | Yes | Bob employee ID |

**Response:**

```json
{
  "imported": 2,
  "errors": []
}
```

---

### `bob_fetch_summaries` — POST `/attendance/reports/summaries`

Fetch aggregated attendance summaries (total hours, overtime, etc.) for a set of employees over a date range.

**Request Body:**

```json
{
  "fromDate": "2025-07-01",
  "toDate": "2025-07-31",
  "employeeIds": ["1234567890", "0987654321"]
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `fromDate` | string | Yes | ISO 8601 start date |
| `toDate` | string | Yes | ISO 8601 end date |
| `employeeIds` | array | No | Limit results to these employee IDs; omit for all employees |

**Response:**

```json
{
  "summaries": [
    {
      "employeeId": "1234567890",
      "totalHours": 160.5,
      "overtimeHours": 8.0,
      "absenceHours": 0.0
    }
  ]
}
```

---

### `bob_fetch_daily_breakdown` — POST `/attendance/reports/daily`

Fetch a day-by-day attendance breakdown for a set of employees. Useful for detailed auditing or payroll reconciliation.

**Request Body:**

```json
{
  "fromDate": "2025-07-01",
  "toDate": "2025-07-07",
  "employeeIds": ["1234567890"]
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `fromDate` | string | Yes | ISO 8601 start date |
| `toDate` | string | Yes | ISO 8601 end date |
| `employeeIds` | array | No | Limit results to these employee IDs; omit for all employees |

**Response:**

```json
{
  "days": [
    {
      "employeeId": "1234567890",
      "date": "2025-07-01",
      "clockIn": "2025-07-01T09:00:00Z",
      "clockOut": "2025-07-01T17:30:00Z",
      "totalHours": 8.5
    }
  ]
}
```

---

### `bob_search_entries` — POST `/attendance/search/entries`

Search attendance entries for a specific employee within a date range.

**Request Body:**

```json
{
  "from": "2025-07-01",
  "to": "2025-07-31",
  "employeeId": "1234567890"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `from` | string | Yes | ISO 8601 start date |
| `to` | string | Yes | ISO 8601 end date |
| `employeeId` | string | Yes | Bob employee ID |

**Response:**

```json
{
  "entries": [
    {
      "id": "entry-001",
      "employeeId": "1234567890",
      "date": "2025-07-01",
      "clockIn": "2025-07-01T09:00:00Z",
      "clockOut": "2025-07-01T17:30:00Z"
    }
  ]
}
```

---

### `bob_create_entries` — POST `/attendance/entries`

Create one or more attendance entries directly (as opposed to raw punch import). ⚠️ **Requires approval before execution.**

**Request Body:** Array of entry objects.

```json
[
  {
    "employeeId": "1234567890",
    "date": "2025-07-14",
    "clockIn": "2025-07-14T09:00:00Z",
    "clockOut": "2025-07-14T17:30:00Z"
  }
]
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `employeeId` | string | Yes | Bob employee ID |
| `date` | string | Yes | ISO 8601 date of the entry |
| `clockIn` | string | Yes | ISO 8601 datetime of clock-in |
| `clockOut` | string | No | ISO 8601 datetime of clock-out |

**Response:**

```json
{
  "created": 1,
  "ids": ["entry-042"]
}
```

---

### `bob_update_entries` — PUT `/attendance/entries`

Update one or more existing attendance entries. ⚠️ **Requires approval before execution.**

**Request Body:** Array of entry objects, each including its `id`.

```json
[
  {
    "id": "entry-042",
    "clockOut": "2025-07-14T18:00:00Z"
  }
]
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Entry identifier to update |
| `clockIn` | string | No | Updated clock-in datetime |
| `clockOut` | string | No | Updated clock-out datetime |

**Response:**

```json
{
  "updated": 1
}
```

---

### `bob_clock_in` — POST `/attendance/clock-in`

Clock in a specific employee at the given datetime. ⚠️ **Requires approval before execution.**

**Request Body:**

```json
{
  "id": "1234567890",
  "dateTime": "2025-07-15T08:55:00Z"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Bob employee ID |
| `dateTime` | string | Yes | ISO 8601 datetime of clock-in event |

**Response:** `200 OK` with the created punch record.

---

### `bob_clock_out` — POST `/attendance/clock-out`

Clock out a specific employee at the given datetime. ⚠️ **Requires approval before execution.**

**Request Body:**

```json
{
  "id": "1234567890",
  "dateTime": "2025-07-15T17:45:00Z"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Bob employee ID |
| `dateTime` | string | Yes | ISO 8601 datetime of clock-out event |

**Response:** `200 OK` with the created punch record.

---

### `bob_delete_entry` — DELETE `/attendance/entries/{entryId}`

Permanently delete an attendance entry. ⚠️ **Requires approval before execution.** This action cannot be undone.

**Path Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `entryId` | string | Yes | Attendance entry identifier |

**Response:** `204 No Content` on success.
