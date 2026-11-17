const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { createRequire } = require("node:module");
const {
  REVIEW_FILE,
  sourceDigest,
  evaluateDiagnosticReviews,
} = require("../diagnostic-review.js");

const TOOL = "react-doctor@0.2.16";
const RULE = "exhaustive-deps";
const RULE_HASH =
  "39f93c68922a19ed90687c21bb01ba2cb62daa9a7dab39ccabc8c08ac2ac3f48";
const SOURCE_COMMIT = "2d1ab245f0cbaa87529067e6e3ed9ac08362d1fc";
const REACT_COMMIT = "976e87ce22e417ae87395d4d0a5488eba3b07b02";
const messageFor = (ref) =>
  "Your cleanup may read the wrong node since the ref `" +
  ref +
  ".current` can change before it runs.";
const semanticTest = (name, fn) =>
  test(`resource semantic correction: ${name}`, fn);

const OBSERVER_RULE = "no-pass-live-state-to-parent";
const OBSERVER_RULE_HASH =
  "c1ccf61ff705ec90f43e8b6893a0f8e64937af0704683c977157f260222996d8";
const OBSERVER_MESSAGE =
  "Pushing state up to a parent from a useEffect costs your users an extra render.";

function fixture(t, source) {
  const repoRoot = fs.mkdtempSync(
    path.join(os.tmpdir(), "doctor-resource-proof-"),
  );
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  const file = "web/app/src/unrelated-resource-owner.tsx";
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
  const add = (ref = "owned", anchor = "() => { return () =>") => {
    const offset = sources.get(file).indexOf(anchor);
    assert.ok(offset >= 0, `missing diagnostic anchor: ${anchor}`);
    const before = sources.get(file).slice(0, offset);
    const diagnostic = {
      filePath: file.slice("web/app/".length),
      plugin: "react-doctor",
      rule: RULE,
      severity: "warning",
      message: messageFor(ref),
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
      "retain complete raw diagnostics, message, position and severity",
    );
    assert.deepEqual(
      JSON.parse(fs.readFileSync(path.join(repoRoot, REVIEW_FILE))),
      manifest,
      "never add a file, symbol, method or diagnostic exception",
    );
    return result;
  };
  return { repoRoot, file, sources, write, doctorReport, add, evaluate };
}

function assertProof(f, diagnostic, exactSpans) {
  const result = f.evaluate();
  assert.equal(result.status, "reviewed");
  assert.equal(result.findings.length, 0);
  assert.equal(result.upstreamCorrections.length, 1);
  const proof = result.upstreamCorrections[0];
  assert.deepEqual(proof.diagnostic, diagnostic);
  assert.equal(proof.predicate, "owned-current-resource-cleanup");
  assert.equal(proof.provenance.toolPackage, TOOL);
  assert.equal(proof.provenance.sourceCommit, SOURCE_COMMIT);
  assert.equal(proof.provenance.ruleSourceSha256, RULE_HASH);
  assert.match(proof.provenance.packageIntegrity, /^sha512-/u);
  assert.ok(
    proof.provenance.references.some((url) =>
      url.includes(`/facebook/react/blob/${REACT_COMMIT}/`),
    ),
  );
  assert.ok(proof.evidence.length > 0);
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
  for (const span of exactSpans)
    assert.ok(
      proof.evidence.some((entry) =>
        entry.spans.some(
          ({ start, end }) =>
            f.sources.get(entry.file).slice(start, end) === span,
        ),
      ),
      `missing exact evidence span: ${span}`,
    );
  return proof;
}
function assertUnproved(f) {
  const result = f.evaluate();
  assert.equal(result.status, "requires-review");
  assert.equal(result.upstreamCorrections?.length ?? 0, 0);
}

