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
const { parseUnit } = require("../source-unit.js");
const {
  CONTROLLER: HTTP_CONTROLLER,
  CANONICAL_CONTRACT,
  actualGraph: actualHttpGraph,
  syntheticGraph: syntheticHttpGraph,
  assertRustTokens,
} = require("./fixtures/shared-http-mutation.js");

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

const HTTP_LOOP_MESSAGE = messages["async-await-in-loop"].replace(
  "while-loop",
  "for…of loop",
);
const HTTP_MUTATION_PREDICATE = "declared-shared-http-mutation-order";

function assertHttpMutationProof(f, diagnostic, requiredFiles, spans = []) {
  const proof = assertProof(f, diagnostic, HTTP_MUTATION_PREDICATE, spans);
  assert.match(proof.reason, /declar/iu);
  assert.match(proof.reason, /mutat/iu);
  assert.match(proof.reason, /independen/iu);
  for (const file of requiredFiles)
    assert.ok(
      proof.evidence.some((entry) => entry.file === file),
      `missing source-bound HTTP/contract witness: ${file}`,
    );
  return proof;
}

function assertHttpUnproved(f) {
  const result = f.evaluate();
  assert.equal(result.status, "requires-review");
  assert.equal(result.upstreamCorrections?.length ?? 0, 0);
}

function httpFixture(t, options) {
  const f = fixture(t, "export const pending = null;");
  const graph = syntheticHttpGraph(f, options);
  const diagnostic = f.add(
    "async-await-in-loop",
    graph.anchor,
    HTTP_LOOP_MESSAGE,
  );
  return { f, graph, diagnostic };
}

function replaceHttpSource(f, file, from, to) {
  const source = f.sources.get(file);
  assert.equal(
    source.split(from).length - 1,
    1,
    `unique fixture edit: ${from}`,
  );
  f.write(file, source.replace(from, to));
  if (/\.(?:ts|tsx)$/.test(file))
    assert.ok(
      parseUnit(f.repoRoot, file),
      `edited HTTP fixture parses: ${file}`,
    );
  if (file.endsWith(".rs")) assertRustTokens(f.sources.get(file));
}

for (const renamed of [false, true]) {
  semanticTest(
    `proves authoritative shared mutation with renamed contract ${renamed}`,
    (t) => {
      const { f, graph, diagnostic } = httpFixture(t, { renamed });
      assertHttpMutationProof(f, diagnostic, graph.required, [
        graph.anchor,
        graph.declaration,
      ]);
    },
  );
}

semanticTest(
  "conditional column mutation keeps its no-action identity branch",
  (t) => {
    const { f, graph } = httpFixture(t);
    f.doctorReport.diagnostics.length = 0;
    f.doctorReport.summary.totalDiagnosticCount = 0;
    const diagnostic = f.add(
      "async-await-in-loop",
      "await adapt(host, column.action)",
      HTTP_LOOP_MESSAGE,
    );
    assertHttpMutationProof(f, diagnostic, graph.required);
  },
);

semanticTest(
  "every actual shared-tree await has full current-source contract evidence",
  (t) => {
    const f = fixture(t, "export const pending = null;", HTTP_CONTROLLER);
    const required = actualHttpGraph(f);
    const source = f.sources.get(f.file);
    const anchors = [
      "await prepareTableAction(host!, next, action, fields)",
      "await prepareTableAction(host!, next, action, fields)",
      "await prepareTableAction(\n                  host!",
    ];
    let searchFrom = 0;
    for (const anchor of anchors) {
      const offset = source.indexOf(anchor, searchFrom);
      assert.ok(offset >= 0, `actual await anchor: ${anchor}`);
      searchFrom = offset + anchor.length;
      const before = source.slice(0, offset);
      f.doctorReport.diagnostics.push({
        filePath: f.file.slice("web/app/".length),
        plugin: "react-doctor",
        rule: "async-await-in-loop",
        severity: "warning",
        message: HTTP_LOOP_MESSAGE,
        line: before.split("\n").length,
        column: before.length - before.lastIndexOf("\n"),
      });
      f.doctorReport.summary.totalDiagnosticCount++;
    }
    const result = f.evaluate();
    assert.equal(result.status, "reviewed");
    assert.equal(result.findings.length, 0);
    assert.equal(result.upstreamCorrections.length, 3);
    for (const [index, proof] of result.upstreamCorrections.entries()) {
      assert.deepEqual(proof.diagnostic, f.doctorReport.diagnostics[index]);
      assert.equal(proof.predicate, HTTP_MUTATION_PREDICATE);
      assert.equal(
        proof.provenance.ruleSourceSha256,
        RULE_HASHES["async-await-in-loop"],
      );
      for (const file of required)
        assert.ok(
          proof.evidence.some((entry) => entry.file === file),
          `actual helper evidence ${file}`,
        );
      for (const entry of proof.evidence) {
        assert.equal(entry.sha256, sourceDigest(f.sources.get(entry.file)));
        assert.ok(entry.spans.length > 0);
        for (const span of entry.spans)
          assert.ok(
            span.start >= 0 &&
              span.start < span.end &&
              span.end <= f.sources.get(entry.file).length,
          );
      }
    }
  },
);

