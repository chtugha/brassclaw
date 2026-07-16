---
name: ibm_bob_hiring
description: Manage open positions, job postings, candidates, interviews, and offers in IBM Bob's applicant tracking system (ATS).
types: [llm, kohai, agent]
---

# IBM Bob – Hiring

Use this skill when the user asks about recruiting, applicant tracking, job postings, candidate pipelines, interview scheduling, offer letters, or any hiring workflow in IBM Bob.

## Capabilities

- List active job postings and their candidate pipelines
- Create or update job postings (title, department, location, description)
- Add candidates to a job posting
- Advance or reject a candidate through pipeline stages
- Schedule or log interviews with feedback
- Create and send offer letters
- Convert accepted offers to employee records

## API Patterns

Base path: `/api/v1/hiring`

### List job postings
```
GET /api/v1/hiring/positions?status={open|closed|draft}&departmentId={id}
```

### Create a job posting
```
POST /api/v1/hiring/positions
Body: {
  "title": "...",
  "departmentId": "...",
  "locationId": "...",
  "description": "...",
  "openDate": "YYYY-MM-DD"
}
```

### List candidates for a position
```
GET /api/v1/hiring/positions/{positionId}/candidates
```

### Add a candidate
```
POST /api/v1/hiring/positions/{positionId}/candidates
Body: { "firstName": "...", "lastName": "...", "email": "...", "resumeUrl": "..." }
```

### Advance candidate stage
```
PUT /api/v1/hiring/positions/{positionId}/candidates/{candidateId}/stage
Body: { "stage": "interview|offer|hired|rejected", "note": "..." }
```

### Create offer letter
```
POST /api/v1/hiring/positions/{positionId}/candidates/{candidateId}/offer
Body: { "startDate": "YYYY-MM-DD", "salary": 100000, "currency": "USD" }
```

## Notes

- Candidates moved to `hired` stage trigger an onboarding workflow; ensure position and start date are confirmed first.
- Rejected candidates are retained in the system for 90 days before automatic archival.
- Job postings can be published to external job boards through the `publish` sub-endpoint.
