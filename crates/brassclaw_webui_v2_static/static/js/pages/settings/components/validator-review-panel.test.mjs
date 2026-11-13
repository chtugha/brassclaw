// Rendered UI gates and actual message packs only. No backend approval actions.
// Tests cover B3-09 requirements:
//  - Class-21 (Recipe) and class-22 (PythonCode) subject inspection
//  - Both classes show actionNotConnected notice for q2_manual queue code
//  - Draft-vs-active / historical subject display
//  - Missing evidence / no submission
//  - Private validator exclusion (non-21/22 classes return null)
//  - En / De message packs are complete for all validatorReview.* keys used
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

const withoutImports = (source) =>
  source.replace(/^import[\s\S]*?from\s+["'][^"']+["'];[ \t]*\n?/gm, "");

function messages(language) {
  let pack;
  const source = readFileSync(
    new URL(`../../../i18n/${language}.js`, import.meta.url),
    "utf8"
  );
  vm.runInNewContext(withoutImports(source), {
    registerPack(name, entries) {
      if (name === language) pack = entries;
    },
  });
  assert.ok(pack, `missing actual ${language} message pack`);
  return pack;
}

function translations(language) {
  let providerContext;
  const context = {
    navigator: { language },
    localStorage: { getItem: () => null },
    document: { documentElement: {} },
    React: {
      createContext(initial) {
        providerContext = { current: initial, Provider: "I18nProvider" };
        return providerContext;
      },
      useState: (initial) => [
        typeof initial === "function" ? initial() : initial,
        () => {},
      ],
      useCallback: (cb) => cb,
      useMemo: (cb) => cb(),
      useEffect: (cb) => cb(),
      useContext: (ctx) => ctx.current,
    },
    html(strings, provider, value) {
      assert.equal(provider, providerContext.Provider);
      providerContext.current = value;
    },
  };
  vm.createContext(context);
  const source = readFileSync(
    new URL("../../../lib/i18n.js", import.meta.url),
    "utf8"
  );
  vm.runInContext(withoutImports(source).replaceAll("export ", ""), context);
  for (const packLanguage of new Set(["en", language])) {
    const packSource = readFileSync(
      new URL(`../../../i18n/${packLanguage}.js`, import.meta.url),
      "utf8"
    );
    vm.runInContext(withoutImports(packSource), context);
  }
  context.I18nProvider({ children: null });
  assert.equal(providerContext.current.lang, language);
  return { useT: context.useT, t: context.useT() };
}

/**
 * Minimal stub view returned by fetchComponentReviewSubmission.
 * This matches the shape of ComponentReviewSubmissionView (Rust types).
 */
function makeView(opts = {}) {
  return {
    receipt: {
      submission_id: opts.submissionId ?? "sub-abc",
      subject_checksum: opts.checksum ?? "sha256:abc123",
      review_status: opts.reviewStatus ?? "pending",
      candidate: {
        uuid: "uuid-cand",
        class_code: opts.classCode ?? 21,
        version: 1,
        checksum: "sha256:cand",
      },
      base: opts.base ?? null,
      dependencies: opts.dependencies ?? [],
      function_partner: opts.functionPartner ?? null,
      catalogue_activated: opts.catalogueActivated ?? false,
      queue_binding_status: "bound",
      queue_ids: [],
    },
    subject_bytes: opts.subjectBytes ?? '{"type":"recipe","name":"test"}',
    component_bytes: opts.componentBytes ?? ['{"name":"comp1"}'],
    review_selection_bytes: opts.reviewSelectionBytes ?? null,
    work: opts.work ?? null,
  };
}

/**
 * Build a fixture with a controlled React/query environment.
 * The panel is inspection-only: no mutations, no approve/reject.
 */
