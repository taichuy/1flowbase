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
const COMMIT = "2d1ab245f0cbaa87529067e6e3ed9ac08362d1fc";
const INTEGRITY =
  "sha512-o108QscqM/TlC1QmKgDORf/P53PeZ69nZbRsvy53rXkKvWdKZC1Y0u1JRPmThbTh6v+QpMispBwEnFHhlpZQ7w==";
const RULE = "design-no-em-dash-in-jsx-text";
const RULE_SHA =
  "1e5908c52d9fc72ed12363e90fc0470b40267b9432009d0b322fb5b175ecda86";
const MESSAGE = "Em dash (—) in UI text reads like AI output to your users.";
const semanticTest = (name, run) =>
  test(`missing marker semantic correction: ${name}`, run);

function fixture(t, source) {
  const repoRoot = fs.mkdtempSync(
    path.join(os.tmpdir(), "doctor-marker-proof-"),
  );
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  const file = "web/app/src/renamed-feature/arbitrary-identity.tsx";
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
  const add = (anchor = "—", occurrence = 0) => {
    const source = sources.get(file);
    let offset = -1;
    for (let index = 0; index <= occurrence; index++)
      offset = source.indexOf(anchor, offset + 1);
    assert.ok(offset >= 0, `missing diagnostic anchor: ${anchor}`);
    const before = source.slice(0, offset);
    const diagnostic = {
      filePath: file.slice("web/app/".length),
      plugin: "react-doctor",
      rule: RULE,
      severity: "warning",
      message: MESSAGE,
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
      "retain every raw diagnostic, position, message and severity",
    );
    for (const [file, source] of sources)
      assert.equal(
        fs.readFileSync(path.join(repoRoot, file), "utf8"),
        source,
        "proof must preserve UI copy, data values and review inventory",
      );
    assert.deepEqual(
      JSON.parse(fs.readFileSync(path.join(repoRoot, REVIEW_FILE))),
      manifest,
      "never add a file, field, component, method or rule identity exemption",
    );
    return result;
  };
  return { repoRoot, file, sources, write, add, doctorReport, evaluate };
}

function assertProof(f, diagnostic, snippets) {
  const result = f.evaluate();
  assert.equal(result.status, "reviewed");
  assert.equal(result.findings.length, 0);
  assert.equal(result.upstreamCorrections.length, 1);
  const proof = result.upstreamCorrections[0];
  assert.deepEqual(proof.diagnostic, diagnostic);
  assert.equal(proof.predicate, "missing-value-marker");
  assert.equal(proof.provenance.toolPackage, TOOL);
  assert.equal(proof.provenance.packageIntegrity, INTEGRITY);
  assert.equal(proof.provenance.sourceCommit, COMMIT);
  assert.equal(proof.provenance.ruleSourceSha256, RULE_SHA);
  assert.ok(
    proof.provenance.references.includes(
      `https://github.com/millionco/react-doctor/blob/${COMMIT}/packages/oxlint-plugin-react-doctor/src/plugin/rules/react-ui/no-em-dash-in-jsx-text.ts`,
    ),
  );
  assert.ok(proof.reason.length > 20);
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
  for (const text of snippets)
    assert.ok(
      proof.evidence.some((entry) =>
        entry.spans.some(
          ({ start, end }) =>
            f.sources.get(entry.file).slice(start, end) === text,
        ),
      ),
      `missing exact evidence span: ${text}`,
    );
  return proof;
}

function assertUnproved(f, toolPackage = TOOL) {
  const result = f.evaluate(toolPackage);
  assert.equal(result.status, "requires-review");
  assert.equal(result.findings.length, 1);
  assert.equal(result.upstreamCorrections?.length ?? 0, 0);
}

for (const [name, value, marker] of [
  ["fragment marker", "value", "<>—</>"],
  [
    "native marker with a renamed value",
    "receipt",
    '<span className="missing">—</span>',
  ],
])
  semanticTest(
    `proves the whole missing branch instead of prose punctuation: ${name}`,
    (t) => {
      const guard = `${value} == null || ${value} === ''`;
      const present = `<span>{${value}}</span>`;
      const f = fixture(
        t,
        `function ArbitraryOwner({ ${value} }: { ${value}: string | null | undefined }) {
  if (${guard}) return ${marker};
  return ${present};
}`,
      );
      const diagnostic = f.add();
      assertProof(f, diagnostic, [guard, marker, present]);
    },
  );

semanticTest(
  "proves the current RuntimeIdentity source at an unrelated path without changing any UI copy",
  (t) => {
    const source = fs.readFileSync(
      path.resolve(
        __dirname,
        "../../../../web/app/src/features/settings/pages/events/runtime/RuntimeIdentity.tsx",
      ),
      "utf8",
    );
    const f = fixture(t, source);
    const diagnostic = f.add();
    assertProof(f, diagnostic, [
      "value == null || value === ''",
      "<>—</>",
      '<span className="event-runtime-identity__value">{value}</span>',
    ]);
    assert.equal(f.sources.get(f.file), source);
  },
);

