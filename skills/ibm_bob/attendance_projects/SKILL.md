---
name: ibm_bob_attendance_projects
description: Query and manage attendance project codes, project-level hour allocations, and project-based time tracking entries via IBM Bob HR.
types: [llm, kohai, agent]
---

# IBM Bob – Attendance Projects

Use this skill when the user asks about attendance project codes, project-based hour tracking, allocation of work hours to specific projects, or project time entries within IBM Bob.

## Capabilities

- List all attendance project codes and their descriptions
- Retrieve hours logged against a specific project code for an employee or date range
- Create, update, or delete project-hour entries tied to attendance records
- Report on project utilization across teams or departments

## API Patterns

All requests use the IBM Bob REST API (`/api/v1/attendances/projects` namespace).
Authenticate with the `iBobApiKey` bearer token from the Kohai provider credentials.

### List project codes
```
GET /api/v1/attendances/projects
```

### Get project hours for an employee
```
GET /api/v1/attendances/{employeeId}/projects?from={YYYY-MM-DD}&to={YYYY-MM-DD}
```

### Add project hours
```
POST /api/v1/attendances/{employeeId}/projects
Body: { "date": "YYYY-MM-DD", "projectCode": "...", "hours": 8 }
```

### Update project hours entry
```
PUT /api/v1/attendances/{employeeId}/projects/{entryId}
```

### Delete project hours entry
```
DELETE /api/v1/attendances/{employeeId}/projects/{entryId}
```

## Notes

- Project codes are company-configurable; always list available codes before attempting to log hours.
- Hours must be positive and within the daily limit set by the company's attendance policy.
- When a date range spans multiple months, paginate by month to stay within API result limits.