function observationSource({
  imports = "import { useEffect, useRef, useState } from 'react';",
  before = "",
  parameters = "{ notify, ready }",
  hook = "useEffect",
  setup = "",
  constructor = "IntersectionObserver",
  callback = "entries => { if (entries.some(entry => entry.isIntersecting)) void notify(); }",
  options = "{ root: viewport.current, rootMargin: '200px' }",
  registration = "if (target) receipt.observe(target);",
  cleanup = "return () => receipt.disconnect();",
  after = "",
} = {}) {
  return `${imports}
${before}
function UnrelatedVisibilityOwner(${parameters}) {
  const viewport = useRef(null);
  const [target, setTarget] = useState(null);
  ${hook}(() => {
    if (!ready) return;
    ${setup}
    const receipt = new ${constructor}(${callback}, ${options});
    ${registration}
    ${cleanup}
  }, [target, ready, notify]);
  ${after}
  return <div ref={viewport}><div ref={setTarget} /></div>;
}`;
}

function addObservation(f, anchor = "receipt.observe(target)") {
  return Object.assign(f.add("unused", anchor), {
    rule: OBSERVER_RULE,
    message: OBSERVER_MESSAGE,
  });
}

function assertObservationProof(f, diagnostic, exactSpans = []) {
  const result = f.evaluate();
  assert.equal(result.status, "reviewed");
  assert.equal(result.findings.length, 0);
  assert.equal(result.upstreamCorrections.length, 1);
  const proof = result.upstreamCorrections[0];
  assert.deepEqual(proof.diagnostic, diagnostic);
  assert.equal(proof.predicate, "native-intersection-observation-registration");
  assert.equal(proof.provenance.toolPackage, TOOL);
  assert.equal(proof.provenance.sourceCommit, SOURCE_COMMIT);
  assert.equal(proof.provenance.ruleSourceSha256, OBSERVER_RULE_HASH);
  assert.match(proof.provenance.packageIntegrity, /^sha512-/u);
  assert.ok(
    proof.provenance.references.some((url) =>
      url.includes("w3c.github.io/IntersectionObserver"),
    ),
  );
  assert.ok(
    proof.provenance.references.some(
      (url) =>
        url.includes(`/${SOURCE_COMMIT}/`) &&
        url.endsWith("/no-pass-live-state-to-parent.ts"),
    ),
  );
  assert.match(proof.reason, /registra/iu);
  assert.match(proof.reason, /defer|asynchronous/iu);
  assert.ok(proof.evidence.length > 0);
  for (const entry of proof.evidence) {
    const source = f.sources.get(entry.file);
    assert.equal(typeof source, "string");
    assert.equal(entry.sha256, sourceDigest(source));
    assert.ok(entry.spans.length > 0);
    for (const span of entry.spans)
      assert.ok(
        Number.isInteger(span.start) &&
          Number.isInteger(span.end) &&
          span.start >= 0 &&
          span.start < span.end &&
          span.end <= source.length,
      );
  }
  for (const span of exactSpans)
    assert.ok(
      proof.evidence.some((entry) =>
        entry.spans.some(
          ({ start, end }) =>
            f.sources.get(entry.file).slice(start, end) === span,
        ),
      ),
      `missing exact observation evidence: ${span}`,
    );
  return proof;
}

for (const [name, options] of [
  ["native constructor and state-held target", {}],
  [
    "renamed React effect import",
    {
      imports:
        "import { useEffect as synchronize, useRef, useState } from 'react';",
      hook: "synchronize",
    },
  ],
  [
    "namespace React effect",
    {
      imports:
        "import * as React from 'react'; import { useRef, useState } from 'react';",
      hook: "React.useEffect",
    },
  ],
  [
    "independent event handler can notify outside the effect",
    { after: "const loadFromScroll = () => notify();" },
  ],
]) {
  semanticTest(`native observation registration: ${name}`, (t) => {
    const f = fixture(t, observationSource(options));
    const diagnostic = addObservation(f);
    assertObservationProof(f, diagnostic, [
      "receipt.observe(target)",
      "receipt.disconnect()",
      "entries => { if (entries.some(entry => entry.isIntersecting)) void notify(); }",
      "new IntersectionObserver(entries => { if (entries.some(entry => entry.isIntersecting)) void notify(); }, { root: viewport.current, rootMargin: '200px' })",
    ]);
  });
}