for (const [name, body, anchor] of [
  ["ordinary prose", "return <p>Wait — retry later</p>;", "Wait — retry later"],
  [
    "prose inside a missing branch",
    "if (value == null || value === '') return <>Missing — retry later</>; return <span>{value}</span>;",
    "Missing — retry later",
  ],
  ["unguarded marker", "return <>—</>;"],
  [
    "guarded by an unrelated flag",
    "if (hidden) return <>—</>; return <span>{value}</span>;",
  ],
  [
    "different guarded value",
    "if (other == null || other === '') return <>—</>; return <span>{value}</span>;",
  ],
  [
    "different present value",
    "if (value == null || value === '') return <>—</>; return <span>{other}</span>;",
  ],
  [
    "no present value evidence",
    "if (value == null || value === '') return <>—</>; return <span>Available</span>;",
  ],
  [
    "reassigned guarded parameter",
    "value = other; if (value == null || value === '') return <>—</>; return <span>{value}</span>;",
  ],
  [
    "mutable local value",
    "let receipt = value; receipt = other; if (receipt == null || receipt === '') return <>—</>; return <span>{receipt}</span>;",
  ],
  [
    "mutable property reads",
    "if (state.value == null || state.value === '') return <>—</>; return <span>{state.value}</span>;",
  ],
  [
    "falsy guard also accepts present zero and false",
    "if (!value) return <>—</>; return <span>{value}</span>;",
  ],
  [
    "guard can also accept present values",
    "if (value == null || hidden) return <>—</>; return <span>{value}</span>;",
  ],
  [
    "present guard inverts marker meaning",
    "if (value != null && value !== '') return <>—</>; return <span>{value}</span>;",
  ],
  [
    "additional native child",
    "if (value == null || value === '') return <><span>Missing</span>—</>; return <span>{value}</span>;",
  ],
  [
    "additional dynamic child",
    "if (value == null || value === '') return <>{other}—</>; return <span>{value}</span>;",
  ],
  [
    "two marker characters",
    "if (value == null || value === '') return <>——</>; return <span>{value}</span>;",
    "——",
  ],
  [
    "opaque marker wrapper",
    "if (value == null || value === '') return <Opaque>—</Opaque>; return <span>{value}</span>;",
  ],
  [
    "marker inside a nested callback",
    "if (value == null || value === '') { const callback = () => <>—</>; return callback(); } return <span>{value}</span>;",
  ],
])
  semanticTest(
    `keeps genuine prose or an unproved missing-value premise unresolved: ${name}`,
    (t) => {
      const f = fixture(
        t,
        `function ArbitraryOwner({ value, other, hidden, state }) { ${body} }`,
      );
      f.add(anchor);
      assertUnproved(f);
    },
  );

semanticTest(
  "a neighboring valid marker cannot authorize prose in the same source",
  (t) => {
    const f = fixture(
      t,
      `function ArbitraryOwner({ value }) {
  if (value == null || value === '') return <>—</>;
  return <span>{value} — saved</span>;
}`,
    );
    f.add(" — saved");
    assertUnproved(f);
  },
);

semanticTest(
  "reassesses changed source instead of reusing a previously proved missing-value guard",
  (t) => {
    const source = `function ArbitraryOwner({ value, other }) {
  if (value == null || value === '') return <>—</>;
  return <span>{value}</span>;
}`;
    const f = fixture(t, source);
    const diagnostic = f.add();
    assertProof(f, diagnostic, [
      "value == null || value === ''",
      "<>—</>",
      "<span>{value}</span>",
    ]);
    f.write(
      f.file,
      source.replace(
        "value == null || value === ''",
        "other == null || other === ''",
      ),
    );
    // The replacement preserves source length and the original marker coordinates.
    assertUnproved(f);
  },
);

semanticTest(
  "fails closed on unknown pin, diagnostic revision, source path and exact JSXText location",
  (t) => {
    const f = fixture(
      t,
      `function ArbitraryOwner({ value }) {
  if (value == null || value === '') return <>—</>;
  return <span>{value}</span>;
}`,
    );
    const diagnostic = f.add();
    assertUnproved(f, "react-doctor@future");
    const original = structuredClone(diagnostic);
    for (const mutation of [
      { plugin: "unknown" },
      { rule: "unknown" },
      { message: "different rule revision" },
      { filePath: "../../outside.tsx" },
      { line: 99999 },
      { column: 99999 },
      { column: original.column + 1 },
    ]) {
      Object.assign(diagnostic, original, mutation);
      assertUnproved(f);
    }
    Object.assign(diagnostic, original);
    f.write(f.file, "export const invalid = (");
    assertUnproved(f);
  },
);
