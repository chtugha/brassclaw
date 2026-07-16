---
name: ibm_bob_learning
description: Access and manage employee learning paths, training enrollments, course completions, and certifications via IBM Bob's learning module.
types: [llm, kohai, agent]
---

# IBM Bob – Learning

Use this skill when the user asks about employee training, learning paths, course enrollments, certification tracking, mandatory compliance training, or learning progress in IBM Bob.

## Capabilities

- List available courses and learning paths
- Enroll an employee in a course or learning path
- Track course completion status and scores
- Record external certifications or manual completions
- Query overdue or expiring compliance training
- Generate learning completion reports

## API Patterns

Base path: `/api/v1/learning`

### List courses
```
GET /api/v1/learning/courses?type={online|classroom|external}&status={active|archived}
```

### List learning paths
```
GET /api/v1/learning/paths
```

### Enroll an employee
```
POST /api/v1/learning/enrollments
Body: {
  "employeeId": "...",
  "courseId": "...",
  "dueDate": "YYYY-MM-DD"
}
```

### Get enrollment status
```
GET /api/v1/learning/enrollments?employeeId={id}&status={enrolled|completed|overdue}
```

### Mark course complete
```
PUT /api/v1/learning/enrollments/{enrollmentId}
Body: { "status": "completed", "completionDate": "YYYY-MM-DD", "score": 95 }
```

### Record external certification
```
POST /api/v1/learning/certifications
Body: {
  "employeeId": "...",
  "name": "...",
  "issuer": "...",
  "issueDate": "YYYY-MM-DD",
  "expiryDate": "YYYY-MM-DD"
}
```

## Notes

- Compliance courses with a mandatory flag generate automatic reminders at 30 and 7 days before the due date.
- External certifications require a document upload via the Documents skill to attach proof.
- Learning paths auto-enroll in component courses when assigned to an employee.