semanticTest(
  "current catalog source proves registration at an unrelated fixture path",
  (t) => {
    const source = fs.readFileSync(
      path.resolve(
        __dirname,
        "../../../../web/app/src/features/settings/components/extension-center/ApplicationTemplateCatalogList.tsx",
      ),
      "utf8",
    );
    const f = fixture(t, source);
    const diagnostic = addObservation(f, "observer.observe(sentinel)");
    const proof = assertObservationProof(f, diagnostic, [
      "observer.observe(sentinel)",
      "observer.disconnect()",
    ]);
    assert.equal(proof.evidence.length, 1);
    assert.equal(proof.evidence[0].file, f.file);
    assert.equal(proof.evidence[0].sha256, sourceDigest(source));
  },
);

for (const [name, options] of [
  ["opaque same-named observe method", { constructor: "UnknownObserver" }],
  [
    "locally shadowed eager constructor",
    {
      before:
        "class IntersectionObserver { constructor(callback) { callback([]); } observe() {} disconnect() {} }",
    },
  ],
  [
    "imported constructor",
    {
      imports:
        "import { useEffect, useRef, useState } from 'react'; import { IntersectionObserver } from './foreign';",
    },
  ],
  [
    "constructor parameter",
    { parameters: "{ notify, ready, IntersectionObserver }" },
  ],
  [
    "aliased native constructor",
    {
      before: "const NativeObserver = IntersectionObserver;",
      constructor: "NativeObserver",
    },
  ],
  ["unknown callback reference", { callback: "notify" }],
  [
    "IIFE callback argument",
    { callback: "(() => { notify(); return entries => {}; })()" },
  ],
  ["options execute during setup", { options: "makeOptions(notify)" }],
  ["options getter", { options: "{ get root() { notify(); return null; } }" }],
  ["options spread", { options: "{ ...foreignOptions }" }],
  ["computed option key", { options: "{ [chooseOption()]: null }" }],
  ["immediate parent notification", { setup: "notify(target);" }],
  ["immediate state update", { setup: "setTarget(viewport.current);" }],
  ["opaque setup executor", { setup: "runUnknown(notify, target);" }],
  [
    "immediate nested callback execution",
    { setup: "(() => notify(target))();" },
  ],
  [
    "observer instance escapes to helper",
    { registration: "publish(receipt); if (target) receipt.observe(target);" },
  ],
  [
    "observer method escapes to helper",
    {
      registration:
        "publish(receipt.observe); if (target) receipt.observe(target);",
    },
  ],
  [
    "observer alias escapes",
    {
      registration:
        "const alias = receipt; publish(alias); if (target) receipt.observe(target);",
    },
  ],
  [
    "observer method replaced",
    {
      registration:
        "receipt.observe = notify; if (target) receipt.observe(target);",
    },
  ],
  [
    "receiver reassigned",
    {
      registration:
        "receipt = foreignObserver; if (target) receipt.observe(target);",
    },
  ],
  ["missing cleanup", { cleanup: "" }],
  ["wrong cleanup receiver", { cleanup: "return () => other.disconnect();" }],
  [
    "cleanup notifies parent",
    { cleanup: "return () => { receipt.disconnect(); notify(target); };" },
  ],
  [
    "fake React effect owner",
    {
      imports: "import { useRef, useState } from 'react';",
      before: "const useEffect = callback => callback();",
    },
  ],
  [
    "global constructor overwritten",
    { before: "IntersectionObserver = EagerObserver;" },
  ],
  [
    "globalThis constructor overwritten",
    { before: "globalThis.IntersectionObserver = EagerObserver;" },
  ],
  [
    "window constructor overwritten",
    { before: "window.IntersectionObserver = EagerObserver;" },
  ],
  [
    "global native method overwritten",
    { before: "IntersectionObserver.prototype.observe = notifyImmediately;" },
  ],
  [
    "global constructor defined through a descriptor",
    {
      before:
        "Object.defineProperty(globalThis, 'IntersectionObserver', { value: EagerObserver });",
    },
  ],
  [
    "global constructor replaced through assignment helper",
    {
      before: "Object.assign(window, { IntersectionObserver: EagerObserver });",
    },
  ],
]) {
  semanticTest(`keeps native observation unproved: ${name}`, (t) => {
    const f = fixture(t, observationSource(options));
    addObservation(f);
    assertUnproved(f);
  });
}

