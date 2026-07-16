---
name: ibm_bob_timeoff
version: "1.0.0"
types: [agent, kohai]
description: >
  IBM Bob Time-Off API. Submit, retrieve, and cancel time-off requests; query who is out; manage balances, adjustments, policies, and calendar events.
activation:
  keywords: ["time off", "vacation", "leave", "absence", "out of office", "PTO", "balance", "policy"]
  tags: ["ibm_bob", "hr", "timeoff"]
credentials:
  - name: hibob_service_user_token
    provider: hibob
    location:
      type: basic_auth
      username: "{{secret:hibob_service_user_id}}"
    hosts: ["api.hibob.com"]
---

## IBM Bob Time-Off API

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
| `bob_submit_timeoff` | POST | `/timeoff/requests` | Submit a time-off request | `Write time-off` | **Yes** |
| `bob_get_timeoff_request` | GET | `/timeoff/requests/{requestId}` | Get time-off request details | `Read time-off` | No |
| `bob_cancel_timeoff` | DELETE | `/timeoff/requests/{requestId}` | Cancel a time-off request | `Write time-off` | **Yes** |
| `bob_get_timeoff_changes` | GET | `/timeoff/changes` | Changes since a date | `Read time-off` | No |
| `bob_whos_out` | GET | `/timeoff/whosout` | Who is out in a date range | `Read time-off` | No |
| `bob_whos_out_today` | GET | `/timeoff/outtoday` | Who is out today | `Read time-off` | No |
| `bob_get_balance` | GET | `/timeoff/employees/{employeeId}/balances` | Employee time-off balances | `Read time-off balance` | No |
| `bob_create_balance_adjustment` | POST | `/timeoff/employees/{employeeId}/adjustments` | Adjust time-off balance | `Write time-off balance` | **Yes** |
| `bob_get_policy_types` | GET | `/timeoff/policy-types` | List all time-off policy types | `Read time-off` | No |
| `bob_get_policies` | GET | `/timeoff/employees/{employeeId}/policies` | Employee's time-off policies | `Read time-off` | No |
| `bob_search_calendar_events` | GET | `/timeoff/calendar/events` | Search calendar events | `Read time-off` | No |

---

### `bob_submit_timeoff` — POST `/timeoff/requests`

Submit a time-off request on behalf of an employee. ⚠️ **Requires approval before execution.**

**Request Body:**

