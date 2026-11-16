const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const {
  REVIEW_FILE,
  sourceDigest,
  evaluateDiagnosticReviews,
} = require("../diagnostic-review.js");

const TOOL = "react-doctor@0.2.16";
const SOURCE_COMMIT = "2d1ab245f0cbaa87529067e6e3ed9ac08362d1fc";
const RULE_HASHES = {
  "async-defer-await":
    "144e708975fff242fadb84c0ac18a2fe8fc2f5617925ccbd28ef5e8629894c5e",
  "async-parallel":
    "80e720fab08e07949dd2a08672937c7292b97a728b9759303520dd6942799550",
  "async-await-in-loop":
    "de85fed2c25b0d4bb26b1e9ef167c6e314e42bdc9000a146889659d31f0f0bc6",
};
const messages = {
  "async-defer-await":
    "This await blocks the function before an early-return that doesn't use the awaited value, so the skip path waits for nothing. Move the await below the guard so it only runs when you need the data",
  "async-parallel":
    "These 3 sequential await statements run one after another even though they look independent, so the page waits longer than it needs to. Run them together with Promise.all() instead",
  "async-await-in-loop":
    "This makes the while-loop slow because each await runs one after another, so collect the independent calls & run them together with `await Promise.all(items.map(...))`",
};
const semanticTest = (name, fn) =>
  test(`async semantic correction: ${name}`, fn);

function fixture(t, source, file = "web/app/src/arbitrary-owner.tsx") {
  const repoRoot = fs.mkdtempSync(
    path.join(os.tmpdir(), "doctor-async-proof-"),
  );
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  const sources = new Map();
  const write = (name, body) => {
    fs.mkdirSync(path.dirname(path.join(repoRoot, name)), { recursive: true });
    fs.writeFileSync(path.join(repoRoot, name), body);
    sources.set(name, body);
  };
  write(file, source);
  const manifest = { schemaVersion: 1, toolPackage: TOOL, reviews: [] };
  write(REVIEW_FILE, JSON.stringify(manifest));
  const doctorReport = {
    diagnostics: [],
    summary: { totalDiagnosticCount: 0, suppressedDiagnosticCount: 0 },
    error: null,
  };
  const add = (rule, anchor, message = messages[rule]) => {
    const offset = sources.get(file).indexOf(anchor);
    assert.ok(offset >= 0, `missing diagnostic anchor ${anchor}`);
    const before = sources.get(file).slice(0, offset);
    const diagnostic = {
      filePath: file.slice("web/app/".length),
      plugin: "react-doctor",
      rule,
      severity: "warning",
      message,
      line: before.split("\n").length,
      column: before.length - before.lastIndexOf("\n"),
    };
    doctorReport.diagnostics.push(diagnostic);
    doctorReport.summary.totalDiagnosticCount++;
    return diagnostic;
  };
  const evaluate = (toolPackage = TOOL) => {
    const raw = structuredClone(doctorReport);
    const result = evaluateDiagnosticReviews({
      repoRoot,
      doctorReport,
      toolPackage,
    });
    assert.deepEqual(
      doctorReport,
      raw,
      "retain raw diagnostics, counts, positions, messages and severities",
    );
    assert.deepEqual(
      JSON.parse(fs.readFileSync(path.join(repoRoot, REVIEW_FILE))),
      manifest,
      "no source identity, method or rule exception may be added",
    );
    return result;
  };
  return { repoRoot, file, sources, write, doctorReport, add, evaluate };
}

function assertProof(f, diagnostic, predicate, spans) {
  const result = f.evaluate();
  assert.equal(result.status, "reviewed");
  assert.equal(result.findings.length, 0);
  assert.equal(result.upstreamCorrections.length, 1);
  const proof = result.upstreamCorrections[0];
  assert.deepEqual(proof.diagnostic, diagnostic);
  assert.equal(proof.predicate, predicate);
  assert.equal(proof.provenance.toolPackage, TOOL);
  assert.equal(proof.provenance.sourceCommit, SOURCE_COMMIT);
  assert.equal(proof.provenance.ruleSourceSha256, RULE_HASHES[diagnostic.rule]);
  assert.match(proof.provenance.packageIntegrity, /^sha512-/u);
  assert.ok(
    proof.provenance.references.some((url) =>
      url.startsWith("https://github.com/millionco/react-doctor/"),
    ),
  );
  for (const evidence of proof.evidence) {
    const source = f.sources.get(evidence.file);
    assert.equal(typeof source, "string");
    assert.equal(evidence.sha256, sourceDigest(source));
    assert.ok(evidence.spans.length > 0);
    for (const span of evidence.spans) {
      assert.ok(Number.isInteger(span.start) && Number.isInteger(span.end));
      assert.ok(
        span.start >= 0 && span.start < span.end && span.end <= source.length,
      );
    }
  }
  for (const text of spans) {
    assert.ok(
      proof.evidence.some((evidence) =>
        evidence.spans.some(
          (span) =>
            f.sources.get(evidence.file).slice(span.start, span.end) === text,
        ),
      ),
      `missing exact proof span: ${text}`,
    );
  }
  return proof;
}