semanticTest(
  "native registration does not erase an adjacent genuine immediate push",
  (t) => {
    const f = fixture(t, observationSource({ setup: "notify(target);" }));
    addObservation(f);
    const immediate = addObservation(f, "notify(target)");
    const result = f.evaluate();
    assert.equal(result.status, "requires-review");
    assert.equal(result.upstreamCorrections?.length ?? 0, 0);
    assert.equal(result.findings.length, 2);
    assert.deepEqual(f.doctorReport.diagnostics[1], immediate);
  },
);

semanticTest(
  "native registration requires the exact pinned diagnostic and call start",
  (t) => {
    const f = fixture(t, observationSource());
    const diagnostic = addObservation(f);
    assert.equal(f.evaluate("react-doctor@future").status, "requires-review");
    const original = structuredClone(diagnostic);
    for (const mutation of [
      { plugin: "foreign" },
      { rule: "no-derived-state-effect" },
      { message: `${OBSERVER_MESSAGE} changed` },
      { filePath: "../../outside.tsx" },
      { column: original.column + 1 },
      { line: original.line + 1 },
      { line: 99999 },
    ]) {
      Object.assign(diagnostic, original, mutation);
      assertUnproved(f);
    }
    Object.assign(diagnostic, original);
    f.write(f.file, "export const broken = (");
    assertUnproved(f);
  },
);

for (const [name, imports, initialize, ref] of [
  [
    "native abort owner",
    "import { useEffect, useRef } from 'react';",
    "useRef(null)",
    "owned",
  ],
  [
    "renamed ref binding",
    "import { useEffect, useRef as makeCell } from 'react';",
    "makeCell(null)",
    "receipt",
  ],
  [
    "namespace React factory",
    "import * as R from 'react'; const useEffect = R.useEffect;",
    "R.useRef(null)",
    "ticket",
  ],
]) {
  semanticTest(`proves deferred singleton acquisition: ${name}`, (t) => {
    const guard = `if (${ref}.current) return;`;
    const assignment = `${ref}.current = new AbortController()`;
    const source = `${imports}
function Owner() {
  const ${ref} = ${initialize};
  useEffect(() => { return () => { ${ref}.current?.abort(); ${ref}.current = null; }; }, []);
  const start = () => { ${guard} ${assignment}; };
  return <button onClick={start}>Start</button>;
}`;
    const f = fixture(t, source);
    const diagnostic = f.add(ref);
    assertProof(f, diagnostic, [
      initialize,
      guard,
      assignment,
      `${ref}.current?.abort()`,
    ]);
  });
}

semanticTest(
  "follows an immutable alias of the same application-owned ref",
  (t) => {
    const f = fixture(
      t,
      `import { useEffect, useRef } from 'react';
function Owner() {
  const reference = useRef(null); const owned = reference;
  useEffect(() => { return () => { owned.current?.abort(); owned.current = null; }; }, []);
  const start = () => { if (reference.current) return; reference.current = new AbortController(); };
  return <button onClick={start}>Start</button>;
}`,
    );
    const diagnostic = f.add();
    assertProof(f, diagnostic, [
      "useRef(null)",
      "const owned = reference;",
      "if (reference.current) return;",
      "reference.current = new AbortController()",
      "owned.current?.abort()",
    ]);
  },
);

semanticTest(
  "proves lazy registry initialization after the cleanup effect setup",
  (t) => {
    const f = fixture(
      t,
      `import { useEffect, useRef } from 'react';
function Owner() {
  const owned = useRef(null);
  useEffect(() => { return () => { for (const handle of [...(owned.current ?? [])]) handle.destroy(); }; }, []);
  const open = () => { const registry = (owned.current ??= new Set()); registry.add({ destroy() {} }); };
  return <button onClick={open}>Open</button>;
}`,
    );
    const diagnostic = f.add();
    assertProof(f, diagnostic, [
      "useRef(null)",
      "owned.current ??= new Set()",
      "registry.add({ destroy() {} })",
      "for (const handle of [...(owned.current ?? [])]) handle.destroy();",
    ]);
  },
);

