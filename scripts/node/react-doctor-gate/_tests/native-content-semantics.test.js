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
const RULE = "prefer-tag-over-role";
const RULE_SHA =
  "a424acbda97ead457a61ba3b06ffade0b2af6c755b0ad4d147de08717507d4ca";
const COMMIT = "2d1ab245f0cbaa87529067e6e3ed9ac08362d1fc";
const TAGS = {
  status: "output",
  table: "table",
  row: "tr",
  columnheader: "th",
  cell: "td",
};
const semanticTest = (name, fn) =>
  test(`native content semantic correction: ${name}`, fn);

const antdRoot = fs.realpathSync(
  path.resolve(__dirname, "../../../../web/app/node_modules/antd"),
);
const listyRoot = fs.realpathSync(
  path.join(path.dirname(antdRoot), "@rc-component/listy"),
);
const virtualRoot = fs.realpathSync(
  path.join(
    path.dirname(path.dirname(listyRoot)),
    "@rc-component/virtual-list",
  ),
);
const contractSources = new Map();
for (const [root, name, files] of [
  [
    antdRoot,
    "antd",
    [
      "package.json",
      "es/index.js",
      "es/spin/index.js",
      "es/progress/index.js",
      "es/progress/progress.js",
      "es/skeleton/index.js",
      "es/skeleton/Skeleton.js",
      "es/listy/index.js",
    ],
  ],
  [
    listyRoot,
    "@rc-component/listy",
    ["package.json", "es/index.js", "es/List.js", "es/VirtualList/index.js"],
  ],
  [
    virtualRoot,
    "@rc-component/virtual-list",
    ["package.json", "es/index.js", "es/List.js", "es/Filler.js"],
  ],
])
  for (const file of files)
    contractSources.set(
      `web/app/node_modules/${name}/${file}`,
      fs.readFileSync(path.join(root, file), "utf8"),
    );
contractSources.set(
  "web/pnpm-lock.yaml",
  fs.readFileSync(
    path.resolve(__dirname, "../../../../web/pnpm-lock.yaml"),
    "utf8",
  ),
);

