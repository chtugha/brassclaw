---
name: local-search
version: "2.0.0"
description: Search files in the workspace using regex patterns
activation:
  keywords:
    - "search"
    - "find"
    - "grep"
    - "locate"
    - "look for"
    - "where is"
  patterns:
    - "(?i)(find|search|grep|locate)\\s.*(file|code|text|function|class)"
    - "(?i)where\\s.*(defined|used|called|declared)"
    - "(?i)(list|show)\\s.*(files|directory)"
  tags:
    - "search"
    - "local"
    - "filesystem"
  max_context_tokens: 256
---

# Local File Search

Use the `local_search` action to find files and content in the workspace.

## Parameters

- `pattern` (required): regex pattern to match
- `path`: directory to search in (defaults to workspace root)
- `glob`: file filter (e.g. `*.rs`, `*.py`)
- `output_mode`: `files_with_matches` (default), `content` (show matching lines), `count`

## Examples

- Find all Python files: `local_search(pattern=".", glob="*.py", output_mode="files_with_matches")`
- Find function definition: `local_search(pattern="fn\\s+process_", glob="*.rs", output_mode="content")`
- Count TODO comments: `local_search(pattern="TODO|FIXME", output_mode="count")`

## Rules

1. Default scope is `workspace` -- safe and sandboxed.
2. Do not request `scope: global` unless the user explicitly asks.
3. Use specific glob patterns to narrow results.