for (const [name, options, change] of [
  ["independent GET contract", { method: "GET", mutating: false }],
  ["pure POST search contract", { method: "POST", mutating: false }],
  [
    "CSRF true does not create a mutation contract",
    { mutating: false, csrf: true },
  ],
  ["CSRF flag conflicts with declaration", { mutating: true, csrf: false }],
  [
    "missing declaration",
    {},
    (f, g) =>
      f.write(g.files.declarations, "// No registered mutation declaration.\n"),
  ],
  [
    "shadowed declaration type",
    {},
    (f, g) =>
      replaceHttpSource(
        f,
        g.files.declarations,
        "use crate::routes::console_interface::{self, ConsoleInterfaceDeclaration, ConsoleInterfacePort};",
        "use crate::routes::console_interface::{self, ConsoleInterfacePort};\nstruct ConsoleInterfaceDeclaration { interface_id: &'static str, binding_id: &'static str, method: &'static str, path: &'static str, mutating: bool }",
      ),
  ],
  [
    "fake canonical import origin",
    {},
    (f, g) =>
      replaceHttpSource(
        f,
        g.files.facade,
        "pub use api_host_kernel::routes::console_interface::*;",
        "pub use pretend_kernel::routes::console_interface::*;",
      ),
  ],
  [
    "unconsumed declarations",
    {},
    (f, g) =>
      replaceHttpSource(
        f,
        g.files.declarations,
        '"fixture-graph", DECLARATIONS, port',
        '"fixture-graph", &[], port',
      ),
  ],
  [
    "contract method mismatch",
    {},
    (f, g) =>
      replaceHttpSource(
        f,
        g.files.declarations,
        'method: "POST"',
        'method: "PATCH"',
      ),
  ],
  [
    "contract path mismatch",
    {},
    (f, g) =>
      replaceHttpSource(
        f,
        g.files.declarations,
        `path: "${g.resource}"`,
        `path: "${g.resource}/different"`,
      ),
  ],
  [
    "registered route path mismatch",
    {},
    (f, g) =>
      replaceHttpSource(
        f,
        g.files.route,
        `route("/${g.names.collection}/:${g.names.key}/records"`,
        `route("/${g.names.collection}/:${g.names.key}/different-records"`,
      ),
  ],
  [
    "handler binding mismatch",
    {},
    (f, g) =>
      replaceHttpSource(
        f,
        g.files.route,
        `"${g.binding}", body`,
        '"http.unrelated.binding.v1", body',
      ),
  ],
  [
    "contradictory first declaration classification",
    {},
    (f, g) =>
      replaceHttpSource(
        f,
        g.files.declarations,
        "const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[",
        `const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[\n ConsoleInterfaceDeclaration { interface_id: "${g.interfaceId}", binding_id: "first.read", method: "GET", path: "/api/console/first-read", mutating: false },`,
      ),
  ],
  [
    "duplicate binding declaration",
    {},
    (f, g) =>
      replaceHttpSource(
        f,
        g.files.declarations,
        "const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[",
        `const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[\n ${g.declaration},`,
      ),
  ],
  [
    "dynamic declaration Boolean",
    {},
    (f, g) =>
      replaceHttpSource(
        f,
        g.files.declarations,
        g.declaration,
        g.declaration.replace("mutating: true", "mutating: UNKNOWN"),
      ),
  ],
  [
    "comment contains a fake mutation declaration",
    { mutating: false },
    (f, g) =>
      f.write(
        g.files.declarations,
        f.sources.get(g.files.declarations) +
          `\n/* ${g.declaration.replace("mutating: false", "mutating: true")} */\n`,
      ),
  ],
  [
    "string contains a fake declaration",
    { mutating: false },
    (f, g) =>
      f.write(
        g.files.declarations,
        f.sources.get(g.files.declarations) +
          `\nconst DECOY: &str = r###"${g.declaration.replace("mutating: false", "mutating: true")}"###;\n`,
      ),
  ],
  [
    "different owner keys for each item",
    {},
    (f, g) =>
      replaceHttpSource(
        f,
        g.files.provider,
        "creation.mutateAsync({ parent: identity, title: action.title })",
        "insert(action.owner, { parent: action.id, title: action.title })",
      ),
  ],
  [
    "provably disjoint mutation owners",
    {},
    (f, g) => {
      replaceHttpSource(
        f,
        g.files.provider,
        "creation.mutateAsync({ parent: identity, title: action.title })",
        "insert(action.owner, { parent: action.id, title: action.title })",
      );
      replaceHttpSource(
        f,
        f.file,
        "for (const action of next.actions)",
        "for (const action of [{ owner: 'alpha', id: 'a' }, { owner: 'beta', id: 'b' }])",
      );
    },
  ],
  [
    "canonical owner no longer classifies business mutations",
    {},
    (f) => {
      const source = f.sources.get(CANONICAL_CONTRACT);
      assert.equal(
        source.split("InterfaceAuditPolicy::Mutating").length - 1,
        2,
      );
      f.write(
        CANONICAL_CONTRACT,
        source.replaceAll(
          "InterfaceAuditPolicy::Mutating",
          "InterfaceAuditPolicy::ReadOnly",
        ),
      );
    },
  ],
  [
    "per-iteration host object",
    {},
    (f) =>
      replaceHttpSource(
        f,
        f.file,
        "\n      actions.push(await adapt(host, action));",
        "\n      actions.push(await adapt(makeHost(action), action));",
      ),
  ],
  [
    "unsafe-looking custom transport",
    {},
    (f, g) =>
      replaceHttpSource(
        f,
        g.files.transport,
        "export async function transmit",
        "const fetch = customTransport;\nexport async function transmit",
      ),
  ],
  [
    "dynamic transport method",
    {},
    (f, g) =>
      replaceHttpSource(
        f,
        g.files.requests,
        'method: "POST"',
        "method: input.method",
      ),
  ],
  [
    "opaque request options override",
    {},
    (f, g) =>
      replaceHttpSource(
        f,
        g.files.requests,
        "method: \"POST\", body: input, baseUrl: ''",
        "method: \"POST\", body: input, baseUrl: '', ...input.options",
      ),
  ],
  [
    "dynamic fetch replacement",
    {},
    (f, g) =>
      f.write(
        g.files.transport,
        'eval("globalThis.fetch = customTransport");\n' +
          f.sources.get(g.files.transport),
      ),
  ],
  [
    "unrelated mutation cannot justify awaited reads",
    {},
    (f, g) =>
      replaceHttpSource(
        f,
        g.files.helper,
        `return host.${g.names.operation}(action);`,
        "return independentRead(action);",
      ),
  ],
  [
    "infeasible mutation branch",
    {},
    (f, g) =>
      replaceHttpSource(
        f,
        g.files.provider,
        "if (!action.enabled) return action;",
        "if (true) return action;",
      ),
  ],
  [
    "contradictory mutation branch",
    {},
    (f, g) =>
      replaceHttpSource(
        f,
        g.files.provider,
        "if (!action.enabled) return action;",
        "if (action.enabled || !action.enabled) return action;",
      ),
  ],
  [
    "unobserved mutation output",
    {},
    (f, g) =>
      replaceHttpSource(
        f,
        g.files.helper,
        `await host.${g.names.store}(candidate);`,
        "return candidate;",
      ),
  ],
]) {
  semanticTest(`keeps shared HTTP mutation unproved: ${name}`, (t) => {
    const { f, graph } = httpFixture(t, options);
    change?.(f, graph);
    for (const [file, source] of f.sources) {
      if (/\.(?:ts|tsx)$/.test(file))
        assert.ok(
          parseUnit(f.repoRoot, file),
          `negative source parses: ${file}`,
        );
      if (file.endsWith(".rs")) assertRustTokens(source);
    }
    assertHttpUnproved(f);
  });
}