semanticTest(
  "follows the proven local disposal factory used by a later callback",
  (t) => {
    const factory = `function createResource() {
  const resource = { disposed: false, controller: null as AbortController | null,
    dispose() { resource.disposed = true; resource.controller?.abort(); } };
  return resource;
}`;
    const f = fixture(
      t,
      `import { useEffect, useRef } from 'react';
${factory}
function Owner({ identity }) {
  const owned = useRef(null);
  useEffect(() => { return () => { owned.current?.dispose(); owned.current = null; }; }, [identity]);
  const start = () => { if (owned.current) return; const resource = createResource(); owned.current = resource; };
  return <button onClick={start}>Start</button>;
}`,
    );
    const diagnostic = f.add();
    assertProof(f, diagnostic, [
      factory,
      "if (owned.current) return;",
      "owned.current = resource",
      "owned.current?.dispose()",
    ]);
  },
);

semanticTest(
  "records inspected relative factory provenance without trusting its name",
  (t) => {
    const factory = `export function createReceipt() {
  const receipt = { disposed: false, controller: null as AbortController | null,
    dispose() { receipt.disposed = true; receipt.controller?.abort(); } };
  return receipt;
}`;
    const f = fixture(
      t,
      `import { useEffect, useRef } from 'react';
import { createReceipt as make } from './resource-factory';
function Owner() {
  const owned = useRef(null);
  useEffect(() => { return () => { owned.current?.dispose(); owned.current = null; }; }, []);
  const start = () => { if (owned.current) return; owned.current = make(); };
  return <button onClick={start}>Start</button>;
}`,
    );
    f.write("web/app/src/resource-factory.ts", factory);
    const diagnostic = f.add();
    assertProof(f, diagnostic, [
      factory,
      "if (owned.current) return;",
      "owned.current = make()",
    ]);
  },
);

semanticTest(
  "follows a locally inspected controller that populates the late registry",
  (t) => {
    const helper = `export function createController() {
  return { wrap(registry) { return { open() {
    const handle = { destroy() {} }; registry.add(handle); return handle;
  } }; } };
}`;
    const f = fixture(
      t,
      `import { useEffect, useRef } from 'react';
import { createController } from './controller';
const controller = createController();
function Owner() {
  const owned = useRef(null);
  useEffect(() => { return () => { for (const handle of [...(owned.current ?? [])]) handle.destroy(); }; }, []);
  const open = () => { const api = controller.wrap(owned.current ??= new Set()); api.open(); };
  return <button onClick={open}>Open</button>;
}`,
    );
    f.write("web/app/src/controller.ts", helper);
    const diagnostic = f.add();
    assertProof(f, diagnostic, [
      helper,
      "owned.current ??= new Set()",
      "registry.add(handle)",
      "for (const handle of [...(owned.current ?? [])]) handle.destroy();",
    ]);
  },
);

