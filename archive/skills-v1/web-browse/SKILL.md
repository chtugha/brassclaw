---
name: web-browse
version: "2.0.0"
description: Browse websites and extract content using the Playwright MCP browser
activation:
  keywords:
    - "browse"
    - "website"
    - "url"
    - "web"
    - "google"
    - "look up"
    - "find online"
    - "open page"
    - "screenshot"
  patterns:
    - "(?i)(search|look up|find|browse)\\s.*(web|online|internet|google)"
    - "(?i)(go to|open|visit|navigate)\\s.*(https?://|www\\.)"
    - "(?i)what (is|are|does).*(website|page|url)"
    - "(?i)(screenshot|capture|snap).*(page|screen|site)"
  tags:
    - "web"
    - "browser"
    - "search"
  max_context_tokens: 320
---

# Web Browse (Playwright MCP)

Use the Playwright MCP browser tool to navigate websites and extract content.

## Actions

- `browser_navigate`: open a URL (param: `url`)
- `browser_get_text`: extract visible text from page or CSS selector
- `browser_screenshot`: capture page as image
- `browser_click`: click element by CSS selector
- `browser_type`: type into an input field (params: `selector`, `text`)
- `browser_select`: select option from dropdown

## Web Search Pattern

1. Navigate to `https://www.google.com/search?q={query}`
2. Extract text from search results
3. Navigate to relevant result links for detail

## Rules

1. Prefer HTTPS URLs. Reject `file://` URLs.
2. Extract only needed text -- avoid full-page dumps.
3. Max 3 page loads per task unless the user requests more.
4. Summarize extracted content; do not return raw HTML.
5. For screenshots, describe what you see in the image.
