import assert from "node:assert/strict";
import test from "node:test";

import {
  parseTemplate,
  classifyTemplate,
  templateFeedback,
} from "./template-feedback.js";

// ── parseTemplate (mirrors template_extractor.rs tests) ─────────────────────

test("parseTemplate returns null for a plain exact-match expression", () => {
  assert.equal(parseTemplate("no slots here"), null);
});

test("parseTemplate splits a dual-anchored template", () => {
  assert.deepEqual(parseTemplate("show me files in the % directory"), {
    prefix: "show me files in the ",
    suffix: " directory",
  });
});

test("parseTemplate prefix-anchored trailing slot (FIND-P9-12)", () => {
  assert.deepEqual(parseTemplate("search for %"), {
    prefix: "search for ",
    suffix: "",
  });
});

test("parseTemplate suffix-anchored leading slot", () => {
  assert.deepEqual(parseTemplate("% directory"), { prefix: "", suffix: " directory" });
});

test("parseTemplate bare slot is no-anchor", () => {
  assert.deepEqual(parseTemplate("%"), { prefix: "", suffix: "" });
});

test("parseTemplate leading and trailing slot is no-anchor", () => {
  // "% in %" → last `%` is at the end, so suffix is "" and prefix is "".
  assert.deepEqual(parseTemplate("% in %"), { prefix: "", suffix: "" });
});

// ── classifyTemplate (§0.17.5 anchor indicators) ─────────────────────────────

test("classifyTemplate plain when there are no slots", () => {
  assert.equal(classifyTemplate("", "", 0), "plain");
});

test("classifyTemplate anchored when prefix is non-empty", () => {
  assert.equal(classifyTemplate("read ", " file", 1), "anchored");
  assert.equal(classifyTemplate("search for ", "", 1), "anchored");
});

test("classifyTemplate suffix-only for leading-% with suffix", () => {
  assert.equal(classifyTemplate("", " directory", 1), "suffix-only");
});

test("classifyTemplate no-anchor when both empty", () => {
  assert.equal(classifyTemplate("", "", 1), "no-anchor");
});

// ── templateFeedback (mirrors recipe_validator.rs intent_example_* tests) ────

test("templateFeedback: plain literal is valid, not a template", () => {
  const fb = templateFeedback("list files");
  assert.equal(fb.isTemplate, false);
  assert.equal(fb.classification, "plain");
  assert.equal(fb.slotCount, 0);
  assert.deepEqual(fb.errors, []);
  assert.deepEqual(fb.warnings, []);
});

test("templateFeedback: dual-anchored template is valid (green)", () => {
  const fb = templateFeedback("read % file");
  assert.equal(fb.isTemplate, true);
  assert.equal(fb.classification, "anchored");
  assert.equal(fb.slotCount, 1);
  assert.deepEqual(fb.errors, []);
  assert.deepEqual(fb.warnings, []);
});

test("templateFeedback: bare `%` is no-anchor hard error (red)", () => {
  const fb = templateFeedback("%");
  assert.equal(fb.classification, "no-anchor");
  assert.equal(fb.errors.length, 1);
  assert.match(fb.errors[0], /no anchor/);
  assert.deepEqual(fb.warnings, []);
});

test("templateFeedback: `% in %` is no-anchor hard error", () => {
  const fb = templateFeedback("% in %");
  assert.equal(fb.classification, "no-anchor");
  assert.equal(fb.errors.length, 1);
  assert.match(fb.errors[0], /no anchor/);
});

test("templateFeedback: leading-`%` with suffix is valid + warning (yellow)", () => {
  const fb = templateFeedback("% directory");
  assert.equal(fb.classification, "suffix-only");
  assert.deepEqual(fb.errors, []);
  assert.equal(fb.warnings.length, 1);
  assert.match(fb.warnings[0], /leading-`%`/);
});

test("templateFeedback: trailing adjacent `%%` is hard error", () => {
  const fb = templateFeedback("search %%");
  assert.equal(fb.errors.length, 1);
  assert.match(fb.errors[0], /adjacent/);
});

test("templateFeedback: middle adjacent `a %% b` is hard error (the windows-check gap)", () => {
  const fb = templateFeedback("a %% b");
  assert.equal(fb.errors.length, 1);
  assert.match(fb.errors[0], /adjacent/);
});

test("templateFeedback: space-separated `% %` is NOT adjacent (space is a literal separator)", () => {
  const fb = templateFeedback("search % %");
  assert.equal(fb.errors.find((e) => /adjacent/.test(e)), undefined);
});

test("templateFeedback: two slots with a literal separator are valid", () => {
  const fb = templateFeedback("search for % in %");
  assert.equal(fb.slotCount, 2);
  assert.equal(fb.classification, "anchored");
  assert.deepEqual(fb.errors, []);
  assert.deepEqual(fb.warnings, []);
});