for (const [name, declarations, cleanup, later, jsx] of [
  [
    "merely initialized null",
    "const owned = useRef(null);",
    "owned.current?.dispose();",
  ],
  [
    "only clearing a ref",
    "const owned = useRef(null);",
    "owned.current?.dispose();",
    "const stop = () => { owned.current = null; };",
  ],
  ["unknown ref parameter", "", "owned.current?.dispose();"],
  [
    "fake current-shaped object",
    "const owned = { current: null };",
    "owned.current?.dispose();",
    "const start = () => { if (owned.current) return; owned.current = new AbortController(); };",
  ],
  [
    "shadowed React useRef factory",
    "const useRef = value => ({ current: value }); const owned = useRef(null);",
    "owned.current?.abort();",
    "const start = () => { if (owned.current) return; owned.current = new AbortController(); };",
  ],
  [
    "React-owned DOM ref",
    "const owned = useRef(null);",
    "owned.current?.removeEventListener('click', click);",
    "",
    "<button ref={owned} />",
  ],
  [
    "DOM ownership through stable alias",
    "const actual = useRef(null); const owned = actual;",
    "owned.current?.abort();",
    "const start = () => { if (actual.current) return; actual.current = new AbortController(); };",
    "<button ref={actual} />",
  ],
  [
    "callback-ref DOM writer",
    "const owned = useRef(null);",
    "owned.current?.dispose();",
    "",
    "<button ref={node => { owned.current = node; }} />",
  ],
  [
    "unknown ref escape",
    "const owned = useRef(null); register(owned);",
    "owned.current?.abort();",
    "const start = () => { if (owned.current) return; owned.current = new AbortController(); };",
  ],
  [
    "returned ref escape",
    "const owned = useRef(null); const escaped = { ref: owned }; expose(escaped);",
    "owned.current?.abort();",
    "const start = () => { if (owned.current) return; owned.current = new AbortController(); };",
  ],
  [
    "unguarded owner replacement",
    "const owned = useRef(null);",
    "owned.current?.abort();",
    "const start = () => { owned.current = new AbortController(); };",
  ],
  [
    "guard for a different ref",
    "const owned = useRef(null); const other = useRef(null);",
    "owned.current?.abort();",
    "const start = () => { if (other.current) return; owned.current = new AbortController(); };",
  ],
  [
    "guard after replacement",
    "const owned = useRef(null);",
    "owned.current?.abort();",
    "const start = () => { owned.current = new AbortController(); if (owned.current) return; };",
  ],
  [
    "opaque resource constructor",
    "const owned = useRef(null);",
    "owned.current?.dispose();",
    "const start = () => { if (owned.current) return; owned.current = createUnknown(); };",
  ],
  [
    "shadowed native abort constructor",
    "const owned = useRef(null); const AbortController = externalFactory;",
    "owned.current?.abort();",
    "const start = () => { if (owned.current) return; owned.current = new AbortController(); };",
  ],
  [
    "DOM node assigned by the application",
    "const owned = useRef(null);",
    "owned.current?.removeEventListener('click', click);",
    "const start = () => { if (owned.current) return; owned.current = document.createElement('button'); };",
  ],
  [
    "wrong cleanup receiver",
    "const owned = useRef(null); const other = useRef(null);",
    "other.current?.abort();",
    "const start = () => { if (owned.current) return; owned.current = new AbortController(); };",
  ],
  [
    "shadowed ref inside cleanup",
    "const owned = useRef(null);",
    "const owned = foreign; owned.current?.abort();",
    "const start = () => { if (owned.current) return; owned.current = new AbortController(); };",
  ],
  [
    "wrong disposal contract",
    "const owned = useRef(null);",
    "owned.current?.destroy();",
    "const start = () => { if (owned.current) return; owned.current = new AbortController(); };",
  ],
  [
    "unknown registry entries",
    "const owned = useRef(null);",
    "for (const handle of [...(owned.current ?? [])]) handle.destroy();",
    "const start = () => { const registry = (owned.current ??= new Set()); registry.add(externalHandle); };",
  ],
  [
    "registry ref passed to JSX",
    "const owned = useRef(null);",
    "for (const handle of [...(owned.current ?? [])]) handle.destroy();",
    "const start = () => { (owned.current ??= new Set()).add({ destroy() {} }); };",
    "<Widget ref={owned} />",
  ],
]) {
  semanticTest(
    `keeps unsafe or unproved ownership unresolved: ${name}`,
    (t) => {
      const f = fixture(
        t,
        `import { useEffect, useRef } from 'react';
function Owner(${name === "unknown ref parameter" ? "{ owned }" : ""}) {
  ${declarations}
  useEffect(() => { return () => { ${cleanup} }; }, []);
  ${later || ""}
  return ${jsx || "<span />"};
}`,
      );
      f.add();
      assertUnproved(f);
    },
  );
}

semanticTest(
  "clearing a local binding with a disposal-like name is not a disposal contract",
  (t) => {
    const f = fixture(
      t,
      `import { useEffect, useRef } from 'react';
function createResource() { return { dispose() { analytics(); } }; }
function Owner() {
  const owned = useRef(null);
  useEffect(() => { return () => { owned.current?.dispose(); owned.current = null; }; }, []);
  const start = () => { if (owned.current) return; owned.current = createResource(); };
  return <button onClick={start}>Start</button>;
}`,
    );
    f.add();
    assertUnproved(f);
  },
);