function fixture({
  language = "en",
  classCode = 21,
  queueCode = "q2_manual",
  originalState = "bound",
  submissionId = "sub-abc",
  viewOverride = null,
  queryError = null,
  queryLoading = false,
} = {}) {
  const pack = messages(language);
  const { useT, t } = translations(language);
  let cursorState = 0;
  const states = [];

  const submittedView = viewOverride ?? makeView({ classCode, submissionId });

  const context = {
    React: {
      useState(initial) {
        const i = cursorState++;
        if (!(i in states))
          states[i] = typeof initial === "function" ? initial() : initial;
        return [
          states[i],
          (val) => {
            states[i] =
              typeof val === "function" ? val(states[i]) : val;
          },
        ];
      },
      useRef(initial) { return { current: initial }; },
      useMemo: (cb) => cb(),
      useCallback: (cb) => cb,
    },
    html: (strings, ...values) => ({ strings: [...strings], values }),
    Badge: "Badge",
    Card: "Card",
    Section: "Section",
    Field: "Field",
    FieldGrid: "FieldGrid",
    CodeBlock: "CodeBlock",
    useT,
    useQuery({ enabled, queryFn } = {}) {
      if (enabled === false) {
        return { isLoading: false, isError: false, data: null };
      }
      if (queryLoading) return { isLoading: true, isError: false, data: null };
      if (queryError) return { isLoading: false, isError: true, error: queryError, data: null };
      return { isLoading: false, isError: false, data: submittedView };
    },
    fetchComponentReviewSubmission() { return Promise.resolve(submittedView); },
  };

  const item = {
    id: "component-id",
    name: "Test Recipe",
    class_code: classCode,
    queue_code: queueCode,
    validation_status: "auto_passed",
    original_review: {
      state: originalState,
      submission_id: submissionId,
      queue_id: "queue-orig-id",
    },
  };

  const source =
    withoutImports(
      readFileSync(
        new URL("./validator-review-panel.js", import.meta.url),
        "utf8"
      )
    ).replaceAll("export ", "");

  vm.createContext(context);
  vm.runInContext(
    source +
      "\nglobalThis.renderPanel = ValidatorReviewPanel;" +
      "\nglobalThis.renderPanelInner = ValidatorReviewPanelInner;",
    context
  );

  return {
    // render() uses the guard wrapper — returns null for excluded classes,
    // returns the inner template node for included classes.
    // Use for class-exclusion tests only.
    render() {
      cursorState = 0;
      return context.renderPanel({ item });
    },
    // renderContent() calls the inner function directly for included classes
    // so static-vm test traversal can reach all displayed values.
    renderContent() {
      cursorState = 0;
      return context.renderPanelInner({ item });
    },
    pack,
    t,
    item,
  };
}

function nodes(root) {
  const out = [];
  const visit = (value) => {
    if (Array.isArray(value)) return value.forEach(visit);
    if (!value?.strings) return;
    out.push(value);
    value.values.forEach(visit);
  };
  visit(root);
  return out;
}

function visible(tree, text) {
  assert.equal(typeof text, "string", "expected an actual message/text value");
  return nodes(tree).some((node) => node.values.includes(text));
}

function button(tree, label) {
  for (const node of nodes(tree)) {
    const end = node.values.indexOf(label);
    if (end < 0) continue;
    let start = end;
    while (start >= 0 && !node.strings[start].includes("<button")) start--;
    if (start >= 0) return { node, start, end };
  }
}

// ── Message completeness ──────────────────────────────────────────────────────

for (const language of ["en", "de"]) {
  test(`${language} message pack contains all validatorReview.* keys`, () => {
    const pack = messages(language);
    const required = [
      "validatorReview.title",
      "validatorReview.scope",
      "validatorReview.noSubmission",
      "validatorReview.loadFailed",
      "validatorReview.historicalSubject",
      "validatorReview.subjectChecksum",
      "validatorReview.reviewStatus",
      "validatorReview.submissionId",
      "validatorReview.catalogueActivated",
      "validatorReview.yes",
      "validatorReview.candidate",
      "validatorReview.base",
      "validatorReview.dependencies",
      "validatorReview.functionPartner",
      "validatorReview.partnerChanged",
      "validatorReview.partnerContext",
      "validatorReview.partnerRef",
      "validatorReview.workStatus",
      "validatorReview.subjectBytes",
      "validatorReview.componentBytes",
      "validatorReview.reviewSelection",
      "validatorReview.classCode",
      "validatorReview.reviewActions",
      "validatorReview.actionNotConnected",
    ];
    for (const key of required) {
      assert.ok(key in pack, `${language} missing key: ${key}`);
      assert.ok(
        pack[key] && pack[key] !== key,
        `${language} key ${key} has no translation`
      );
    }
  });
}

for (const language of ["en", "de"]) {
  test(`${language} message pack does not contain removed keys`, () => {
    const pack = messages(language);
    const removed = [
      "validatorReview.prerequisitesMissing",
      "validatorReview.staleViewConflict",
      "validatorReview.newDraftHint",
    ];
    for (const key of removed) {
      assert.ok(
        !(key in pack),
        `${language} must not contain removed key: ${key}`
      );
    }
  });
}

// ── Class exclusion ───────────────────────────────────────────────────────────

for (const classCode of [0, 1, 2, 3, 10, 13, 16, 23, 50]) {
  test(`class ${classCode} renders null — not a Recipe/PythonCode reviewer`, () => {
    const f = fixture({ classCode });
    const tree = f.render();
    assert.equal(tree, null);
  });
}

