---
name: ibm_bob_reports
description: Generate, retrieve, and export HR reports including headcount, turnover, compensation, and custom report definitions from IBM Bob.
types: [ibm_bob]
---

# IBM Bob – Reports

Use this skill when the user asks to generate or retrieve HR reports, export data for analysis, access headcount summaries, turnover metrics, compensation breakdowns, or any custom report definition within IBM Bob.

## Capabilities

- List available report templates (system and custom)
- Execute a report with specified filters and date ranges
- Download report results as CSV or JSON
- Create or update custom report definitions
- Schedule recurring reports

## API Patterns

Base path: `/api/v1/reports`

### List available reports
```
GET /api/v1/reports
```

### Run a report
```
POST /api/v1/reports/{reportId}/run
Body: { "filters": { "department": "...", "from": "YYYY-MM-DD", "to": "YYYY-MM-DD" } }
```

### Get report results
```
GET /api/v1/reports/{reportId}/results/{runId}
```

### Download as CSV
```
GET /api/v1/reports/{reportId}/results/{runId}?format=csv
Accept: text/csv
```

### Create custom report
```
POST /api/v1/reports
Body: { "name": "...", "fields": [...], "filters": {...} }
```

## Notes

- Report execution is asynchronous for large datasets; poll `/results/{runId}` until `status` is `ready`.
- System reports cannot be deleted but can be cloned into custom reports.
- Always confirm the report `runId` before downloading to avoid stale results.
