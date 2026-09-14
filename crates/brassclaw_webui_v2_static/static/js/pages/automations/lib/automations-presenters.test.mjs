import assert from "node:assert/strict";
import test from "node:test";

import {
  automationSummary,
  durationLabel,
  filterAutomations,
  nextCronFire,
  normalizeAutomations,
  runStatusTone,
  scheduleLabel,
} from "./automations-presenters.js";

test("normalizeAutomations keeps only schedule rows and avoids raw schedule text", () => {
  const automations = normalizeAutomations({
    automations: [
      {
        automation_id: "daily",
        name: "Daily summary",
        source: { type: "schedule", cron: "0 9 * * 1-5" },
        state: "active",
        is_active: true,
        next_run_at: "2026-06-05T16:00:00Z",
        last_run_at: "2026-06-04T16:01:00Z",
        last_status: "ok",
      },
      {
        automation_id: "future-webhook",
        name: "Future webhook",
        source: { type: "webhook" },
        state: "active",
        is_active: true,
      },
    ],
  });

  assert.equal(automations.length, 1);
  assert.equal(automations[0].display_name, "Daily summary");
  assert.equal(automations[0].schedule_label, "Weekdays at 9:00 AM");
  assert.equal(automations[0].last_status_label, "Done");
});

test("normalizeAutomations handles empty and malformed schedule payloads", () => {
  assert.deepEqual(normalizeAutomations(null), []);
  assert.deepEqual(normalizeAutomations({ automations: "not-an-array" }), []);

  const automations = normalizeAutomations({
    automations: [
      {
        automation_id: "partial",
        source: { type: "schedule", cron: "bad schedule value" },
        state: "unexpected",
        is_active: false,
        next_run_at: "not-a-date",
        last_run_at: null,
        created_at: null,
        last_status: "unknown",
      },
    ],
  });

  assert.equal(automations.length, 1);
  assert.equal(automations[0].display_name, "Untitled automation");
  assert.equal(automations[0].schedule_label, "Custom schedule");
  assert.equal(automations[0].state_label, "Unknown");
  assert.equal(automations[0].state_tone, "muted");
  assert.equal(automations[0].next_run_label, "Not scheduled");
  assert.equal(automations[0].last_run_label, "No runs yet");
  assert.equal(automations[0].created_label, "Unknown");
  assert.equal(automations[0].last_status_label, "No result");
  assert.equal(automations[0].last_status_tone, "muted");
});

test("scheduleLabel presents common recurring schedules in friendly language", () => {
  assert.equal(scheduleLabel("30 14 * * *"), "Every day at 2:30 PM");
  assert.equal(scheduleLabel("0 30 14 * * *"), "Every day at 2:30 PM");
  assert.equal(scheduleLabel("00 30 14 * * * *"), "Every day at 2:30 PM");
  assert.equal(scheduleLabel("0 8 * * 1"), "Mondays at 8:00 AM");
  assert.equal(scheduleLabel("0 8 * * MON"), "Mondays at 8:00 AM");
  assert.equal(scheduleLabel("0 8 * * 7"), "Sundays at 8:00 AM");
  assert.equal(scheduleLabel("0 9 * * MON-FRI"), "Weekdays at 9:00 AM");
  assert.equal(scheduleLabel("0 17 1 * *"), "1st day of each month at 5:00 PM");
  assert.equal(scheduleLabel("0 17 11 * *"), "11th day of each month at 5:00 PM");
  assert.equal(scheduleLabel("0 17 12 * *"), "12th day of each month at 5:00 PM");
  assert.equal(scheduleLabel("0 17 13 * *"), "13th day of each month at 5:00 PM");
  assert.equal(scheduleLabel("0 0 9 1 1 * 2027"), "Jan 1, 2027 at 9:00 AM");
  assert.equal(scheduleLabel("*/5 * * * *"), "Custom schedule");
  assert.equal(scheduleLabel("* 0 9 * * *"), "Custom schedule");
  assert.equal(scheduleLabel("0 24 * * *"), "Custom schedule");
  assert.equal(scheduleLabel("0 0 32 * *"), "Custom schedule");
  assert.equal(scheduleLabel("0 0 * 13 *"), "Custom schedule");
});

