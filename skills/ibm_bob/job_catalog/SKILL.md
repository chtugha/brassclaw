---
name: ibm_bob_job_catalog
description: Manage the job catalog in IBM Bob — job families, job levels, titles, grades, and compensation bands.
types: [ibm_bob]
---

# IBM Bob – Job Catalog

Use this skill when the user asks about job families, job titles, job levels, compensation grades, pay bands, or the company's job architecture defined in IBM Bob.

## Capabilities

- List all job families and their associated levels
- Retrieve job title definitions and associated grade/band
- Create or update job catalog entries
- Map employees to job catalog entries
- Query compensation bands linked to a job level
- Archive obsolete job titles

## API Patterns

Base path: `/api/v1/payroll/job-catalog` (some endpoints also under `/api/v1/company/job-catalog`)

### List job families
```
GET /api/v1/company/job-catalog/families
```

### List job titles in a family
```
GET /api/v1/company/job-catalog/titles?familyId={id}
```

### Get a job title definition
```
GET /api/v1/company/job-catalog/titles/{titleId}
```

### Create a job title
```
POST /api/v1/company/job-catalog/titles
Body: {
  "familyId": "...",
  "title": "...",
  "level": "IC3",
  "gradeId": "...",
  "compensationBandId": "..."
}
```

### Update a job title
```
PUT /api/v1/company/job-catalog/titles/{titleId}
```

### Archive a job title
```
DELETE /api/v1/company/job-catalog/titles/{titleId}
```

## Notes

- Job catalog changes do not retroactively alter employee records; use a separate position-update request to move employees.
- Compensation band IDs are resolved via the compensation module; always verify band existence before creating a title.
- Levels follow the company's defined level ladder (e.g., IC1–IC7, M1–M5); validate level codes before submission.