for (const [refName, ticketName] of [
  ["generation", "ticket"],
  ["cursor", "receipt"],
]) {
  semanticTest(`proves renamed ref snapshot freshness ${refName}`, (t) => {
    const guard = `if (${ticketName} !== ${refName}.current) return;`;
    const source = `import { useRef as makeReference } from 'react';
export function useOwner(load, commit) {
  const ${refName} = makeReference(0);
  const invalidate = () => { ${refName}.current++; };
  async function save() {
    const ${ticketName} = ${refName}.current;
    const saved = await load();
    ${guard}
    commit(saved);
  }
  return { save, invalidate };
}`;
    const f = fixture(t, source);
    const diagnostic = f.add(
      "async-defer-await",
      "const saved = await load();",
    );
    assertProof(f, diagnostic, "completion-time-guard", [
      guard,
      `makeReference(0)`,
    ]);
  });
}

semanticTest(
  "follows a stable local predicate instead of trusting its spelling",
  (t) => {
    const predicate = "const acceptable = () => live.current === token;";
    const guard = "if (!acceptable()) return;";
    const f = fixture(
      t,
      `import * as R from 'react';
export function owner(load, commit) {
  const token = {};
  const live = R.useRef(token);
  const acceptable = () => live.current === token;
  const replace = () => { live.current = {}; };
  async function run() {
    await load();
    ${guard}
    commit();
  }
  return { run, replace };
}`,
    );
    const diagnostic = f.add("async-defer-await", "await load();");
    assertProof(f, diagnostic, "completion-time-guard", [guard, predicate]);
  },
);

semanticTest(
  "recognizes a native abort owner and its cleanup rather than a name",
  (t) => {
    const guard = "if (request.signal.aborted) return;";
    const f = fixture(
      t,
      `export function owner(write, commit) {
  const request = new AbortController();
  const dispose = () => request.abort();
  async function run() {
    await write();
    ${guard}
    commit();
  }
  return { run, dispose };
}`,
    );
    const diagnostic = f.add("async-defer-await", "await write();");
    assertProof(f, diagnostic, "completion-time-guard", [
      guard,
      "new AbortController()",
    ]);
  },
);

semanticTest(
  "follows the native abort controller captured through a local React ref",
  (t) => {
    const guard = "if (controller.signal.aborted) return;";
    const f = fixture(
      t,
      `import { useEffect, useRef } from 'react';
export function owner(write, commit) {
  const request = useRef(null);
  useEffect(() => {
    const controller = new AbortController();
    request.current = controller;
    return () => controller.abort();
  }, []);
  async function run() {
    const controller = request.current;
    if (!controller) return;
    await write();
    ${guard}
    commit();
  }
  return { run };
}`,
    );
    const diagnostic = f.add("async-defer-await", "await write();");
    assertProof(f, diagnostic, "completion-time-guard", [
      guard,
      "new AbortController()",
    ]);
  },
);

semanticTest(
  "accepts a captured scalar that another closure can invalidate",
  (t) => {
    const guard = "if (snapshot !== revision) return;";
    const f = fixture(
      t,
      `export function owner(load, commit) {
  let revision = 0;
  const invalidate = () => { revision++; };
  async function run() {
    const snapshot = revision;
    const result = await load();
    ${guard}
    commit(result);
  }
  return { run, invalidate };
}`,
    );
    const diagnostic = f.add(
      "async-defer-await",
      "const result = await load();",
    );
    assertProof(f, diagnostic, "completion-time-guard", [guard, "revision++"]);
  },
);