semanticTest(
  "shared mutation proof needs the exact pin and await position",
  (t) => {
    const { f, diagnostic } = httpFixture(t);
    assert.equal(f.evaluate("react-doctor@future").status, "requires-review");
    const original = structuredClone(diagnostic);
    for (const mutation of [
      { plugin: "other" },
      { rule: "other" },
      { message: `${HTTP_LOOP_MESSAGE} changed` },
      { column: original.column + 1 },
      { filePath: "../../outside.tsx" },
    ]) {
      Object.assign(diagnostic, original, mutation);
      assertHttpUnproved(f);
    }
  },
);

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

// These cases exercise Boolean influence and lifetime capability separately:
// reading a React ref is insufficient if the skip decision cannot change.
for (const [name, initial, writer, guard, expose = true, declarations = ""] of [
  [
    "QA constant short circuit",
    "0",
    "revision.current++",
    "true || revision.current",
  ],
  ["QA private invariant ref", "true", "", "revision.current", false],
  [
    "false and live read",
    "false",
    "revision.current = true",
    "false && revision.current",
  ],
  [
    "live read or true",
    "false",
    "revision.current = true",
    "revision.current || true",
  ],
  [
    "constant alias short circuit",
    "false",
    "revision.current = true",
    "always || revision.current",
    true,
    "const always = true;",
  ],
  [
    "correlated tautology",
    "false",
    "revision.current = true",
    "revision.current || !revision.current",
  ],
  [
    "correlated contradiction",
    "false",
    "revision.current = true",
    "revision.current && !revision.current",
  ],
  [
    "complementary comparisons",
    "0",
    "revision.current++",
    "revision.current !== snapshot || revision.current === snapshot",
  ],
  [
    "self comparison",
    "0",
    "revision.current++",
    "revision.current === revision.current",
  ],
  [
    "aliased self comparison",
    "0",
    "revision.current++",
    "alias.current === revision.current",
    true,
    "const alias = revision;",
  ],
  [
    "incompatible comparisons",
    "0",
    "revision.current++",
    "revision.current === 0 && revision.current === 1",
  ],
  [
    "dead private writer",
    "false",
    "revision.current = true",
    "revision.current",
    false,
  ],
  [
    "unreachable exposed writer",
    "false",
    "if (false) revision.current = true",
    "revision.current",
  ],
  ["same value writer", "true", "revision.current = true", "revision.current"],
  [
    "positive counter stays truthy",
    "1",
    "revision.current++",
    "revision.current",
  ],
  [
    "unknown predicate in dependency",
    "false",
    "revision.current = true",
    "opaque(revision.current)",
  ],
]) {
  semanticTest(`rejects ineffective completion dependency: ${name}`, (t) => {
    const f = fixture(
      t,
      `import { useRef } from 'react';
export function useOwner(load, commit) {
  const revision = useRef(${initial});
  ${declarations}
  const invalidate = () => { ${writer}; };
  async function run() {
    const snapshot = revision.current;
    const result = await load();
    if (${guard}) return;
    commit(result);
  }
  return ${expose ? "{ run, invalidate }" : "run"};
}`,
    );
    f.add("async-defer-await", "const result = await load();");
    const result = f.evaluate();
    assert.equal(result.status, "requires-review");
    assert.equal(result.upstreamCorrections?.length ?? 0, 0);
  });
}

