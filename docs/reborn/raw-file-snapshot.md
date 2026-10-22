# Complete UTF-8 file capture

Source implementation and scoped local acceptance: 2026-10-23. This is
R10's filesystem acquisition prerequisite, not an activated doc-conversion,
doc-sync or Prefix workflow. No Prefix-owned files or stores are changed.

The [validation record](../validation/2026-10-23-raw-file-snapshot.md) identifies
the actual native callers, live WebUI/PostgreSQL checks, backend race tests and
retained binaries. This is primitive support, not standalone pure approval or
canonical Skill/Recipe qualification.

The existing `builtin.read_file` registration accepts an explicit representation:

```python
host.read_file(path=inputs["path"], representation="raw_utf8_preserved")
```

The ordinary missing/`numbered_lines` representation retains its numbered preview
and offset/limit behavior. Raw capture forbids both range fields, including zero,
null and other explicit values. Caller-supplied `complete`, content or source
identity is not accepted. The adapter and registered input schema agree on the
two profiles.

A successful raw result has exactly these required fields:

| Field | Contract |
| --- | --- |
| `content` | Complete captured UTF-8 text; preserve BOM, Unicode, CR, LF, CRLF and final-newline presence. Empty content is valid. |
| `complete` | `true`, produced only after the bounded native read and encoding checks succeed. |
| `representation` | `raw_utf8_preserved`. |
| `source_ref` | `file-read-snapshot/1:<native-invocation-uuid>:<lowercase-blake3-digest>`. |

The versioned digest uses BLAKE3 derive-key context
`brassclaw.file-read-snapshot.1`, followed by the resolved scoped path's UTF-8 byte
length as little-endian u64, its bytes, then the exact captured file bytes.
The invocation is host-assigned. The reference identifies the captured output;
it is not a mutable path/refetch handle, a grant, component approval or an OS
filesystem checkpoint. Retained execution keeps the actual output under its
existing native invocation journal. Consumers must retain that snapshot and its
reference rather than reread the path later and claim it is the original source.
Do not claim atomic filesystem-wide state against unrelated external writers.

The existing current Tool policy, mount/path containment, sensitive/structured
entry protection, binary guard and acknowledged `max_coding_read_bytes` apply.
The preview's `default_coding_read_lines` does not cut a raw result. Invalid UTF-8
and UTF-16 are errors, with no replacement decoding or conversion. Oversized
or failed reads and kernel output limits are whole-operation failures, never
partial snapshots marked complete. No new fixed read quota or setting is added.

The separate reusable Skill export/ToolSkill association, R10 pure checker,
canonical execution Recipe, package qualification and doc-sync/doc-convert
consumer migration still require implementation and acceptance. Exposing the
primitive's registered parameter does not activate a component or bypass those
gates. See [the composition owner](../plans/monty.composition.implementation.md#r10-proposed-complete-document-text).
