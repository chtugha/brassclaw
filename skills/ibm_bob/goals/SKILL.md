---
name: ibm_bob_goals
description: Create, track, and evaluate individual and team performance goals within IBM Bob's goal management system.
types: [llm, kohai, agent]
---

# IBM Bob – Goals

Use this skill when the user asks about employee performance goals, OKRs, goal cycles, progress check-ins, goal evaluations, or aligning team objectives in IBM Bob.

## Capabilities

- List goals for an employee or team, filtered by cycle or status
- Create individual or shared goals with targets, KPIs, and due dates
- Update goal progress or status (on-track, at-risk, completed)
- Link goals to a performance review cycle
- Archive or delete goals
- Query goal cycle definitions (annual, quarterly)

## API Patterns

Base path: `/api/v1/performance/goals`

### List goals for an employee
```
GET /api/v1/performance/goals?employeeId={id}&cycleId={cycleId}
```

### List goal cycles
```
GET /api/v1/performance/goals/cycles
```

### Create a goal
```
POST /api/v1/performance/goals
Body: {
  "employeeId": "...",
  "cycleId": "...",
  "title": "...",
  "description": "...",
  "dueDate": "YYYY-MM-DD",
  "kpis": [{ "name": "...", "target": 100, "unit": "%" }]
}
```

### Update goal progress
```
PUT /api/v1/performance/goals/{goalId}
Body: { "progress": 75, "status": "on-track" }
```

### Delete a goal
```
DELETE /api/v1/performance/goals/{goalId}
```

## Notes

- Goals can be marked as `private` to restrict visibility to the employee and their manager.
- KPI targets must use consistent units within a goal.
- Linking a goal to a closed cycle is not permitted; verify cycle status before creation.
