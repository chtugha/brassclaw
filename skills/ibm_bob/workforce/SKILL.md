---
name: ibm_bob_workforce
description: Access workforce planning data, headcount forecasts, organizational structure snapshots, and position management via IBM Bob.
types: [ibm_bob]
---

# IBM Bob – Workforce

Use this skill when the user asks about workforce planning, headcount projections, org chart data, position management, department structures, or FTE budgets in IBM Bob.

## Capabilities

- Retrieve organizational structure (departments, sub-departments, reporting lines)
- List open and filled positions with headcount budgets
- Get headcount snapshots for a date or period
- Create or update position records
- Query FTE (full-time equivalent) data by department or cost center
- Export org chart data for visualization tools

## API Patterns

Base path: `/api/v1/workforce`

### Get org structure
```
GET /api/v1/workforce/org-chart
Query params: departmentId (optional), depth (optional, default 3)
```

### List positions
```
GET /api/v1/workforce/positions?status={open|filled|all}&departmentId={id}
```

### Get headcount snapshot
```
GET /api/v1/workforce/headcount?date=YYYY-MM-DD&groupBy={department|location|costCenter}
```

### Create a position
```
POST /api/v1/workforce/positions
Body: {
  "title": "...",
  "departmentId": "...",
  "fte": 1.0,
  "location": "...",
  "startDate": "YYYY-MM-DD"
}
```

### Update a position
```
PUT /api/v1/workforce/positions/{positionId}
```

## Notes

- Org chart depth beyond 5 levels may result in large payloads; use `departmentId` to scope requests.
- Headcount snapshots are point-in-time; historical data is available from the company's go-live date.
- FTE values must be between 0.1 and 1.0 inclusive.
