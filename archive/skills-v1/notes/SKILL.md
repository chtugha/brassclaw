---
name: notes
version: "2.0.0"
description: Store and retrieve local plain-text notes in ~/.brassclaw/notes.md
activation:
  keywords:
    - "note"
    - "notes"
    - "remember"
    - "memo"
    - "jot"
    - "write down"
    - "remind me"
  patterns:
    - "(?i)(remember|note|jot|write down).*(this|that|it)"
    - "(?i)(what|show).*(notes|noted|remembered)"
    - "(?i)(read|search|find).*(notes|memo)"
  tags:
    - "notes"
    - "local"
    - "memory"
  max_context_tokens: 192
---

# Local Notes

Manage persistent notes stored at `~/.brassclaw/notes.md`.

## Actions (via file tools)

- **Append**: Use `file_write` with `mode: append` to add to `~/.brassclaw/notes.md`. Prefix each entry with a timestamp line: `## YYYY-MM-DD HH:MM`.
- **Read**: Use `file_read` on `~/.brassclaw/notes.md` to show all notes.
- **Search**: Use `grep` or `local_search` with the query against `~/.brassclaw/notes.md`.

## Rules

1. Keep entries concise. One idea per note.
2. Always prepend a date header when appending.
3. When asked "what did I note about X", search before reading the full file.