semanticTest(
  "fails closed when an imported helper source cannot be inspected",
  (t) => {
    const f = fixture(
      t,
      `import { useEffect, useRef } from 'react'; import { make } from './missing';
function Owner() {
  const owned = useRef(null);
  useEffect(() => { return () => { owned.current?.dispose(); owned.current = null; }; }, []);
  const start = () => { if (owned.current) return; owned.current = make(); };
  return <button onClick={start}>Start</button>;
}`,
    );
    f.add();
    assertUnproved(f);
  },
);

semanticTest(
  "fails closed on unknown provenance, diagnostic or exact source position",
  (t) => {
    const source = `import { useEffect, useRef } from 'react'; function Owner() {
  const owned = useRef(null);
  useEffect(() => { return () => { owned.current?.abort(); owned.current = null; }; }, []);
  const start = () => { if (owned.current) return; owned.current = new AbortController(); };
  return <button onClick={start}>Start</button>;
}`;
    const f = fixture(t, source);
    const diagnostic = f.add();
    assert.equal(f.evaluate("react-doctor@future").status, "requires-review");
    const original = structuredClone(diagnostic);
    for (const mutation of [
      { plugin: "unknown" },
      { rule: "unknown" },
      { message: "unknown revision" },
      { message: messageFor("other") },
      { filePath: "../../outside.tsx" },
      { line: 9999 },
      { column: 9999 },
    ]) {
      Object.assign(diagnostic, original, mutation);
      assertUnproved(f);
    }
    Object.assign(diagnostic, original);
    f.write(f.file, "export const syntax = (");
    assertUnproved(f);
  },
);

semanticTest(
  "actual React cleanup releases a handle created after setup; capturing setup null leaks it",
  async () => {
    const requireWeb = createRequire(
      path.resolve(__dirname, "../../../../web/package.json"),
    );
    const requireApp = createRequire(
      path.resolve(__dirname, "../../../../web/app/package.json"),
    );
    const { JSDOM } = requireWeb("jsdom");
    const dom = new JSDOM("<!doctype html><html><body></body></html>", {
      url: "http://localhost/",
    });
    const keys = [
      "window",
      "document",
      "navigator",
      "IS_REACT_ACT_ENVIRONMENT",
    ];
    const originals = new Map(
      keys.map((key) => [
        key,
        Object.getOwnPropertyDescriptor(globalThis, key),
      ]),
    );
    for (const [key, value] of Object.entries({
      window: dom.window,
      document: dom.window.document,
      navigator: dom.window.navigator,
      IS_REACT_ACT_ENVIRONMENT: true,
    })) {
      Object.defineProperty(globalThis, key, {
        value,
        writable: true,
        configurable: true,
      });
    }
    let root;
    try {
      const React = requireApp("react");
      const { createRoot } = requireApp("react-dom/client");
      for (const captureAtSetup of [false, true]) {
        const container = document.createElement("div");
        document.body.append(container);
        let open;
        let destroys = 0;
        function Owner() {
          const owned = React.useRef(null);
          React.useEffect(() => {
            const setupSnapshot = owned.current;
            return () => {
              for (const handle of [
                ...((captureAtSetup ? setupSnapshot : owned.current) ?? []),
              ])
                handle.destroy();
            };
          }, []);
          open = () => {
            (owned.current ??= new Set()).add({
              destroy() {
                destroys++;
              },
            });
          };
          return React.createElement("button", { onClick: open }, "Open");
        }
        root = createRoot(container);
        await React.act(async () => root.render(React.createElement(Owner)));
        assert.equal(destroys, 0);
        await React.act(async () => open());
        assert.equal(destroys, 0);
        await React.act(async () => root.unmount());
        root = null;
        assert.equal(
          destroys,
          captureAtSetup ? 0 : 1,
          captureAtSetup
            ? "capturing null at setup leaks the later handle"
            : "cleanup must read the live resource registry",
        );
        container.remove();
      }
    } finally {
      if (root) {
        const React = requireApp("react");
        await React.act(async () => root.unmount());
      }
      dom.window.close();
      for (const [key, descriptor] of originals) {
        if (descriptor) Object.defineProperty(globalThis, key, descriptor);
        else delete globalThis[key];
      }
    }
  },
);
