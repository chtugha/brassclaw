// Controlled JS transport pages only: no backend/SQL/approval acceptance.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

function fixture(reply) {
  const calls = [];
  const context = {
    URLSearchParams,
    apiFetch: async path => {
      const url = new URL(path, "https://ui.invalid");
      assert.equal(url.pathname, "/api/webchat/v2/validation-queue");
      assert.equal(url.searchParams.get("project_id"), "default");
      const call = { bucket: url.searchParams.get("q"), after: url.searchParams.get("after"), path };
      assert.ok(["auto", "manual"].includes(call.bucket));
      assert.deepEqual([...url.searchParams.keys()].sort(), call.after === null ? ["project_id", "q"] : ["after", "project_id", "q"]);
      calls.push(call);
      return reply(call);
    },
  };
  const source = readFileSync(new URL("./settings-api.js", import.meta.url), "utf8")
    .split("\n").filter(line => !line.startsWith("import ")).join("\n").replaceAll("export ", "");
  vm.createContext(context);
  vm.runInContext(source + "\nglobalThis.readQueue = fetchValidationQueue;", context);
  return { read: () => context.readQueue(), calls };
}
const time = second => `2026-11-04T00:00:${String(second).padStart(2, "0")}.000000000Z`;
const row = (classCode, id, second, extra = {}) => ({ class_code: classCode, id, queued_at: time(second), created_at: time(59), ...extra });
const plain = value => JSON.parse(JSON.stringify(value));

test("both buckets follow every opaque cursor and merge all classes in global order", async () => {
  const manualFirst = "manual cursor/+==&?";
  const manualSecond = "manual-final";
  const automaticNext = "automatic/+==";
  const pages = {
    manual: new Map([
      [null, { items: [row(21, "shared", 3, { phase: "manual-current" }), row(21, "recipe-last", 6)], page_size: 2, next_cursor: manualFirst }],
      [manualFirst, { items: [], page_size: 2, next_cursor: manualSecond }],
      [manualSecond, { items: [row(21, "recipe-first", 2)], page_size: 2, next_cursor: null }],
    ]),
    auto: new Map([
      [null, { items: [row(22, "shared", 3), row(0, "tool-first", 1)], page_size: 2, next_cursor: automaticNext }],
      [automaticNext, { items: [
        row(21, "shared", 3, { phase: "auto-before-transition" }),
        row(13, "binding-z", 4), row(13, "binding-a", 4),
        { class_code: 1, id: "older-wire-without-queued-at", created_at: time(5) },
      ], page_size: 4 }],
    ]),
  };
  const f = fixture(({ bucket, after }) => {
    assert.ok(pages[bucket].has(after), `unexpected ${bucket} cursor ${after}`);
    return pages[bucket].get(after);
  });
  const result = await f.read();
  assert.deepEqual(f.calls.filter(c => c.bucket === "manual").map(c => c.after), [null, manualFirst, manualSecond]);
  assert.deepEqual(f.calls.filter(c => c.bucket === "auto").map(c => c.after), [null, automaticNext]);
  assert.deepEqual(plain(result.items).map(r => `${r.class_code}:${r.id}`), [
    "0:tool-first", "21:recipe-first", "21:shared", "22:shared", "13:binding-a", "13:binding-z", "1:older-wire-without-queued-at", "21:recipe-last",
  ]);
  assert.equal(result.items.filter(r => r.class_code === 21 && r.id === "shared").length, 1, "a live bucket transition must not duplicate an identity");
  assert.equal(result.items.find(r => r.class_code === 21 && r.id === "shared").phase, "manual-current");
  assert.equal(result.items.filter(r => r.id === "shared").length, 2, "class-qualified identities cannot displace each other");
});

for (const failedBucket of ["manual", "auto"]) {
  test(`${failedBucket} failure after a loaded page rejects the entire fetch without a partial success`, async () => {
    const failure = Object.assign(Error(`${failedBucket} page unavailable`), { status: 503 });
    const f = fixture(({ bucket, after }) => {
      if (after === null) return { items: [row(bucket === "manual" ? 21 : 22, `${bucket}-prefix`, 1)], next_cursor: `${bucket}-next` };
      assert.equal(after, `${bucket}-next`);
      if (bucket === failedBucket) throw failure;
      return { items: [row(13, `${bucket}-last`, 2)], next_cursor: null };
    });
    await assert.rejects(f.read(), error => error === failure);
    assert.deepEqual(f.calls.filter(c => c.bucket === failedBucket).map(c => c.after), [null, `${failedBucket}-next`]);
  });
}

test("a cursor that does not advance is an error after a populated page", async () => {
  let manualCalls = 0;
  const f = fixture(({ bucket, after }) => {
    if (bucket === "auto") return { items: [], next_cursor: null };
    manualCalls++;
    if (manualCalls > 2) throw Error("unexpected repeated cursor request");
    return { items: [row(21, `manual-${manualCalls}`, manualCalls)], next_cursor: "stuck-cursor" };
  });
  await assert.rejects(f.read(), /Queue page did not advance/);
  assert.equal(manualCalls, 2);
  assert.deepEqual(f.calls.filter(c => c.bucket === "manual").map(c => c.after), [null, "stuck-cursor"]);
});