for (const [name, declarations, guard] of [
  ["invariant preflight", "const enabled = allowed;", "if (!enabled) return;"],
  [
    "immutable comparison",
    "const revision = 2; const snapshot = revision;",
    "if (snapshot !== revision) return;",
  ],
  [
    "fake ref property",
    "const revision = { current: 2 }; const snapshot = revision.current;",
    "if (snapshot !== revision.current) return;",
  ],
  [
    "opaque predicate",
    "const acceptable = () => externalCheck();",
    "if (!acceptable()) return;",
  ],
  [
    "shadowed React factory",
    "const makeReference = value => ({ current: value }); const revision = makeReference(2); const snapshot = revision.current;",
    "if (snapshot !== revision.current) return;",
  ],
  [
    "shadowed native constructor",
    "const AbortController = Factory; const request = new AbortController();",
    "if (request.signal.aborted) return;",
  ],
]) {
  semanticTest(
    `keeps genuine or unproved defer case unresolved: ${name}`,
    (t) => {
      const f = fixture(
        t,
        `export function owner(load, allowed, commit, Factory) {
      ${declarations}
      async function run() { const result = await load(); ${guard} commit(result); }
      return run;
    }`,
      );
      f.add("async-defer-await", "const result = await load();");
      assert.equal(f.evaluate().status, "requires-review");
      assert.equal(f.evaluate().upstreamCorrections?.length ?? 0, 0);
    },
  );
}

for (const [name, declaration, later] of [
  [
    "bare await argument",
    "const original = await load();",
    "await write({ document: project(original) });",
  ],
  [
    "destructured awaited binding",
    "const { original } = await load();",
    "await write(original);",
  ],
  [
    "transparent TypeScript access",
    "const original = await load();",
    "await write((original as { key: string }).key);",
  ],
  [
    "member receiver",
    "const original = await load();",
    "await original.flush();",
  ],
  [
    "genuine callback capture",
    "const original = await load();",
    "await write(items.map(item => ({ item, original })));",
  ],
]) {
  semanticTest(`proves actual await result dependence: ${name}`, (t) => {
    const f = fixture(
      t,
      `export async function owner(load, write, finish, project, items) {
      ${declaration}
      ${later}
      await finish();
    }`,
    );
    const diagnostic = f.add("async-parallel", declaration);
    assertProof(f, diagnostic, "await-data-dependency", [declaration, later]);
  });
}

for (const [name, body] of [
  [
    "independent bound results",
    "const original = await load(); const other = await different(); const last = await finish(); return [original, other, last];",
  ],
  [
    "callback parameter shadows prior result",
    "const original = await load(); await write(items.map(original => original.value)); await finish();",
  ],
  [
    "nested binding shadows prior result",
    "const original = await load(); await write(items.map(item => { const original = item; return original.value; })); await finish();",
  ],
  [
    "write-only reference is not a read",
    "let original = await load(); await write(() => { original = 42; }); await finish();",
  ],
  [
    "opaque side-effect port is not proof",
    "await port.first(); await port.second(); await port.third();",
  ],
]) {
  semanticTest(
    `keeps genuine or unproved parallel case unresolved: ${name}`,
    (t) => {
      const f = fixture(
        t,
        `export async function owner(load, different, write, finish, items, port) { ${body} }`,
      );
      f.add(
        "async-parallel",
        name === "opaque side-effect port is not proof"
          ? "await port.first();"
          : name === "write-only reference is not a read"
            ? "let original = await load();"
            : "const original = await load();",
      );
      assert.equal(f.evaluate().status, "requires-review");
      assert.equal(f.evaluate().upstreamCorrections?.length ?? 0, 0);
    },
  );
}

for (const [name, branch] of [
  ["break", "if (snapshot.done) break;"],
  ["function return", "if (snapshot.done) return snapshot;"],
  [
    "derived terminal test",
    "const status = normalize(snapshot); if (!isActive(status)) return;",
  ],
]) {
  semanticTest(`proves result-dependent polling ${name}`, (t) => {
    const f = fixture(
      t,
      `export async function owner(read, pause, normalize, isActive) {
      while (true) {
        const snapshot = await read();
        ${branch}
        await pause();
      }
    }`,
    );
    const diagnostic = f.add("async-await-in-loop", "await read()");
    assertProof(f, diagnostic, "await-loop-control-dependency", [
      "await read()",
      branch.slice(branch.indexOf("if")),
    ]);
  });
}