```json
{
  "employeeId": "1234567890",
  "policyType": "annual_leave",
  "startDate": "2025-08-01",
  "endDate": "2025-08-08",
  "startPortion": "all_day",
  "endPortion": "all_day",
  "description": "Summer holiday"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `employeeId` | string | Yes | Bob employee ID |
| `policyType` | string | Yes | Policy type identifier (from `bob_get_policy_types`) |
| `startDate` | string | Yes | ISO 8601 start date (`YYYY-MM-DD`) |
| `endDate` | string | Yes | ISO 8601 end date (`YYYY-MM-DD`) |
| `startPortion` | string | No | `all_day`, `morning`, or `afternoon` |
| `endPortion` | string | No | `all_day`, `morning`, or `afternoon` |
| `description` | string | No | Optional free-text note |

**Response:**

```json
{
  "id": "req-00456"
}
```

---

### `bob_get_timeoff_request` — GET `/timeoff/requests/{requestId}`

Retrieve details of a specific time-off request.

**Path Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `requestId` | string | Yes | Time-off request identifier |

**Response:**

```json
{
  "id": "req-00456",
  "employeeId": "1234567890",
  "policyType": "annual_leave",
  "startDate": "2025-08-01",
  "endDate": "2025-08-08",
  "status": "approved",
  "description": "Summer holiday"
}
```

---

### `bob_cancel_timeoff` — DELETE `/timeoff/requests/{requestId}`

Cancel an existing time-off request. ⚠️ **Requires approval before execution.**

**Path Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `requestId` | string | Yes | Time-off request identifier |

**Response:** `200 OK` on success.

---

### `bob_get_timeoff_changes` — GET `/timeoff/changes`

Retrieve all time-off changes (new requests, approvals, cancellations) since a specified date. Useful for syncing external calendars or audit logs.

**Query Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `since` | string | Yes | ISO 8601 datetime — return changes after this point |

**Response:**

```json
{
  "changes": [
    {
      "requestId": "req-00456",
      "changeType": "approved",
      "changedAt": "2025-07-10T09:30:00Z"
    }
  ]
}
```

---

### `bob_whos_out` — GET `/timeoff/whosout`

Query which employees are out (on approved leave) within a date range.

**Query Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `from` | string | Yes | ISO 8601 start date |
| `to` | string | Yes | ISO 8601 end date |
| `includeHourly` | boolean | No | Include hourly/partial-day absences |
| `includePrivate` | boolean | No | Include private/confidential requests |

**Response:**

```json
{
  "outs": [
    {
      "employeeId": "1234567890",
      "displayName": "Jane Doe",
      "startDate": "2025-08-01",
      "endDate": "2025-08-08",
      "policyType": "annual_leave"
    }
  ]
}
```

---

### `bob_whos_out_today` — GET `/timeoff/outtoday`

Retrieve the list of employees who are on approved leave today.

**Response:**

```json
{
  "outs": [
    {
      "employeeId": "1234567890",
      "displayName": "Jane Doe",
      "policyType": "annual_leave"
    }
  ]
}
```

---

### `bob_get_balance` — GET `/timeoff/employees/{employeeId}/balances`

Retrieve time-off balances for an employee, optionally filtered to a specific policy type.

**Path Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `employeeId` | string | Yes | Bob employee ID |

**Query Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `policyType` | string | No | Filter to a specific policy type |

**Response:**

```json
{
  "balances": [
    {
      "policyType": "annual_leave",
      "balance": 12.5,
      "unit": "days"
    }
  ]
}
```

---

### `bob_create_balance_adjustment` — POST `/timeoff/employees/{employeeId}/adjustments`

Manually adjust an employee's time-off balance. ⚠️ **Requires approval before execution.**

**Path Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `employeeId` | string | Yes | Bob employee ID |

**Request Body:**

```json
{
  "policyType": "annual_leave",
  "adjustmentType": "add",
  "effectiveDate": "2025-07-01",
  "amount": 2.0,
  "reason": "Carry-over from previous year"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `policyType` | string | Yes | Policy type to adjust |
| `adjustmentType` | string | Yes | `add` or `subtract` |
| `effectiveDate` | string | Yes | ISO 8601 date the adjustment takes effect |
| `amount` | number | Yes | Number of days/hours to add or subtract |
| `reason` | string | No | Explanation for the adjustment |

---

### `bob_get_policy_types` — GET `/timeoff/policy-types`

List all time-off policy types configured in the Bob instance (e.g. Annual Leave, Sick Leave, Parental Leave).

**Response:**

```json
{
  "policyTypes": [
    {
      "policyType": "annual_leave",
      "name": "Annual Leave",
      "unit": "days"
    }
  ]
}
```

---

### `bob_get_policies` — GET `/timeoff/employees/{employeeId}/policies`

Retrieve the time-off policies assigned to a specific employee.

**Path Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `employeeId` | string | Yes | Bob employee ID |

**Response:**

```json
{
  "policies": [
    {
      "policyType": "annual_leave",
      "name": "Annual Leave",
      "accrualMethod": "monthly",
      "unit": "days"
    }
  ]
}
```

---

### `bob_search_calendar_events` — GET `/timeoff/calendar/events`

Search for calendar events (approved absences) within a date range, optionally filtered to a specific employee.

**Query Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `from` | string | Yes | ISO 8601 start date |
| `to` | string | Yes | ISO 8601 end date |
| `employeeId` | string | No | Restrict results to a single employee |

**Response:**

```json
{
  "events": [
    {
      "employeeId": "1234567890",
      "displayName": "Jane Doe",
      "startDate": "2025-08-01",
      "endDate": "2025-08-08",
      "policyType": "annual_leave",
      "status": "approved"
    }
  ]
}
```
