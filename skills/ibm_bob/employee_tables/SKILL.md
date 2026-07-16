---
name: ibm_bob_employee_tables
version: "1.0.0"
types: [agent, kohai]
description: >
  IBM Bob Employee Tables API. Read structured per-employee history tables including work history, lifecycle events, salary, equity, variable pay, training, and bank accounts.
activation:
  keywords: ["work history", "lifecycle", "employment history", "salary", "equity", "variable pay", "training", "bank account"]
  tags: ["ibm_bob", "hr", "employee_tables"]
credentials:
  - name: hibob_service_user_token
    provider: hibob
    location:
      type: basic_auth
      username: "{{secret:hibob_service_user_id}}"
    hosts: ["api.hibob.com"]
---

## IBM Bob Employee Tables API

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
| `bob_get_work_history` | GET | `/people/{id}/employment/history/work` | Work history entries | `Read employee data` | No |
| `bob_get_lifecycle` | GET | `/people/{id}/lifecycle` | Lifecycle events | `Read employee data` | No |
| `bob_get_employment_history` | GET | `/people/{id}/employment/history` | Employment history entries | `Read employee data` | No |
| `bob_get_salary_history` | GET | `/people/{id}/salaries` | Salary history | `Read salary data` | No |
| `bob_get_equity_grants` | GET | `/people/{id}/equities/grants` | Equity grant history | `Read equity data` | No |
| `bob_get_variable_pay` | GET | `/people/{id}/variable/payments` | Variable pay history | `Read variable pay data` | No |
| `bob_get_training` | GET | `/people/{id}/training/records` | Training records | `Read employee data` | No |
| `bob_get_bank_accounts` | GET | `/people/{id}/bankAccounts` | Bank account records | `Read bank account data` | No |

---

### `bob_get_work_history` — GET `/people/{id}/employment/history/work`

Retrieve all work history entries for an employee, including role changes, title changes, and department moves.

**Path Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id` | string | Yes | Employee ID |

**Response:**

```json
{
  "values": [
    {
      "id": "1",
      "effectiveDate": "2023-01-01",
      "title": "Senior Engineer",
      "department": "Engineering",
      "reportsTo": { "id": "999", "displayName": "Alice Manager" }
    }
  ]
}
```

---

### `bob_get_lifecycle` — GET `/people/{id}/lifecycle`

Retrieve lifecycle events for an employee (e.g. hire, leave, return from leave, termination).

**Path Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id` | string | Yes | Employee ID |

**Response:**

```json
{
  "values": [
    {
      "id": "1",
      "type": "hired",
      "effectiveDate": "2022-03-01"
    }
  ]
}
```

---

### `bob_get_employment_history` — GET `/people/{id}/employment/history`

Retrieve all employment history entries including contract-type and tenure changes.

**Path Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id` | string | Yes | Employee ID |

**Response:**

```json
{
  "values": [
    {
      "id": "1",
      "effectiveDate": "2022-03-01",
      "contract": "Full-Time",
      "salaryPayType": "Monthly"
    }
  ]
}
```

---

### `bob_get_salary_history` — GET `/people/{id}/salaries`

Retrieve the full salary history for an employee. Requires elevated `Read salary data` permission.

**Path Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id` | string | Yes | Employee ID |

**Response:**

```json
{
  "values": [
    {
      "id": "1",
      "effectiveDate": "2024-01-01",
      "base": { "value": 120000, "currency": "USD", "period": "Annual" }
    }
  ]
}
```

---

### `bob_get_equity_grants` — GET `/people/{id}/equities/grants`

Retrieve equity grant history for an employee. Requires `Read equity data` permission.

**Path Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id` | string | Yes | Employee ID |

**Response:**

```json
{
  "values": [
    {
      "id": "1",
      "grantDate": "2023-01-01",
      "vestingDate": "2027-01-01",
      "quantity": 1000,
      "type": "RSU"
    }
  ]
}
```

---

### `bob_get_variable_pay` — GET `/people/{id}/variable/payments`

Retrieve variable pay history (bonuses, commissions, etc.) for an employee. Requires `Read variable pay data` permission.

**Path Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id` | string | Yes | Employee ID |

**Response:**

```json
{
  "values": [
    {
      "id": "1",
      "effectiveDate": "2024-03-31",
      "amount": { "value": 5000, "currency": "USD" },
      "type": "Annual Bonus"
    }
  ]
}
```

---

### `bob_get_training` — GET `/people/{id}/training/records`

Retrieve training and certification records for an employee.

**Path Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id` | string | Yes | Employee ID |

**Response:**

```json
{
  "values": [
    {
      "id": "1",
      "name": "Security Awareness Training",
      "completionDate": "2024-06-01",
      "status": "Completed"
    }
  ]
}
```

---

### `bob_get_bank_accounts` — GET `/people/{id}/bankAccounts`

Retrieve bank account records for an employee. Requires `Read bank account data` permission. Handle this data with strict confidentiality.

**Path Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id` | string | Yes | Employee ID |

**Response:**

```json
{
  "values": [
    {
      "id": "1",
      "accountType": "Checking",
      "bankName": "First National Bank",
      "accountNumber": "****1234"
    }
  ]
}
```