for (const [name, body] of [
  [
    "independent result collection",
    "for (const id of ids) { const snapshot = await read(id); output.push(snapshot); }",
  ],
  [
    "unrelated early exit",
    "while (true) { const snapshot = await read(); if (stop) break; output.push(snapshot); }",
  ],
  [
    "shadowed terminal callback parameter",
    "while (true) { const snapshot = await read(); const done = items.some(snapshot => snapshot.done); if (done) break; output.push(snapshot); }",
  ],
  [
    "nested function return does not stop loop",
    "for (const id of ids) { const snapshot = await read(id); const check = () => { if (snapshot.done) return; }; check(); output.push(snapshot); }",
  ],
  [
    "inner loop break does not stop outer loop",
    "for (const id of ids) { const snapshot = await read(id); for (const item of items) { if (snapshot.done) break; } output.push(snapshot); }",
  ],
]) {
  semanticTest(
    `keeps genuine or unproved loop case unresolved: ${name}`,
    (t) => {
      const f = fixture(
        t,
        `export async function owner(ids, read, output, stop, items) { ${body} }`,
      );
      const forLoop = body.startsWith("for");
      const message = messages["async-await-in-loop"].replace(
        "while-loop",
        forLoop ? "for…of loop" : "while-loop",
      );
      f.add(
        "async-await-in-loop",
        forLoop ? "await read(id)" : "await read()",
        message,
      );
      assert.equal(f.evaluate().status, "requires-review");
      assert.equal(f.evaluate().upstreamCorrections?.length ?? 0, 0);
    },
  );
}

semanticTest(
  "source changes invalidate dependency proof without changing diagnostic identity",
  (t) => {
    const prefix = "export async function owner(load, write, finish, value) {";
    const f = fixture(
      t,
      `${prefix} const original = await load(); await write(original); await finish(); }`,
    );
    const diagnostic = f.add(
      "async-parallel",
      "const original = await load();",
    );
    assertProof(f, diagnostic, "await-data-dependency", [
      "await write(original);",
    ]);
    f.write(
      f.file,
      `${prefix} const original = await load(); await write(value); await finish(); }`,
    );
    assert.equal(f.evaluate().status, "requires-review");
  },
);

semanticTest(
  "fails closed on wrong tool, message, location, path or source syntax",
  (t) => {
    const source = `import { useRef } from 'react'; function owner(load) { const state = useRef(0); async function run() { const token = state.current; const value = await load(); if (token !== state.current) return; use(value); } return run; }`;
    const f = fixture(t, source);
    const diagnostic = f.add(
      "async-defer-await",
      "const value = await load();",
    );
    assert.equal(f.evaluate("react-doctor@future").status, "requires-review");
    const original = structuredClone(diagnostic);
    for (const mutation of [
      { plugin: "different" },
      { rule: "different" },
      { message: "unknown revision" },
      { line: 99999 },
      { column: 99999 },
      { filePath: "../../outside.ts" },
    ]) {
      Object.assign(diagnostic, original, mutation);
      assert.equal(f.evaluate().status, "requires-review");
    }
    Object.assign(diagnostic, original);
    f.write(f.file, "export function invalid(");
    assert.equal(f.evaluate().status, "requires-review");
  },
);

semanticTest(
  "runtime counterexample: a pre-await check accepts a stale completion",
  async () => {
    async function observe(checkAfter) {
      const revision = { current: 0 };
      const committed = [];
      let release;
      const request = new Promise((resolve) => {
        release = resolve;
      });
      const run = async () => {
        const snapshot = revision.current;
        if (!checkAfter && snapshot !== revision.current) return;
        const result = await request;
        if (checkAfter && snapshot !== revision.current) return;
        committed.push(result);
      };
      const pending = run();
      revision.current++;
      release("old response");
      await pending;
      return committed;
    }
    assert.deepEqual(await observe(true), []);
    assert.deepEqual(await observe(false), ["old response"]);
  },
);

semanticTest(
  "runtime counterexample: result-dependent termination prevents later polling requests",
  async () => {
    const requested = [];
    const read = async (id) => {
      requested.push(id);
      return { done: true };
    };
    for (const id of [1, 2, 3]) {
      const snapshot = await read(id);
      if (snapshot.done) break;
    }
    assert.deepEqual(requested, [1]);
    requested.length = 0;
    await Promise.all([1, 2, 3].map(read));
    assert.deepEqual(requested, [1, 2, 3]);
  },
);
