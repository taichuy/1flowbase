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
const COMMIT = "2d1ab245f0cbaa87529067e6e3ed9ac08362d1fc";
const DERIVED = "no-derived-state";
const EVENT = "no-event-handler";
const HASHES = {
  [DERIVED]: "db98ba7989c837bb635b3fed080a937ed425ecce6330126dd58b15f66e9eb91f",
  [EVENT]: "1f89d763b79829a89a3c00e9d655ff8299e26b98c5f7d46a770f3bb0ccf74062",
};
const EVENT_PROP =
  "Faking an event handler with a prop plus a useEffect costs an extra render & runs late.";
const EVENT_STATE =
  "Faking an event handler with state plus a useEffect costs an extra render & runs late.";
const semanticTest = (name, fn) =>
  test(`history observation semantic correction: ${name}`, fn);

function fixture(t, source, file = "web/app/src/arbitrary-observation-owner.tsx") {
  const repoRoot = fs.mkdtempSync(
    path.join(os.tmpdir(), "doctor-history-observation-"),
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
  const add = (
    rule,
    anchor,
    message = rule === EVENT
      ? EVENT_PROP
      : 'Storing "snapshot" in state when you can derive it from other values costs an extra render.',
  ) => {
    const offset = sources.get(file).indexOf(anchor);
    assert.ok(offset >= 0, `missing diagnostic anchor ${anchor}`);
    const prefix = sources.get(file).slice(0, offset);
    const diagnostic = {
      filePath: file.slice("web/app/".length),
      plugin: "react-doctor",
      rule,
      severity: "warning",
      message,
      line: prefix.split("\n").length,
      column: prefix.length - prefix.lastIndexOf("\n"),
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
      "retain the entire raw inventory, message, severity and positions",
    );
    assert.deepEqual(
      JSON.parse(fs.readFileSync(path.join(repoRoot, REVIEW_FILE))),
      manifest,
      "do not store file, symbol, method or diagnostic exceptions",
    );
    return result;
  };
  return { repoRoot, file, sources, write, add, evaluate, doctorReport };
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
  assert.equal(proof.provenance.sourceCommit, COMMIT);
  assert.equal(proof.provenance.ruleSourceSha256, HASHES[diagnostic.rule]);
  assert.match(proof.provenance.packageIntegrity, /^sha512-/u);
  assert.ok(proof.provenance.references.length > 0);
  assert.ok(proof.evidence.length > 0);
  for (const evidence of proof.evidence) {
    const source = f.sources.get(evidence.file);
    assert.equal(typeof source, "string");
    assert.equal(evidence.sha256, sourceDigest(source));
    assert.ok(evidence.spans.length > 0);
    for (const span of evidence.spans)
      assert.ok(
        Number.isInteger(span.start) &&
          Number.isInteger(span.end) &&
          span.start >= 0 &&
          span.start < span.end &&
          span.end <= source.length,
      );
  }
  for (const span of spans)
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

function snapshotSource({
  body = "if (completed) setSnapshot({ scope, diagnostics: completed.diagnostics });",
  fallback = "completed ?? (snapshot?.scope === scope ? snapshot : null)",
  after = "",
  imports = "import { useEffect, useState } from 'react';",
} = {}) {
  return `${imports}
function Owner({ scope, completed }) {
  const [snapshot, setSnapshot] = useState(null);
  useEffect(() => { ${body} }, [scope, completed]);
  ${after}
  const visible = ${fallback};
  return <output>{visible?.diagnostics}</output>;
}`;
}

for (const [name, body, fallback, spans] of [
  [
    "retained completed diagnostics",
    "if (completed) setSnapshot({ scope, diagnostics: completed.diagnostics });",
    "completed ?? (snapshot?.scope === scope ? snapshot : null)",
    [
      "if (completed) setSnapshot({ scope, diagnostics: completed.diagnostics });",
      "snapshot?.scope === scope ? snapshot : null",
    ],
  ],
  [
    "renamed owner-tag property",
    "if (completed) setSnapshot({ owner: scope, diagnostics: completed.diagnostics });",
    "completed ?? (snapshot?.owner === scope ? snapshot : null)",
    [
      "setSnapshot({ owner: scope, diagnostics: completed.diagnostics })",
      "snapshot?.owner === scope ? snapshot : null",
    ],
  ],
]) {
  semanticTest(
    `proves history is retained while the current result is absent: ${name}`,
    (t) => {
      const f = fixture(t, snapshotSource({ body, fallback }));
      const diagnostic = f.add(DERIVED, "setSnapshot({");
      assertProof(f, diagnostic, "retained-owner-snapshot", spans);
    },
  );
}

semanticTest(
  "associates a prop diagnostic with the proven retained snapshot effect",
  (t) => {
    const f = fixture(t, snapshotSource());
    const diagnostic = f.add(EVENT, "completed) setSnapshot");
    assertProof(f, diagnostic, "retained-owner-snapshot", [
      "if (completed) setSnapshot({ scope, diagnostics: completed.diagnostics });",
      "snapshot?.scope === scope ? snapshot : null",
    ]);
  },
);

for (const [name, options] of [
  [
    "unconditional current-value copy",
    { body: "setSnapshot({ scope, diagnostics: completed?.diagnostics });" },
  ],
  [
    "clearing on a pending result",
    {
      body: "if (completed) setSnapshot({ scope, diagnostics: completed.diagnostics }); else setSnapshot(null);",
    },
  ],
  ["no consumer fallback", { fallback: "completed" }],
  ["unscoped consumer fallback", { fallback: "completed ?? snapshot" }],
  [
    "different owner in the consumer",
    {
      fallback:
        "completed ?? (snapshot?.scope === otherScope ? snapshot : null)",
    },
  ],
  [
    "owner stamp and consumer key differ",
    {
      body: "if (completed) setSnapshot({ owner: scope, diagnostics: completed.diagnostics });",
    },
  ],
  [
    "owner fallback returns the current value",
    { fallback: "completed ?? (snapshot?.scope === scope ? completed : null)" },
  ],
  [
    "a guard with an unrelated completion source",
    {
      body: "if (ready) setSnapshot({ scope, diagnostics: completed.diagnostics });",
    },
  ],
  ["snapshot reset on every render", { after: "setSnapshot(null);" }],
  [
    "another effect unconditionally replaces the history",
    {
      after:
        "useEffect(() => { setSnapshot({ scope, diagnostics: completed?.diagnostics }); }, [scope, completed]);",
    },
  ],
  [
    "mixed event dispatch in the snapshot effect",
    {
      body: "if (completed) { setSnapshot({ scope, diagnostics: completed.diagnostics }); submit(completed); }",
    },
  ],
  [
    "opaque setter function",
    {
      imports:
        "import { useEffect } from 'react'; const useState = () => [null, externalSetter];",
    },
  ],
]) {
  semanticTest(
    `keeps unproved or genuine snapshot copying unresolved: ${name}`,
    (t) => {
      const f = fixture(t, snapshotSource(options));
      f.add(DERIVED, "setSnapshot({");
      assertUnproved(f);
    },
  );
}

function latchSource({
  latch = "hydrated",
  initialize = "useRef(false)",
  guard,
  update,
  body,
  after = "",
  prefix = "",
  seed = "{ ...incoming.data }",
} = {}) {
  return `import { useEffect, useRef, useState } from 'react';
function Owner({ incoming }) {
  const [draft, setDraft] = useState({});
  const ${latch} = ${initialize};
  ${prefix}
  useEffect(() => { ${body ?? `if (${guard ?? `!${latch}.current && incoming.data`}) { ${update ?? `${latch}.current = true;`} setDraft(${seed}); }`} }, [incoming.data]);
  ${after}
  return <input value={draft.name || ''} onChange={event => setDraft({ name: event.target.value })} />;
}`;
}
for (const latch of ["hydrated", "firstSuccess"]) {
  semanticTest(
    `proves the lexical first-success latch independent of a Ref suffix: ${latch}`,
    (t) => {
      const f = fixture(t, latchSource({ latch }));
      const diagnostic = f.add(
        DERIVED,
        "setDraft({ ...incoming.data })",
        'Storing "draft" in state when you can derive it from other values costs an extra render.',
      );
      assertProof(f, diagnostic, "first-success-seed", [
        "useRef(false)",
        `${latch}.current = true`,
        "setDraft({ ...incoming.data })",
      ]);
    },
  );
}
semanticTest(
  "proves first-success effect synchronization is not a delayed user event",
  (t) => {
    const f = fixture(t, latchSource());
    const diagnostic = f.add(EVENT, "incoming.data) {");
    assertProof(f, diagnostic, "first-success-seed", [
      "if (!hydrated.current && incoming.data) { hydrated.current = true; setDraft({ ...incoming.data }); }",
    ]);
  },
);

for (const [name, options] of [
  ["latch is never set", { update: "" }],
  ["latch is set back to false", { update: "hydrated.current = false;" }],
  ["latch resets during every render", { prefix: "hydrated.current = false;" }],
  [
    "another effect resets the latch",
    { after: "useEffect(() => { hydrated.current = false; }, [incoming]);" },
  ],
  [
    "latch uses a fake current-shaped object",
    { initialize: "({ current: false })" },
  ],
  ["guard does not include the latch", { guard: "incoming.data" }],
  [
    "guard only checks that the latch object exists",
    { guard: "hydrated && incoming.data" },
  ],
  [
    "guard checks a different latch",
    {
      prefix: "const other = useRef(false);",
      guard: "!other.current && incoming.data",
    },
  ],
  ["guarded seed is a different source", { seed: "otherRecord.data" }],
  [
    "another effect continuously mirrors the seed",
    {
      after:
        "useEffect(() => { setDraft({ ...incoming.data }); }, [incoming.data]);",
    },
  ],
  [
    "a setter runs before the latch condition",
    {
      body: "setDraft({ ...incoming.data }); if (!hydrated.current && incoming.data) hydrated.current = true;",
    },
  ],
  [
    "history read is discarded",
    { seed: "readAndDiscard(draft, incoming.data)" },
  ],
  [
    "arbitrary spread followed by prop overwrite",
    { seed: "{ ...draft, name: incoming.data.name }" },
  ],
  [
    "defaults and relation overlay remain outside this proof",
    {
      seed: "previous => ({ ...defaults, ...previous, ...(relation ? { [relation.key]: parentId } : {}) })",
    },
  ],
  [
    "mixed analytics dispatch",
    {
      body: "if (!hydrated.current && incoming.data) { hydrated.current = true; setDraft({ ...incoming.data }); track(incoming.data); }",
    },
  ],
]) {
  semanticTest(
    `keeps unsafe or unproved first-load state unresolved: ${name}`,
    (t) => {
      const f = fixture(t, latchSource(options));
      f.add(
        DERIVED,
        "setDraft(",
        'Storing "draft" in state when you can derive it from other values costs an extra render.',
      );
      assertUnproved(f);
    },
  );
}

function querySource({
  query = "useInfiniteQuery({ queryKey: ['children'], queryFn: load })",
  imports = "import { useInfiniteQuery } from '@tanstack/react-query';",
  declarations = "",
  refetch = "const { refetch: refresh } = query;",
  signature = "`${status}:${finished}`",
  initialize = "useRef(signature)",
  bookkeeping = "previous.current = signature;",
  action = "void refresh();",
  extra = "",
  effectTail = "",
} = {}) {
  return `import { useEffect, useRef, useState } from 'react';
${imports}
function Owner({ status, finished, initiallyExpanded, hasChildren, load }) {
  const [expanded, setExpanded] = useState(initiallyExpanded);
  ${declarations}
  const query = ${query};
  ${refetch}
  const signature = ${signature};
  const previous = ${initialize};
  useEffect(() => {
    const changed = previous.current !== signature;
    ${bookkeeping}
    if (changed && expanded && hasChildren) ${action}
    ${effectTail}
  }, [signature, expanded, hasChildren, refresh]);
  ${extra}
  return <button onClick={() => setExpanded(value => !value)}>Expand</button>;
}`;
}
for (const [name, options] of [
  ["renamed useInfiniteQuery refetch", {}],
  [
    "useQuery refetch result",
    {
      imports: "import { useQuery } from '@tanstack/react-query';",
      query: "useQuery({ queryKey: ['children'], queryFn: load })",
    },
  ],
  [
    "immutable refetch member alias",
    { refetch: "const refresh = query.refetch;" },
  ],
  [
    "namespace query factory",
    {
      imports: "import * as Q from '@tanstack/react-query';",
      query: "Q.useInfiniteQuery({ queryKey: ['children'], queryFn: load })",
    },
  ],
]) {
  semanticTest(
    `proves external cache observation without event dispatch: ${name}`,
    (t) => {
      const f = fixture(t, querySource(options));
      const diagnostic = f.add(EVENT, "status}:${finished}");
      assertProof(f, diagnostic, "query-observation-synchronization", [
        "previous.current !== signature",
        "previous.current = signature",
        "void refresh()",
        options.refetch ?? "const { refetch: refresh } = query;",
      ]);
    },
  );
}
semanticTest(
  "associates the state diagnostic at its initial-prop source with query observation",
  (t) => {
    const f = fixture(t, querySource());
    const diagnostic = f.add(EVENT, "initiallyExpanded);", EVENT_STATE);
    assertProof(f, diagnostic, "query-observation-synchronization", [
      "useState(initiallyExpanded)",
      "void refresh()",
    ]);
  },
);
semanticTest(
  "associates a direct policy-guard prop with query observation",
  (t) => {
    const f = fixture(t, querySource());
    const diagnostic = f.add(EVENT, "hasChildren) void");
    assertProof(f, diagnostic, "query-observation-synchronization", [
      "if (changed && expanded && hasChildren) void refresh();",
    ]);
  },
);
semanticTest(
  "a separate effect for another input does not create false ambiguity",
  (t) => {
    const f = fixture(
      t,
      querySource({
        extra: "useEffect(() => { if (load) submit(load); }, [load]);",
      }),
    );
    const diagnostic = f.add(EVENT, "status}:${finished}");
    assertProof(f, diagnostic, "query-observation-synchronization", [
      "previous.current = signature",
      "void refresh()",
    ]);
  },
);
semanticTest(
  "two candidate query effects are accepted only with evidence for both",
  (t) => {
    const second = `const secondPrevious = useRef(signature);
  useEffect(() => { const secondChanged = secondPrevious.current !== signature;
    secondPrevious.current = signature;
    if (secondChanged && expanded && hasChildren) void refresh();
  }, [signature, expanded, hasChildren, refresh]);`;
    const f = fixture(t, querySource({ extra: second }));
    const diagnostic = f.add(EVENT, "status}:${finished}");
    assertProof(f, diagnostic, "query-observation-synchronization", [
      "previous.current = signature",
      "secondPrevious.current = signature",
      "secondPrevious.current !== signature",
    ]);
  },
);

for (const [name, options] of [
  ["unknown query producer", { imports: "", query: "externalQuery()" }],
  [
    "local query-shaped object",
    { imports: "", query: "({ refetch: () => submit() })" },
  ],
  [
    "shadowed imported query hook",
    { declarations: "const useInfiniteQuery = externalQuery;" },
  ],
  [
    "refetch-looking local action",
    { refetch: "const refresh = () => submit();" },
  ],
  ["bookkeeping never advances", { bookkeeping: "" }],
  [
    "bookkeeping updates another ref",
    {
      declarations: "const other = useRef(null);",
      bookkeeping: "other.current = signature;",
    },
  ],
  [
    "bookkeeping does not record the observed signature",
    { bookkeeping: "previous.current = finished;" },
  ],
  ["unrelated initial stamp", { initialize: "useRef(null)" }],
  [
    "signature rebuilt by an opaque producer",
    { signature: "makeSignature(status, finished)" },
  ],
  ["mixed user event dispatch", { action: "{ void refresh(); submit(); }" }],
  ["mixed state mutation", { effectTail: "setExpanded(false);" }],
  [
    "refetch guard ignores the signature transition",
    {
      action: "void refresh();",
      bookkeeping: "previous.current = signature; if (expanded) submit();",
    },
  ],
  [
    "the same input also reaches a genuine event effect",
    { extra: "useEffect(() => { if (status) submit(status); }, [status]);" },
  ],
  [
    "the same input reaches an opaque effect",
    { extra: "useEffect(() => { if (status) unknown(status); }, [status]);" },
  ],
]) {
  semanticTest(
    `keeps unsafe or unproved cache synchronization unresolved: ${name}`,
    (t) => {
      const f = fixture(t, querySource(options));
      f.add(
        EVENT,
        name === "signature rebuilt by an opaque producer"
          ? "status, finished)"
          : "status}:${finished}",
      );
      assertUnproved(f);
    },
  );
}

function completionSource({
  selected = "focus",
  collection = "marked",
  panel = "popover",
  action = "void client.invalidateQueries({ queryKey: observationKey(scope, run.state.id, selected) });",
  extra = "",
  resetEffect = true,
  client = "useQueryClient()",
  declarations = "",
  dependencies = "run.active, run.state.id, selected, scope, client",
  resetProvider = "useView",
} = {}) {
  return `import { useEffect } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { useView } from './view-bridge';
import { observeExecution } from 'external-execution-source';
import { observationKey } from './observation-key';
function RenamedConsumer({ scope, onComplete }) {
  const client = ${client};
  const run = observeExecution(scope);
  const selected = useView(state => state.${selected});
  const reset = ${resetProvider}(state => state.clearView);
  ${declarations}
  useEffect(() => {
    if (!run.active && run.state.id && selected) { ${action} }
  }, [${dependencies}]);
  ${resetEffect ? "useEffect(() => { if (!run.active) return; reset(); }, [run.active, reset]);" : ""}
  ${extra}
  return <output>{selected}</output>;
}`;
}

function completionFixture(t, options = {}) {
  const selected = options.selected ?? "focus";
  const collection = options.collection ?? "marked";
  const panel = options.panel ?? "popover";
  const f = fixture(t, completionSource(options));
  f.write("web/app/src/view-bridge.ts", `export { useOwnedView as useView } from './view-provider';`);
  f.write("web/app/src/view-provider.ts", `import { createContext, useContext } from 'react';
import { useStore } from 'zustand';
import { createOwnedStore } from './view-store';
type OwnedStore = ReturnType<typeof createOwnedStore>;
export const OwnedContext = createContext<OwnedStore | null>(null);
export function useOwnedView(selector) {
  const store = useContext(OwnedContext);
  if (!store) throw new Error('missing view owner');
  return useStore(store, selector);
}`);
  f.write("web/app/src/view-store.ts", `import { createStore } from 'zustand/vanilla';
function changed(document, baseline) { return JSON.stringify(document) !== JSON.stringify(baseline); }
export function createOwnedStore(receipt) {
  return createStore(set => ({
    document: receipt.document,
    baseline: receipt.document,
    ${selected}: null,
    ${collection}: [],
    ${panel}: { open: false, anchor: null },
    dirty: false,
    choose: value => set({ ${selected}: value, ${collection}: [value] }),
    replace: document => set(current => ({ document, dirty: changed(document, current.baseline) })),
    clearView: () => set(current => ({
      ${selected}: null,
      ${collection}: [],
      ${panel}: { ...current.${panel}, open: false, anchor: null }
    }))
  }));
}`);
  f.write("web/app/src/observation-key.ts", `export const observationKey = (scope, execution, item) => ['observation', scope, execution, item] as const;`);
  return f;
}

semanticTest("proves conditional native query invalidation without a hook-cleanup premise", (t) => {
  const f = completionFixture(t, { resetEffect: false });
  const diagnostic = f.add(EVENT, "observeExecution(scope)");
  const proof = assertProof(f, diagnostic, "query-observation-synchronization", [
    "useQueryClient()",
    "if (!run.active && run.state.id && selected) { void client.invalidateQueries({ queryKey: observationKey(scope, run.state.id, selected) }); }",
    "export const observationKey = (scope, execution, item) => ['observation', scope, execution, item] as const;",
  ]);
  assert.ok(proof.evidence.some((entry) => entry.file === "web/app/src/observation-key.ts"));
});

for (const names of [
  { selected: "focus", collection: "marked", panel: "popover" },
  { selected: "cursor", collection: "highlighted", panel: "flyout" },
]) {
  semanticTest(`proves all completion effects with renamed owned transient fields ${names.selected}`, (t) => {
    const f = completionFixture(t, names);
    const diagnostic = f.add(EVENT, "observeExecution(scope)");
    const proof = assertProof(f, diagnostic, "query-observation-synchronization", [
      "if (!run.active) return; reset();",
      `export { useOwnedView as useView } from './view-provider';`,
      "return useStore(store, selector);",
      `${names.selected}: null`,
    ]);
    for (const suffix of ["view-bridge.ts", "view-provider.ts", "view-store.ts", "observation-key.ts"]) {
      assert.ok(proof.evidence.some((entry) => entry.file.endsWith(suffix)), `missing resolved provenance ${suffix}`);
    }
  });
}

for (const [name, options, mutate] of [
  ["genuine completion callback", { action: "onComplete(run.state.id);" }],
  ["mixed callback beside invalidation", { action: "void client.invalidateQueries({ queryKey: observationKey(scope, run.state.id, selected) }); onComplete(run.state.id);" }],
  ["fake query client", { client: "({ invalidateQueries: onComplete })" }],
  ["patched native client", { declarations: "client.invalidateQueries = onComplete;" }],
  ["missing observation dependency", { dependencies: "run.active, selected, scope, client" }],
  ["extra same-root business effect", { extra: "useEffect(() => { if (run.state.id) onComplete(run.state.id); }, [run.state.id, onComplete]);" }],
  ["business document reset", {}, (f) => f.write("web/app/src/view-store.ts", f.sources.get("web/app/src/view-store.ts").replace("clearView: () => set(current => ({", "clearView: () => set(current => ({ document: null,"))],
  ["business-derived dirty reset", {}, (f) => f.write("web/app/src/view-store.ts", f.sources.get("web/app/src/view-store.ts").replace("clearView: () => set(current => ({", "clearView: () => set(current => ({ dirty: false,"))],
  ["mixed store callback", {}, (f) => f.write("web/app/src/view-store.ts", f.sources.get("web/app/src/view-store.ts").replace("clearView: () => set(current => ({", "clearView: () => { notify(); return set(current => ({").replace("    }))\n  }));", "    })); }\n  }));"))],
  ["impure key helper", {}, (f) => f.write("web/app/src/observation-key.ts", "export const observationKey = (scope, execution, item) => submit(scope, execution, item);")],
  ["opaque provider import", {}, (f) => f.write("web/app/src/view-bridge.ts", "export { useOwnedView as useView } from 'opaque-provider';")],
  ["store without an observable business partition", {}, (f) => f.write("web/app/src/view-store.ts", `import { createStore } from 'zustand/vanilla';
export function createOwnedStore() {
  return createStore(set => ({ focus: null, marked: [], popover: { open: false, anchor: null },
    clearView: () => set({ focus: null, marked: [], popover: { open: false, anchor: null } }) }));
}`)],
]) {
  semanticTest(`holds unsafe completion observation: ${name}`, (t) => {
    const f = completionFixture(t, options);
    mutate?.(f);
    f.add(EVENT, "observeExecution(scope)");
    assertUnproved(f);
  });
}

semanticTest("holds a reset taken from a different store than the observed selection", (t) => {
  const f = completionFixture(t, { resetProvider: "useForeignView", declarations: "" });
  f.write(f.file, f.sources.get(f.file).replace("import { useView } from './view-bridge';", "import { useView, useForeignView } from './view-bridge';"));
  f.write("web/app/src/view-bridge.ts", "export { useOwnedView as useView, useForeignView } from './view-provider';");
  f.write("web/app/src/view-provider.ts", `${f.sources.get("web/app/src/view-provider.ts")}
const ForeignContext = createContext<OwnedStore | null>(null);
export function useForeignView(selector) {
  const store = useContext(ForeignContext);
  if (!store) throw new Error('missing foreign owner');
  return useStore(store, selector);
}`);
  f.add(EVENT, "observeExecution(scope)");
  assertUnproved(f);
});

semanticTest("proves the actual Frame cache observation and all owned run-entry resets", (t) => {
  const repoRoot = path.resolve(__dirname, "../../../..");
  const file = "web/app/src/features/workflow/components/WorkflowCanvasFrame.tsx";
  const f = fixture(t, fs.readFileSync(path.join(repoRoot, file), "utf8"), file);
  const dependencies = [
    "web/app/src/features/agent-flow/api/runtime.ts",
    "web/app/src/features/agent-flow/store/editor/provider.ts",
    "web/app/src/features/flow-editor/store/provider.tsx",
    "web/app/src/features/flow-editor/store/index.ts",
  ];
  for (const dependency of dependencies) f.write(dependency, fs.readFileSync(path.join(repoRoot, dependency), "utf8"));
  const diagnostic = f.add(EVENT, "useWorkflowTestRun(applicationId, workingDocument)");
  const proof = assertProof(f, diagnostic, "query-observation-synchronization", [
    "useQueryClient()",
    "if (!testRun.active) return;",
    "clearRunEntryTransientState();",
  ]);
  for (const dependency of dependencies) assert.ok(proof.evidence.some((entry) => entry.file === dependency), `missing actual resolved source ${dependency}`);
});

semanticTest("resolved store and key source changes invalidate completion proof", (t) => {
  const f = completionFixture(t);
  const diagnostic = f.add(EVENT, "observeExecution(scope)");
  assertProof(f, diagnostic, "query-observation-synchronization", []);
  const original = f.sources.get("web/app/src/observation-key.ts");
  f.write("web/app/src/observation-key.ts", "export const observationKey = (...values) => notify(values);");
  assertUnproved(f);
  f.write("web/app/src/observation-key.ts", original);
  f.write("web/app/src/view-store.ts", f.sources.get("web/app/src/view-store.ts").replace("clearView: () => set(current => ({", "clearView: () => set(current => ({ document: null,"));
  assertUnproved(f);
});

semanticTest(
  "all candidate effects reached by one diagnostic must have a permitted proof",
  (t) => {
    const f = fixture(
      t,
      `import { useEffect, useRef, useState } from 'react';
function Owner({ incoming }) {
  const [draft, setDraft] = useState({}); const hydrated = useRef(false);
  useEffect(() => { if (!hydrated.current && incoming.data) { hydrated.current = true; setDraft({ ...incoming.data }); } }, [incoming.data]);
  useEffect(() => { if (incoming.data) submit(incoming.data); }, [incoming.data]);
  return <input value={draft.name || ''} onChange={event => setDraft({ name: event.target.value })} />;
}`,
    );
    f.add(EVENT, "incoming })");
    assertUnproved(f);
  },
);
semanticTest(
  "an unrelated occurrence of the same binding is not a diagnostic-to-effect association",
  (t) => {
    const f = fixture(
      t,
      snapshotSource({ prefix: "" }).replace(
        "const [snapshot,",
        "void completed; const [snapshot,",
      ),
    );
    f.add(EVENT, "completed; const");
    assertUnproved(f);
  },
);
semanticTest(
  "a stale diagnostic at a reducer initializer cannot authorize a later snapshot effect",
  (t) => {
    const f = fixture(
      t,
      snapshotSource({
        imports: "import { useEffect, useReducer, useState } from 'react';",
      }).replace(
        "const [snapshot,",
        "const [selection] = useReducer(reducer, completed); const [snapshot,",
      ),
    );
    f.add(EVENT, "completed); const");
    assertUnproved(f);
  },
);
semanticTest(
  "first-load proof never accepts the held defaults-plus-relation-overlay shape",
  (t) => {
    const f = fixture(
      t,
      `import { useEffect, useState } from 'react';
function Owner({ incoming, relation, parentId }) {
  const [draft, setDraft] = useState({});
  useEffect(() => { if (incoming) setDraft(previous => ({ ...incoming, ...previous, ...(relation ? { [relation.key]: parentId } : {}) })); }, [incoming, relation, parentId]);
  return <input value={draft.name || ''} />;
}`,
    );
    f.add(
      DERIVED,
      "setDraft(",
      'Storing "draft" in state when you can derive it from other values costs an extra render.',
    );
    assertUnproved(f);
  },
);
semanticTest(
  "fails closed on unknown provenance, source revision and exact position",
  (t) => {
    const f = fixture(t, snapshotSource());
    const diagnostic = f.add(DERIVED, "setSnapshot({");
    assert.equal(f.evaluate("react-doctor@future").status, "requires-review");
    const original = structuredClone(diagnostic);
    for (const mutation of [
      { plugin: "unknown" },
      { rule: "unknown" },
      { message: "unknown revision" },
      { filePath: "../../outside.tsx" },
      { line: 99999 },
      { column: 99999 },
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
  "actual React retains owner-scoped completed history and edits after a first-success seed",
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
    }))
      Object.defineProperty(globalThis, key, {
        value,
        writable: true,
        configurable: true,
      });
    let root;
    try {
      const React = requireApp("react");
      const { createRoot } = requireApp("react-dom/client");
      const container = document.createElement("div");
      document.body.append(container);
      let edit;
      function Owner({ scope, completed, seed }) {
        const [snapshot, setSnapshot] = React.useState(null);
        const [draft, setDraft] = React.useState("");
        const hydrated = React.useRef(false);
        React.useEffect(() => {
          if (completed) setSnapshot({ scope, value: completed });
        }, [scope, completed]);
        React.useEffect(() => {
          if (!hydrated.current && seed) {
            hydrated.current = true;
            setDraft(seed);
          }
        }, [seed]);
        edit = setDraft;
        const value =
          completed ?? (snapshot?.scope === scope ? snapshot.value : "pending");
        return React.createElement("output", null, `${value}|${draft}`);
      }
      root = createRoot(container);
      await React.act(async () =>
        root.render(
          React.createElement(Owner, {
            scope: "a",
            completed: "finished-a",
            seed: "first",
          }),
        ),
      );
      assert.equal(container.textContent, "finished-a|first");
      await React.act(async () => edit("user edit"));
      await React.act(async () =>
        root.render(
          React.createElement(Owner, {
            scope: "a",
            completed: null,
            seed: "refetched",
          }),
        ),
      );
      assert.equal(
        container.textContent,
        "finished-a|user edit",
        "current inputs cannot reconstruct retained completion or user edits",
      );
      await React.act(async () =>
        root.render(
          React.createElement(Owner, {
            scope: "b",
            completed: null,
            seed: "refetched",
          }),
        ),
      );
      assert.equal(
        container.textContent,
        "pending|user edit",
        "a different owner must not receive the previous completion",
      );
      await React.act(async () => root.unmount());
      root = null;
      container.remove();
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
