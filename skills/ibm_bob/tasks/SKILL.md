---
name: ibm_bob_tasks
description: Create, assign, and track HR tasks and to-do items for employees and managers via IBM Bob.
types: [ibm_bob]
---

# IBM Bob – Tasks

Use this skill when the user asks about HR tasks, onboarding checklists, employee to-dos, task assignments, or task completion tracking within IBM Bob.

## Capabilities

- List open, completed, or overdue tasks for an employee or assignee
- Create new tasks with due dates, priorities, and assignees
- Update task status (open → in-progress → complete)
- Delete or reassign tasks
- Query tasks by category (onboarding, offboarding, compliance, general)

## API Patterns

Base path: `/api/v1/tasks`

### List tasks for an employee
```
GET /api/v1/tasks?employeeId={id}&status={open|completed|all}
```

### Create a task
```
POST /api/v1/tasks
Body: {
  "title": "...",
  "employeeId": "...",
  "dueDate": "YYYY-MM-DD",
  "priority": "high|medium|low",
  "assigneeId": "..."
}
```

### Update task status
```
PUT /api/v1/tasks/{taskId}
Body: { "status": "completed" }
```

### Delete a task
```
DELETE /api/v1/tasks/{taskId}
```

## Notes

- Tasks can be scoped to a specific employee or left unscoped (company-wide HR tasks).
- Use the `category` filter to narrow results when handling onboarding vs. compliance workflows.
- Overdue tasks surface in manager dashboards; ensure due dates are set accurately.
