---
name: ibm_bob_onboarding
description: Manage employee onboarding workflows, task checklists, welcome packets, and new-hire completion tracking in IBM Bob.
types: [ibm_bob]
---

# IBM Bob – Onboarding

Use this skill when the user asks about new employee onboarding, preboarding tasks, onboarding checklists, welcome emails, buddy assignments, or Day-1 readiness in IBM Bob.

## Capabilities

- Retrieve or start onboarding workflows for a new hire
- List onboarding tasks and their completion status
- Assign buddies or onboarding managers
- Send or resend welcome packets and portal invitations
- Mark tasks complete on behalf of the new hire or HR
- Query employees currently in onboarding and their progress

## API Patterns

Base path: `/api/v1/onboarding`

### List employees in onboarding
```
GET /api/v1/onboarding?status={pending|in-progress|completed}
```

### Get onboarding status for an employee
```
GET /api/v1/onboarding/{employeeId}
```

### Start onboarding workflow
```
POST /api/v1/onboarding/{employeeId}/start
Body: { "workflowTemplateId": "...", "startDate": "YYYY-MM-DD" }
```

### List onboarding tasks
```
GET /api/v1/onboarding/{employeeId}/tasks
```

### Complete a task
```
PUT /api/v1/onboarding/{employeeId}/tasks/{taskId}
Body: { "status": "completed", "completedBy": "...", "note": "..." }
```

### Assign a buddy
```
POST /api/v1/onboarding/{employeeId}/buddy
Body: { "buddyEmployeeId": "..." }
```

### Send portal invitation
```
POST /api/v1/onboarding/{employeeId}/invite
```

## Notes

- Onboarding workflows are template-driven; list available templates with `GET /api/v1/onboarding/templates`.
- Tasks that are auto-completed by other systems (e.g., IT provisioning) will show `completedBy: system`.
- New hires cannot log into the portal until the invitation is sent.
