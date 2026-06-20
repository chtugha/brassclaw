---
name: caldav
version: "2.0.0"
description: CalDAV calendar and task management via HTTP tool with automatic credential injection
activation:
  keywords:
    - "calendar"
    - "event"
    - "meeting"
    - "schedule"
    - "appointment"
    - "todo"
    - "reminder"
    - "caldav"
  exclude_keywords:
    - "google calendar"
  patterns:
    - "(?i)(add|create|new|schedule)\\s.*(event|meeting|appointment)"
    - "(?i)(list|show|what).*(calendar|events|meetings|schedule)"
    - "(?i)(delete|remove|cancel).*(event|meeting)"
    - "(?i)(todo|task|reminder)"
  tags:
    - "calendar"
    - "local"
    - "productivity"
  max_context_tokens: 384
credentials:
  - name: caldav_password
    provider: caldav
    location:
      type: basic_auth
      username: admin
    hosts: []
    setup_instructions: "Set CALDAV_URL, CALDAV_USERNAME, CALDAV_PASSWORD in your environment"
---

# CalDAV Calendar API

Manage calendar events and tasks on any CalDAV server (Nextcloud, Radicale, iCloud, Baikal, etc.) using the `http` tool. Credentials are injected automatically for the configured CalDAV host.

## Discovery

```
http(method="PROPFIND", url="{CALDAV_URL}", headers={"Depth": "1", "Content-Type": "application/xml"}, body="<?xml version='1.0'?><d:propfind xmlns:d='DAV:'><d:prop><d:displayname/><d:resourcetype/></d:prop></d:propfind>")
```

## List events (REPORT)

```
http(method="REPORT", url="{CALDAV_URL}/{calendar_path}/",
  headers={"Depth": "1", "Content-Type": "application/xml"},
  body="<?xml version='1.0'?><c:calendar-query xmlns:d='DAV:' xmlns:c='urn:ietf:params:xml:ns:caldav'><d:prop><d:getetag/><c:calendar-data/></d:prop><c:filter><c:comp-filter name='VCALENDAR'><c:comp-filter name='VEVENT'><c:time-range start='{start}' end='{end}'/></c:comp-filter></c:comp-filter></c:filter></c:calendar-query>")
```

Dates use iCalendar format: `20260515T100000Z`.

## Create event (PUT)

```
http(method="PUT", url="{CALDAV_URL}/{calendar_path}/{uid}.ics",
  headers={"Content-Type": "text/calendar"},
  body="BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nUID:{uid}\nDTSTART:{start}\nDTEND:{end}\nSUMMARY:{title}\nDESCRIPTION:{desc}\nEND:VEVENT\nEND:VCALENDAR")
```

Generate a unique UID with `{uuid}@brassclaw`.

## Delete event (DELETE)

```
http(method="DELETE", url="{CALDAV_URL}/{calendar_path}/{uid}.ics")
```

## Rules

1. Always discover calendars first if no path is known.
2. Confirm with the user before deleting events.
3. Use UTC timestamps in iCalendar format.