test("filterAutomations, sorting, and summary use browser-visible active state", () => {
  const automations = normalizeAutomations({
    automations: [
      {
        automation_id: "active",
        name: "Active",
        source: { type: "schedule", cron: "0 9 * * *" },
        state: "active",
        is_active: false,
        next_run_at: "2026-06-05T17:00:00Z",
      },
      {
        automation_id: "scheduled",
        name: "Scheduled",
        source: { type: "schedule", cron: "0 10 * * *" },
        state: "scheduled",
        is_active: false,
        next_run_at: "2026-06-05T16:00:00Z",
      },
      {
        automation_id: "paused",
        name: "Paused",
        source: { type: "schedule", cron: "0 11 * * *" },
        state: "paused",
        is_active: true,
        next_run_at: "2026-06-05T18:00:00Z",
      },
    ],
  });

  assert.deepEqual(
    automations.map((automation) => automation.automation_id),
    ["scheduled", "active", "paused"],
  );
  assert.deepEqual(
    filterAutomations(automations, "active").map((automation) => automation.automation_id),
    ["scheduled", "active"],
  );
  assert.deepEqual(
    filterAutomations(automations, "paused").map((automation) => automation.automation_id),
    ["paused"],
  );
  assert.deepEqual(automationSummary(automations), {
    scheduled: 3,
    active: 2,
    paused: 1,
    nextRun: automations[0].next_run_label,
  });
});

test("automationSummary ignores unparseable next_run_at values", () => {
  const automations = normalizeAutomations({
    automations: [
      {
        automation_id: "invalid-next-run",
        name: "Invalid next run",
        source: { type: "schedule", cron: "0 9 * * *" },
        state: "scheduled",
        is_active: false,
        next_run_at: "not-a-date",
      },
    ],
  });

  assert.equal(automations[0].next_run_label, "Not scheduled");
  assert.deepEqual(automationSummary(automations), {
    scheduled: 1,
    active: 1,
    paused: 0,
    nextRun: null,
  });
});

test("nextCronFire returns null for invalid cron expression", () => {
  assert.equal(nextCronFire(""), null);
  assert.equal(nextCronFire("not a cron"), null);
  assert.equal(nextCronFire("* * * *"), null);  // too few fields
});

test("nextCronFire returns a string for a valid simple daily cron", () => {
  // "0 9 * * *" — daily at 09:00
  const result = nextCronFire("0 9 * * *");
  // Either a non-empty string (future time) or null if current time > 09:00
  // and the implementation returns the next day — either way it's string or null
  assert.ok(result === null || typeof result === "string");
});

test("durationLabel formats sub-minute, minute, and multi-hour durations", () => {
  const start = "2024-01-01T10:00:00Z";
  assert.equal(durationLabel(start, "2024-01-01T10:00:45Z"), "45s");
  assert.equal(durationLabel(start, "2024-01-01T10:02:14Z"), "2m 14s");
  assert.equal(durationLabel(start, "2024-01-01T11:30:00Z"), "90m 0s");
  assert.equal(durationLabel(start, null), "—");
  assert.equal(durationLabel(start, undefined), "—");
  assert.equal(durationLabel("bad", "2024-01-01T10:00:00Z"), "—");
});

test("runStatusTone maps all three values correctly", () => {
  assert.equal(runStatusTone("ok"), "success");
  assert.equal(runStatusTone("error"), "danger");
  assert.equal(runStatusTone("unknown"), "muted");
  assert.equal(runStatusTone(undefined), "muted");
});

test("normalizeAutomations correctly carries through prompt and completion_policy fields", () => {
  const automations = normalizeAutomations({
    automations: [
      {
        automation_id: "with-fields",
        name: "Daily",
        source: { type: "schedule", cron: "0 9 * * *" },
        state: "scheduled",
        is_active: false,
        prompt: "Summarise recent events",
        completion_policy: "recurring",
      },
    ],
  });

  assert.equal(automations.length, 1);
  assert.equal(automations[0].prompt, "Summarise recent events");
  assert.equal(automations[0].completion_policy, "recurring");
});

test("normalizeAutomations preserves explicit unknown state even when is_active is true", () => {
  const automations = normalizeAutomations({
    automations: [
      {
        automation_id: "unknown-active",
        name: "Unknown active",
        source: { type: "schedule", cron: "0 9 * * *" },
        state: "unknown",
        is_active: true,
      },
    ],
  });

  assert.equal(automations[0].state_label, "Unknown");
  assert.equal(automations[0].state_tone, "muted");
});