// ── Class 21 (Recipe) subject inspection ─────────────────────────────────────

test("class-21 item shows title and subject checksum", () => {
  const f = fixture({ classCode: 21, originalState: "bound" });
  const tree = f.renderContent();
  assert.notEqual(tree, null);
  assert.ok(
    visible(tree, f.pack["validatorReview.title"]),
    "title must be visible"
  );
  assert.ok(visible(tree, "sha256:abc123"), "subject checksum must be visible");
});

test("class-21 q2_manual item shows actionNotConnected notice", () => {
  const f = fixture({ classCode: 21, queueCode: "q2_manual", originalState: "bound" });
  const tree = f.renderContent();
  assert.ok(
    visible(tree, f.pack["validatorReview.actionNotConnected"]),
    "actionNotConnected notice must be visible for class-21 q2_manual"
  );
});

test("class-21 q2_manual item shows no approve or reject buttons", () => {
  const f = fixture({ classCode: 21, queueCode: "q2_manual", originalState: "bound" });
  const tree = f.renderContent();
  // No approve/reject buttons — this panel is inspection-only
  assert.equal(
    button(tree, "Approve"),
    undefined,
    "approve button must not appear"
  );
  assert.equal(
    button(tree, "Reject"),
    undefined,
    "reject button must not appear"
  );
});

test("class-21 non-q2_manual queue code shows no review section", () => {
  const f = fixture({ classCode: 21, queueCode: "auto", originalState: "bound" });
  const tree = f.renderContent();
  assert.notEqual(tree, null);
  assert.equal(
    visible(tree, f.pack["validatorReview.actionNotConnected"]),
    false,
    "actionNotConnected only appears on q2_manual items"
  );
});

// ── Class 22 (PythonCode) subject inspection — Q2 writer not yet connected ───

test("class-22 item shows subject checksum and actionNotConnected notice", () => {
  const f = fixture({ classCode: 22, originalState: "bound" });
  const tree = f.renderContent();
  assert.notEqual(tree, null);
  // Subject checksum is shown for inspection.
  assert.ok(visible(tree, "sha256:abc123"), "subject checksum must be visible");
  // No approve/reject buttons — backend writer not connected.
  assert.equal(button(tree, "Approve"), undefined,
    "approve button must not appear");
  assert.equal(button(tree, "Reject"), undefined,
    "reject button must not appear");
  // Explicit notice shown instead.
  assert.ok(
    visible(tree, f.pack["validatorReview.actionNotConnected"]),
    "actionNotConnected notice must be visible for class 22"
  );
});

test("class-22 non-q2_manual queue code shows no review section", () => {
  const f = fixture({ classCode: 22, queueCode: "auto", originalState: "bound" });
  const tree = f.renderContent();
  assert.notEqual(tree, null);
  assert.equal(
    visible(tree, f.pack["validatorReview.actionNotConnected"]),
    false,
    "actionNotConnected only appears on q2_manual items"
  );
});

// ── Historical subject labeling ───────────────────────────────────────────────

test("approved review status shows historical subject warning", () => {
  const f = fixture({
    classCode: 21,
    originalState: "bound",
    viewOverride: makeView({ reviewStatus: "approved", classCode: 21 }),
  });
  const tree = f.renderContent();
  assert.ok(
    visible(tree, f.pack["validatorReview.historicalSubject"]),
    "historical subject label must appear for non-pending status"
  );
  assert.ok(visible(tree, "approved"), "review status value must be visible");
});

test("pending review status does not show historical subject warning", () => {
  const f = fixture({ classCode: 21, originalState: "bound" }); // default reviewStatus: "pending"
  const tree = f.renderContent();
  assert.equal(
    visible(tree, f.pack["validatorReview.historicalSubject"]),
    false,
    "pending is not historical — no warning expected"
  );
});

// ── No submission linked ──────────────────────────────────────────────────────

test("item with no submission_id shows noSubmission message", () => {
  const f = fixture({
    classCode: 21,
    originalState: "bound",
    submissionId: "",
  });
  const tree = f.renderContent();
  assert.ok(
    visible(tree, f.pack["validatorReview.noSubmission"]),
    "noSubmission must be visible when submission_id is absent"
  );
  // No approve button when no submission
  assert.equal(button(tree, "Approve"), undefined);
});

test("item with null submission_id shows noSubmission message", () => {
  const f = fixture({ classCode: 21, originalState: "bound" });
  // Override item's submission_id to null
  f.item.original_review.submission_id = null;
  const tree = f.renderContent();
  assert.ok(visible(tree, f.pack["validatorReview.noSubmission"]));
});

