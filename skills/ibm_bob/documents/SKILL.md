---
name: ibm_bob_documents
description: Upload, retrieve, and manage HR documents, contracts, and employee files stored in IBM Bob.
types: [llm, kohai, agent]
---

# IBM Bob – Documents

Use this skill when the user asks about uploading or retrieving employee documents, HR contracts, offer letters, policy acknowledgements, or any file stored in IBM Bob's document management system.

## Capabilities

- List documents attached to an employee profile
- Upload a new document (PDF, DOCX, image) to an employee record
- Download or get a signed URL for an existing document
- Update document metadata (name, category, expiry date)
- Delete a document
- List document categories and required document types

## API Patterns

Base path: `/api/v1/docs`

### List employee documents
```
GET /api/v1/docs/{employeeId}
```

### Upload a document
```
POST /api/v1/docs/{employeeId}
Content-Type: multipart/form-data
Fields: file, category, documentName, expirationDate (optional)
```

### Get document download URL
```
GET /api/v1/docs/{employeeId}/{documentId}
```

### Update document metadata
```
PUT /api/v1/docs/{employeeId}/{documentId}
Body: { "documentName": "...", "category": "...", "expirationDate": "YYYY-MM-DD" }
```

### Delete a document
```
DELETE /api/v1/docs/{employeeId}/{documentId}
```

## Notes

- Maximum file size is 25 MB per document.
- Supported MIME types: `application/pdf`, `application/msword`, `application/vnd.openxmlformats-officedocument.wordprocessingml.document`, common image types.
- Sensitive documents (contracts, IDs) require the `document:sensitive` scope on the API key.
- Documents with an `expirationDate` generate alerts when they are within 30 days of expiry.