function fixture(t, source, libraries = false) {
  const repoRoot = fs.mkdtempSync(
    path.join(os.tmpdir(), "doctor-native-content-"),
  );
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  const file = "web/app/src/arbitrary-native-content-owner.tsx";
  const sources = new Map();
  const write = (name, body) => {
    fs.mkdirSync(path.dirname(path.join(repoRoot, name)), { recursive: true });
    fs.writeFileSync(path.join(repoRoot, name), body);
    sources.set(name, body);
  };
  write(file, source);
  if (libraries) for (const [name, body] of contractSources) write(name, body);
  const manifest = { schemaVersion: 1, toolPackage: TOOL, reviews: [] };
  write(REVIEW_FILE, JSON.stringify(manifest));
  const doctorReport = {
    diagnostics: [],
    summary: { totalDiagnosticCount: 0, suppressedDiagnosticCount: 0 },
    error: null,
  };
  const add = (role, anchor = `role="${role}"`) => {
    const offset = sources.get(file).indexOf(anchor);
    assert.ok(offset >= 0, `missing diagnostic anchor: ${anchor}`);
    const before = sources.get(file).slice(0, offset);
    const diagnostic = {
      filePath: file.slice("web/app/".length),
      plugin: "react-doctor",
      rule: RULE,
      severity: "warning",
      message: `Screen reader users get more reliable semantics from \`<${TAGS[role]}>\` than \`role="${role}"\`, so use \`<${TAGS[role]}>\` instead.`,
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
      "retain all raw diagnostics, count, severity, message and coordinates",
    );
    assert.deepEqual(
      JSON.parse(fs.readFileSync(path.join(repoRoot, REVIEW_FILE))),
      manifest,
      "do not add source, component, method or rule identity exceptions",
    );
    return result;
  };
  return { repoRoot, file, sources, write, add, evaluate, doctorReport };
}
function assertProof(f, diagnostic, predicate, spans, expectedFiles = []) {
  const result = f.evaluate();
  assert.equal(result.status, "reviewed");
  assert.equal(result.findings.length, 0);
  assert.equal(result.upstreamCorrections.length, 1);
  const proof = result.upstreamCorrections[0];
  assert.deepEqual(proof.diagnostic, diagnostic);
  assert.equal(proof.predicate, predicate);
  assert.equal(proof.provenance.toolPackage, TOOL);
  assert.equal(proof.provenance.sourceCommit, COMMIT);
  assert.equal(proof.provenance.ruleSourceSha256, RULE_SHA);
  assert.match(proof.provenance.packageIntegrity, /^sha512-/u);
  assert.ok(
    proof.provenance.references.some((url) =>
      url.startsWith("https://html.spec.whatwg.org/"),
    ),
  );
  assert.ok(proof.evidence.length > 0);
  for (const entry of proof.evidence) {
    const source = f.sources.get(entry.file);
    assert.equal(
      typeof source,
      "string",
      "inspect every cited source or installed-contract file",
    );
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
  for (const span of spans)
    assert.ok(
      proof.evidence.some((entry) =>
        entry.spans.some(
          ({ start, end }) =>
            f.sources.get(entry.file).slice(start, end) === span,
        ),
      ),
      `missing exact source witness: ${span}`,
    );
  for (const file of expectedFiles)
    assert.ok(
      proof.evidence.some((entry) => entry.file === file),
      `missing inspected contract: ${file}`,
    );
  return proof;
}
function assertUnproved(f) {
  const result = f.evaluate();
  assert.equal(result.status, "requires-review");
  assert.equal(result.upstreamCorrections?.length ?? 0, 0);
}

for (const [name, child] of [
  ["native flow child", "<div>Background operation</div>"],
  ["native paragraph child", "<p>Background operation</p>"],
])
  semanticTest(
    `proves the proposed output rejects existing content: ${name}`,
    (t) => {
      const f = fixture(
        t,
        `function Owner() { return <div role="status">${child}</div>; }`,
      );
      const diagnostic = f.add("status");
      assertProof(f, diagnostic, "native-status-content-model-conflict", [
        'role="status"',
        child,
      ]);
    },
  );

for (const [name, imports, child, contract] of [
  [
    "standalone Spin",
    "import { Spin } from 'antd';",
    '<Spin size="small" />',
    "es/spin/index.js",
  ],
  [
    "renamed Spin import",
    "import { Spin as Busy } from 'antd';",
    '<Busy size="small" />',
    "es/spin/index.js",
  ],
  [
    "immutable Spin alias",
    "import { Spin } from 'antd'; const Busy = Spin;",
    "<Busy />",
    "es/spin/index.js",
  ],
  [
    "namespace Spin import",
    "import * as A from 'antd';",
    "<A.Spin />",
    "es/spin/index.js",
  ],
  [
    "Progress host",
    "import { Progress } from 'antd';",
    "<Progress percent={percent} />",
    "es/progress/progress.js",
  ],
  [
    "Skeleton default loading branch",
    "import { Skeleton } from 'antd';",
    "<Skeleton active />",
    "es/skeleton/Skeleton.js",
  ],
])
  semanticTest(
    `proves the imported render contract rather than the component name: ${name}`,
    (t) => {
      const f = fixture(
        t,
        `${imports}
function Owner({ percent }) { return <div role="status">${child}</div>; }`,
        true,
      );
      const diagnostic = f.add("status");
      assertProof(
        f,
        diagnostic,
        "native-status-content-model-conflict",
        ['role="status"', child],
        [
          "web/pnpm-lock.yaml",
          "web/app/node_modules/antd/package.json",
          `web/app/node_modules/antd/${contract}`,
        ],
      );
    },
  );

for (const [name, source, libraries] of [
  ["plain result text", '<div role="status">Total: 42</div>'],
  [
    "phrasing-only children",
    '<div role="status"><span>Result <b>42</b></span></div>',
  ],
  [
    "native form result control",
    '<span role="status"><input readOnly value="42" /></span>',
  ],
  ["opaque composite child", '<div role="status"><UnknownWidget /></div>'],
  ["same-named local Spin", '<div role="status"><Spin /></div>', false],
  ["shadowed imported Spin", '<div role="status"><Busy /></div>', true],
  [
    "unknown child spread props",
    '<div role="status"><Spin {...props} /></div>',
    true,
  ],
  [
    "root role spread override",
    '<div role="status" {...props}><Spin /></div>',
    true,
  ],
  [
    "unreachable flow branch",
    '<div role="status">{false && <div>Never rendered</div>}</div>',
  ],
  [
    "Skeleton explicitly not loading",
    '<div role="status"><Skeleton loading={false}><span>Ready</span></Skeleton></div>',
    true,
  ],
  [
    "Skeleton unknown loading branch",
    '<div role="status"><Skeleton loading={loading}>{children}</Skeleton></div>',
    true,
  ],
  [
    "unproved polymorphic native alternative",
    '<div role="status"><Spin as={host} /></div>',
    true,
  ],
])
  semanticTest(
    `keeps native-convertible or unknown status content unresolved: ${name}`,
    (t) => {
      const declarations =
        name === "same-named local Spin"
          ? "const Spin = () => <span>Ready</span>;"
          : libraries
            ? "import { Spin, Skeleton, Spin as Busy } from 'antd';"
            : "";
      const params =
        name === "shadowed imported Spin"
          ? "{ Busy, props, loading, children, host }"
          : "{ props, loading, children, host }";
      const f = fixture(
        t,
        `${declarations} function Owner(${params}) { return ${source}; }`,
        libraries,
      );
      f.add("status");
      assertUnproved(f);
    },
  );

for (const [name, mutate] of [
  [
    "modified installed renderer",
    (f) => {
      const p = "web/app/node_modules/antd/es/spin/index.js";
      f.write(p, f.sources.get(p) + "\n// modified artifact\n");
    },
  ],
  [
    "modified installed package version",
    (f) => {
      const p = "web/app/node_modules/antd/package.json";
      const pkg = JSON.parse(f.sources.get(p));
      pkg.version = "6.6.3";
      f.write(p, JSON.stringify(pkg));
    },
  ],
  [
    "modified lockfile integrity",
    (f) => {
      const p = "web/pnpm-lock.yaml";
      f.write(
        p,
        f.sources
          .get(p)
          .replace(
            "sha512-aTOPWXsqfWrlSiI0a1qR9UWR4jBCTJ8FNZfZmYQP9/aGGLJSrw9/c5uE+4vSVSt3riQlIPAwR3BcAgEHmE7GMg==",
            "sha512-unknown",
          ),
      );
    },
  ],
])
  semanticTest(`fails closed on ${name}`, (t) => {
    const f = fixture(
      t,
      "import { Spin } from 'antd'; function Owner() { return <div role=\"status\"><Spin /></div>; }",
      true,
    );
    mutate(f);
    f.add("status");
    assertUnproved(f);
  });

function virtualTable({
  imports = "import { Listy } from 'antd';",
  widget = "Listy",
  virtual = "virtual",
  rootProps = "",
  tableName = 'aria-label="Entries"',
  slot = 'itemRender={(row, index) => <div role="row" aria-rowindex={index + 2}><div role="cell">{row.name}</div></div>}',
  before = "",
  after = "",
} = {}) {
  return `${imports}
function Owner({ rows, total, props }) {
  ${before}
  return <div role="table" ${tableName} aria-rowcount={total + 1} aria-colcount={1} ${rootProps}>
    <div role="row"><div role="columnheader">Name</div></div>
    <div className="viewport"><${widget} ${virtual} height={120} items={rows} rowKey="id" ${slot} /></div>
    ${after}
  </div>;
}`;
}
for (const [name, role, anchor] of [
  ["table", "table", 'role="table"'],
  ["header row", "row", 'role="row"><div role="columnheader"'],
  ["column header", "columnheader", 'role="columnheader"'],
  ["virtualized data row", "row", 'role="row" aria-rowindex'],
  ["virtualized data cell", "cell", 'role="cell"'],
])
  semanticTest(
    `proves coordinated native replacement still breaks the virtual content model: ${name}`,
    (t) => {
      const f = fixture(t, virtualTable(), true);
      const diagnostic = f.add(role, anchor);
      assertProof(
        f,
        diagnostic,
        "virtual-table-content-model-conflict",
        ['role="table"', 'role="cell"'],
        [
          "web/pnpm-lock.yaml",
          "web/app/node_modules/antd/es/listy/index.js",
          "web/app/node_modules/@rc-component/listy/es/VirtualList/index.js",
          "web/app/node_modules/@rc-component/virtual-list/es/List.js",
          "web/app/node_modules/@rc-component/virtual-list/es/Filler.js",
        ],
      );
    },
  );

semanticTest(
  "a renamed imported virtualizer retains its source-bound contract",
  (t) => {
    const f = fixture(
      t,
      virtualTable({
        imports: "import { Listy as Window } from 'antd';",
        widget: "Window",
      }),
      true,
    );
    const diagnostic = f.add("table");
    assertProof(
      f,
      diagnostic,
      "virtual-table-content-model-conflict",
      ['role="table"'],
      ["web/app/node_modules/@rc-component/listy/es/VirtualList/index.js"],
    );
  },
);

for (const [name, options] of [
  ["opaque virtualizer", { imports: "", widget: "UnknownWindow" }],
  ["shadowed imported virtualizer", { before: "const Listy = unknownWindow;" }],
  ["unknown renderer slot", { slot: "itemRender={externalRenderer}" }],
  [
    "non-contract render slot",
    {
      slot: 'renderRow={(row) => <div role="row"><div role="cell">{row.name}</div></div>}',
    },
  ],
  ["virtualization disabled", { virtual: "virtual={false}" }],
  [
    "unproved polymorphic virtual host",
    { virtual: 'virtual component="tbody"' },
  ],
  ["unknown root role override", { rootProps: "{...props}" }],
  ["missing accessible table name", { tableName: "" }],
  ["empty accessible table name", { tableName: 'aria-label=""' }],
  [
    "cell lacks a row owner",
    { slot: 'itemRender={row => <div role="cell">{row.name}</div>}' },
  ],
  [
    "opaque row ownership wrapper",
    {
      slot: 'itemRender={row => <OpaqueRow><div role="cell">{row.name}</div></OpaqueRow>}',
    },
  ],
])
  semanticTest(
    `keeps unknown or invalid virtual ARIA structure unresolved: ${name}`,
    (t) => {
      const f = fixture(t, virtualTable(options), true);
      f.add("table");
      assertUnproved(f);
    },
  );

for (const role of ["table", "row", "columnheader", "cell"])
  semanticTest(
    `a directly native-convertible ARIA ${role} remains actionable`,
    (t) => {
      const f = fixture(
        t,
        `function Owner() { return <div role="table" aria-label="Entries">
    <div role="row"><div role="columnheader">Name</div></div>
    <div role="row"><div role="cell">Entry</div></div>
  </div>; }`,
      );
      f.add(role);
      assertUnproved(f);
    },
  );

semanticTest(
  "a plain table is not authorized by a neighboring virtual table",
  (t) => {
    const plain =
      '<div role="table" aria-label="Plain"><div role="row"><div role="cell">Plain entry</div></div></div>';
    const f = fixture(
      t,
      virtualTable()
        .replace(
          'return <div role="table"',
          `return <><aside>${plain}</aside><div role="table"`,
        )
        .replace("</div>;\n}", "</div></>;\n}"),
      true,
    );
    f.add("cell", 'role="cell">Plain entry');
    assertUnproved(f);
  },
);

semanticTest(
  "an unused row function is not the virtualizer render slot",
  (t) => {
    const f = fixture(
      t,
      virtualTable({
        before:
          'const unused = row => <div role="row"><div role="cell">Unused</div></div>;',
      }),
      true,
    );
    f.add("cell", 'role="cell">Unused');
    assertUnproved(f);
  },
);

semanticTest(
  "a changed virtual host artifact invalidates the source-bound table proof",
  (t) => {
    const f = fixture(t, virtualTable(), true);
    const p = "web/app/node_modules/@rc-component/virtual-list/es/Filler.js";
    f.write(p, f.sources.get(p) + "\n// modified contract artifact\n");
    f.add("table");
    assertUnproved(f);
  },
);

semanticTest(
  "fails closed on unknown tool, diagnostic revision and exact source position",
  (t) => {
    const f = fixture(
      t,
      'function Owner() { return <div role="status"><div>Pending</div></div>; }',
    );
    const diagnostic = f.add("status");
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
  "real pinned AntD renders flow hosts while a non-loading Skeleton can render phrasing content",
  () => {
    const requireApp = createRequire(
      path.resolve(__dirname, "../../../../web/app/package.json"),
    );
    const requireWeb = createRequire(
      path.resolve(__dirname, "../../../../web/package.json"),
    );
    const React = requireApp("react");
    const { renderToStaticMarkup } = requireApp("react-dom/server");
    const { Spin, Progress, Skeleton } = requireApp("antd");
    const { JSDOM } = requireWeb("jsdom");
    for (const [Component, props, selector] of [
      [Spin, { size: "small" }, ".ant-spin"],
      [Progress, { percent: 42 }, ".ant-progress"],
      [Skeleton, { active: true }, ".ant-skeleton"],
    ]) {
      const markup = renderToStaticMarkup(
        React.createElement(Component, props),
      );
      assert.equal(
        JSDOM.fragment(markup).querySelector(selector).tagName,
        "DIV",
      );
      const invalidOutput = JSDOM.fragment(
        `<output>${markup}</output>`,
      ).querySelector("output");
      assert.ok(
        invalidOutput.querySelector("div"),
        "the proposed native wrapper contains non-phrasing content",
      );
    }
    const nativeControl = renderToStaticMarkup(
      React.createElement(
        Skeleton,
        { loading: false },
        React.createElement("span", null, "Ready"),
      ),
    );
    assert.equal(
      JSDOM.fragment(nativeControl).firstElementChild.tagName,
      "SPAN",
    );
  },
);

semanticTest(
  "native table SSR parsing repairs an invalid virtual wrapper; a valid table control keeps its structure",
  () => {
    const requireApp = createRequire(
      path.resolve(__dirname, "../../../../web/app/package.json"),
    );
    const requireWeb = createRequire(
      path.resolve(__dirname, "../../../../web/package.json"),
    );
    const React = requireApp("react");
    const { renderToStaticMarkup } = requireApp("react-dom/server");
    const { JSDOM } = requireWeb("jsdom");
    const markup = renderToStaticMarkup(
      React.createElement(
        "table",
        null,
        React.createElement(
          "div",
          { "data-window": true },
          React.createElement(
            "div",
            { "data-holder": true },
            React.createElement(
              "tr",
              null,
              React.createElement("td", null, "Entry"),
            ),
          ),
        ),
      ),
    );
    const parsed = JSDOM.fragment(markup);
    assert.ok(parsed.querySelector("table tr td"));
    assert.equal(
      parsed.querySelector("[data-window] tr"),
      null,
      "browser table repair moves the row out of its scroll/window ancestor",
    );
    const valid = JSDOM.fragment(
      renderToStaticMarkup(
        React.createElement(
          "table",
          null,
          React.createElement(
            "tbody",
            null,
            React.createElement(
              "tr",
              null,
              React.createElement("td", null, "Entry"),
            ),
          ),
        ),
      ),
    );
    assert.equal(valid.querySelector("td").parentElement.tagName, "TR");
    assert.equal(valid.querySelector("tr").parentElement.tagName, "TBODY");
    assert.equal(valid.querySelector("tbody").parentElement.tagName, "TABLE");
  },
);

semanticTest(
  "real pinned Listy preserves ARIA descendant ownership through neutral div virtual hosts",
  () => {
    const requireApp = createRequire(
      path.resolve(__dirname, "../../../../web/app/package.json"),
    );
    const requireWeb = createRequire(
      path.resolve(__dirname, "../../../../web/package.json"),
    );
    const React = requireApp("react");
    const { renderToStaticMarkup } = requireApp("react-dom/server");
    const { Listy } = requireApp("antd");
    const { JSDOM } = requireWeb("jsdom");
    const markup = renderToStaticMarkup(
      React.createElement(
        "div",
        { role: "table", "aria-label": "Entries", "aria-rowcount": 1 },
        React.createElement(Listy, {
          virtual: true,
          height: 120,
          items: [{ id: "entry", name: "Entry" }],
          rowKey: "id",
          itemRender: (row) =>
            React.createElement(
              "div",
              { role: "row", "aria-rowindex": 1 },
              React.createElement("div", { role: "cell" }, row.name),
            ),
        }),
      ),
    );
    const parsed = JSDOM.fragment(markup);
    const row = parsed.querySelector('[role="row"]');
    assert.ok(row);
    assert.equal(row.parentElement.tagName, "DIV");
    assert.equal(
      row.parentElement.closest("[role]").getAttribute("role"),
      "table",
    );
    assert.equal(row.querySelector('[role="cell"]').parentElement, row);
    assert.ok(parsed.querySelector(".ant-listy-holder-inner"));
  },
);