// ── Load failure ──────────────────────────────────────────────────────────────

test("loadFailed error message is shown when query fails", () => {
  const f = fixture({
    classCode: 21,
    originalState: "bound",
    queryError: { message: "Network timeout" },
  });
  const tree = f.renderContent();
  // The rendered value is the interpolated translation, not the template key.
  const interpolated = f.t("validatorReview.loadFailed", { message: "Network timeout" });
  assert.notEqual(interpolated, "validatorReview.loadFailed", "key must translate");
  assert.ok(
    visible(tree, interpolated),
    "interpolated load-failed message must be visible in tree"
  );
});

// ── Loading state ─────────────────────────────────────────────────────────────

test("loading state shows skeleton without buttons", () => {
  const f = fixture({ classCode: 21, originalState: "bound", queryLoading: true });
  const tree = f.renderContent();
  assert.notEqual(tree, null);
  assert.equal(button(tree, "Approve"), undefined);
  assert.equal(button(tree, "Reject"), undefined);
});

// ── Subject bytes displayed ───────────────────────────────────────────────────

test("submission ID is visible in the subject view", () => {
  const f = fixture({
    classCode: 21,
    originalState: "bound",
    submissionId: "my-exact-sub-id",
    viewOverride: makeView({ submissionId: "my-exact-sub-id", classCode: 21 }),
  });
  const tree = f.renderContent();
  assert.ok(
    visible(tree, "my-exact-sub-id"),
    "submissionId from the view must be visible"
  );
});

// ── German translations ───────────────────────────────────────────────────────

test("de bound class-21 item shows translated title", () => {
  const f = fixture({ language: "de", classCode: 21, originalState: "bound" });
  const tree = f.renderContent();
  const pack = f.pack;
  assert.ok(
    visible(tree, pack["validatorReview.title"]),
    "German title must be visible"
  );
});

test("de class-21 q2_manual item shows translated actionNotConnected notice", () => {
  const f = fixture({ language: "de", classCode: 21, queueCode: "q2_manual", originalState: "bound" });
  const tree = f.renderContent();
  assert.ok(
    visible(tree, f.pack["validatorReview.actionNotConnected"]),
    "German actionNotConnected notice must be visible"
  );
  // The German message must not mention any internal plan names
  assert.ok(
    !f.pack["validatorReview.actionNotConnected"].includes("B1-06"),
    "German actionNotConnected must not mention B1-06"
  );
});

test("de class-22 item shows translated actionNotConnected notice", () => {
  const f = fixture({ language: "de", classCode: 22, queueCode: "q2_manual", originalState: "bound" });
  const tree = f.renderContent();
  assert.ok(
    visible(tree, f.pack["validatorReview.actionNotConnected"]),
    "German actionNotConnected must be visible for class 22"
  );
});

// ── Work status display ───────────────────────────────────────────────────────

test("work status is shown when present in submission view", () => {
  const viewWithWork = makeView({
    classCode: 21,
    work: { status: "structural_observations_confirmed" },
  });
  const f = fixture({
    classCode: 21,
    originalState: "bound",
    viewOverride: viewWithWork,
  });
  const tree = f.renderContent();
  const workMsg = f.pack["componentSubmission.work.structural_observations_confirmed"];
  assert.ok(workMsg, "work status key must exist in en pack");
  assert.ok(
    visible(tree, workMsg),
    "work status message must be visible in expanded view"
  );
});

// ── Catalogue activated flag ──────────────────────────────────────────────────

test("catalogue_activated = true shows the activated notice", () => {
  const viewActivated = makeView({ classCode: 21, catalogueActivated: true });
  const f = fixture({
    classCode: 21,
    originalState: "bound",
    viewOverride: viewActivated,
  });
  const tree = f.renderContent();
  assert.ok(
    visible(tree, f.pack["validatorReview.catalogueActivated"]),
    "catalogue activated label must be shown"
  );
});

test("catalogue_activated = false does not show the activated notice", () => {
  const f = fixture({ classCode: 21, originalState: "bound" }); // default: false
  const tree = f.renderContent();
  assert.equal(
    visible(tree, f.pack["validatorReview.catalogueActivated"]),
    false,
    "catalogue activated label must not appear when false"
  );
});

// ── actionNotConnected does not mention internal plan names ──────────────────

test("en actionNotConnected message does not mention B1-06", () => {
  const pack = messages("en");
  assert.ok(
    !pack["validatorReview.actionNotConnected"].includes("B1-06"),
    "en actionNotConnected must not expose internal plan names to operators"
  );
});