for (const guard of [
  "revision.current",
  "false || revision.current",
  "true && revision.current",
]) {
  semanticTest(`retains effective counter truthiness: ${guard}`, (t) => {
    const f = fixture(
      t,
      `import { useRef } from 'react';
export function useOwner(load, commit) {
  const revision = useRef(0);
  const invalidate = () => { revision.current++; };
  async function run() {
    const result = await load();
    if (${guard}) return;
    commit(result);
  }
  return { run, invalidate };
}`,
    );
    const diagnostic = f.add(
      "async-defer-await",
      "const result = await load();",
    );
    assertProof(f, diagnostic, "completion-time-guard", [
      `if (${guard}) return;`,
    ]);
  });
}

semanticTest(
  "rejects a native controller with no reachable cancellation",
  (t) => {
    const f = fixture(
      t,
      `export function owner(load, commit) {
  const controller = new AbortController();
  const dispose = () => controller.abort();
  async function run() {
    const result = await load();
    if (controller.signal.aborted) return;
    commit(result);
  }
  return run;
}`,
    );
    f.add("async-defer-await", "const result = await load();");
    assert.equal(f.evaluate().status, "requires-review");
  },
);

// Read actual consumer code; anchors follow the current source rather than old
// diagnostic line numbers. No stored review entries authorize these controls.
const actualCompletionGuards = [
  [
    "features/agent-flow/hooks/runtime/useAgentFlowDebugSession.ts",
    "const detail = await fetchApplicationRunDebugSnapshot(",
  ],
  [
    "features/agent-flow/hooks/useEmbeddedAssistantSession.ts",
    "const conversation = await createConsoleAssistantConversation(",
  ],
  [
    "features/agent-flow/hooks/useEmbeddedAssistantSession.ts",
    "const detail = await fetchApplicationRunDebugSnapshot(",
  ],
  [
    "features/agent-flow/hooks/useEmbeddedAssistantSession.ts",
    "const conversation = await createConsoleAssistantConversation(",
    1,
  ],
  [
    "features/agent-flow/hooks/useEmbeddedAssistantSession.ts",
    "const detail = await cancelConsoleFlowRun(",
  ],
  [
    "features/auth/components/PublicAuthBlock.tsx",
    "const prepared = await prepareNativeReactSource(",
  ],
  [
    "features/frontstage/components/files/FileTableView.tsx",
    "const result = await uploadConsoleFile(",
  ],
  [
    "features/frontstage/components/jsx-studio/JsxStudioRunPanel.tsx",
    "const prepared = await prepareNativeReactSource(",
  ],
  [
    "features/frontstage/components/jsx-studio/JsxStudioRunPanel.tsx",
    "const catalog = await fetchFrontstageBlockCatalog();",
  ],
  [
    "features/frontstage/pages/frontstage-page/controllers/use-frontstage-page-refresh.ts",
    "await onRefreshPage();",
  ],
  [
    "features/settings/components/mcp-management/McpInstancesTab/use-directory-draft.ts",
    "const saved = await replaceSettingsMcpInstanceDirectory(",
  ],
  [
    "features/settings/components/record-permissions/use-record-permission-session.ts",
    "await replaceRecordPermissions(",
  ],
  [
    "features/settings/pages/notifications/NotificationAuthorization.tsx",
    "const result = await authorizeNotificationInstance(",
  ],
  [
    "features/settings/pages/settings-page/useExtensionCenterController.tsx",
    "const results = await Promise.all(",
  ],
  ["features/workflow/hooks/use-workflow-test-run.ts", "await presentation;"],
  [
    "features/workflow/hooks/use-workflow-test-run.ts",
    "const detail = await fetchApplicationRunDebugSnapshot(",
  ],
];
for (const [relative, anchor, occurrence = 0] of actualCompletionGuards) {
  semanticTest(
    `retains actual completion guard ${relative} ${anchor} #${occurrence}`,
    (t) => {
      const file = `web/app/src/${relative}`;
      const source = fs.readFileSync(
        path.resolve(__dirname, "../../../..", file),
        "utf8",
      );
      const f = fixture(t, source, file);
      const diagnostic = f.add("async-defer-await", anchor);
      if (occurrence) {
        let offset = -1;
        for (let index = 0; index <= occurrence; index++)
          offset = source.indexOf(anchor, offset + 1);
        assert.ok(offset >= 0, "the actual diagnostic occurrence must exist");
        const before = source.slice(0, offset);
        diagnostic.line = before.split("\n").length;
        diagnostic.column = before.length - before.lastIndexOf("\n");
      }
      assertProof(f, diagnostic, "completion-time-guard", []);
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
