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
const SOURCE_COMMIT = "2d1ab245f0cbaa87529067e6e3ed9ac08362d1fc";
const RULE_HASHES = {
  "rendering-hydration-mismatch-time":
    "26a97648e84587588c40d4bc2b2f547928c708aecd4f806a977b4da3a51a4fc7",
  "prefer-tag-over-role":
    "a424acbda97ead457a61ba3b06ffade0b2af6c755b0ad4d147de08717507d4ca",
  "click-events-have-key-events":
    "853dc1db4c6f4ae5af0d9bda8aefd826f7d7841b8678835b4858bd7be8ac86a3",
  "no-static-element-interactions":
    "6b3b9ae205ce6a3b85e443d05f2fb9ee6dd4bc897744fc68fce3e99fe8c16831",
  "js-tosorted-immutable":
    "296d54afccde391f23130786c6f601806df3925f1c0983062f6537d017183e48",
  "js-hoist-intl":
    "ad9c0a9d6c992567e6cba6c7a14d852c41c57624d8e743c8206bc7e5b1f4ecf0",
};
const HYDRATION_RULE = "rendering-hydration-mismatch-time";
const SORT_RULE = "js-tosorted-immutable";
const GRAPHIC_RULE = "prefer-tag-over-role";
const INTL_RULE = "js-hoist-intl";
const intlMessage = (name) =>
  `This is slow because new Intl.${name}() rebuilds on every call inside a function, so move it to the top of the file, or wrap it in useMemo`;
const PROPAGATION_RULES = [
  "click-events-have-key-events",
  "no-static-element-interactions",
];
const messages = {
  [HYDRATION_RULE]:
    "This breaks hydration because crypto.randomUUID() reached from JSX gives a different value on the server than in the browser, so move it into useEffect+useState to run only in the browser, or add suppressHydrationWarning to the parent if it's on purpose",
  [GRAPHIC_RULE]:
    'Screen reader users get more reliable semantics from `<img>` than `role="img"`, so use `<img>` instead.',
  "click-events-have-key-events":
    "Keyboard users can't trigger this click handler because there's no keyboard one, so add `onKeyUp`, `onKeyDown`, or `onKeyPress`.",
  "no-static-element-interactions":
    "Screen reader users can't tell this click handler is interactive because it has no `role`, so add a `role` or use a button or link.",
  [SORT_RULE]:
    "This wastes work because [...array].sort() copies the array just to sort it, so use array.toSorted() to sort without the extra copy (ES2023)",
  [INTL_RULE]: intlMessage("NumberFormat"),
};
const semanticTest = (name, fn) =>
  test(`render semantic correction: ${name}`, fn);

function fixture(t, source, file = "web/app/src/unrelated-owner.tsx") {
  const repoRoot = fs.mkdtempSync(
    path.join(os.tmpdir(), "doctor-render-proof-"),
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
    assert.ok(offset >= 0, `missing diagnostic anchor: ${anchor}`);
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
      "raw messages, severity, positions and inventory must remain intact",
    );
    assert.deepEqual(
      JSON.parse(fs.readFileSync(path.join(repoRoot, REVIEW_FILE))),
      manifest,
      "no file, rule or component identity review exceptions may be created",
    );
    return result;
  };
  return { repoRoot, file, sources, write, doctorReport, add, evaluate };
}

function assertProof(f, diagnostic, predicate, exactSpans) {
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
  assert.ok(proof.provenance.references.length > 0);
  for (const evidence of proof.evidence) {
    const source = f.sources.get(evidence.file);
    assert.equal(
      typeof source,
      "string",
      "evidence must be an inspected source/configuration file",
    );
    assert.equal(evidence.sha256, sourceDigest(source));
    assert.ok(evidence.spans.length > 0);
    for (const span of evidence.spans) {
      assert.ok(Number.isInteger(span.start) && Number.isInteger(span.end));
      assert.ok(
        span.start >= 0 && span.start < span.end && span.end <= source.length,
      );
    }
  }
  for (const text of exactSpans) {
    assert.ok(
      proof.evidence.some((entry) =>
        entry.spans.some(
          (span) =>
            f.sources.get(entry.file).slice(span.start, span.end) === text,
        ),
      ),
      `missing exact evidence span: ${text}`,
    );
  }
  return proof;
}
function assertUnproved(f) {
  const result = f.evaluate();
  assert.equal(result.status, "requires-review");
  assert.equal(result.upstreamCorrections?.length ?? 0, 0);
}

for (const [name, imports, tag, handler] of [
  ["native arrow", "", "button", "() => consume(crypto.randomUUID())"],
  [
    "native function expression",
    "",
    "button",
    "function () { consume(crypto.randomUUID()); }",
  ],
  [
    "imported renamed AntD Button",
    "import { Button as Action } from 'antd';",
    "Action",
    "() => consume(crypto.randomUUID())",
  ],
  [
    "stable AntD Button alias",
    "import { Button } from 'antd'; const Action = Button;",
    "Action",
    "() => consume(crypto.randomUUID())",
  ],
]) {
  semanticTest(`proves deferred click callback: ${name}`, (t) => {
    const source = `${imports}\nexport function Panel() { return <${tag} onClick={${handler}}>Add</${tag}>; }`;
    const f = fixture(t, source);
    const diagnostic = f.add(HYDRATION_RULE, "crypto.randomUUID()");
    assertProof(f, diagnostic, "deferred-event-callback", [
      "crypto.randomUUID()",
      `onClick={${handler}}`,
    ]);
  });
}

for (const [name, source, anchor, direct] of [
  [
    "direct JSX value",
    "function Panel() { return <div>{crypto.randomUUID()}</div>; }",
    "{crypto.randomUUID()}",
    true,
  ],
  [
    "IIFE child",
    "function Panel() { return <div>{(() => crypto.randomUUID())()}</div>; }",
    "crypto.randomUUID()",
  ],
  [
    "IIFE handler factory",
    "function Panel() { return <button onClick={(() => crypto.randomUUID())()}>Add</button>; }",
    "crypto.randomUUID()",
  ],
  [
    "useMemo factory",
    "import { useMemo } from 'react'; function Panel() { return <div>{useMemo(() => crypto.randomUUID(), [])}</div>; }",
    "crypto.randomUUID()",
  ],
  [
    "lazy useState initializer",
    "import { useState } from 'react'; function Panel() { return <div>{useState(() => crypto.randomUUID())[0]}</div>; }",
    "crypto.randomUUID()",
  ],
  [
    "synchronous iterator callback",
    "function Panel({ items }) { return <div>{items.map(() => crypto.randomUUID())}</div>; }",
    "crypto.randomUUID()",
  ],
  [
    "unknown callback executor",
    "function Panel({ execute }) { return <div>{execute(() => crypto.randomUUID())}</div>; }",
    "crypto.randomUUID()",
  ],
  [
    "opaque composite onClick",
    "function Panel({ Action }) { return <Action onClick={() => consume(crypto.randomUUID())} />; }",
    "crypto.randomUUID()",
  ],
  [
    "shadowed AntD Button",
    "import { Button } from 'antd'; function Panel({ Button }) { return <Button onClick={() => consume(crypto.randomUUID())} />; }",
    "crypto.randomUUID()",
  ],
  [
    "local composite executes prop in render",
    "function Action({ onClick }) { onClick(); return <span />; } function Panel() { return <Action onClick={() => consume(crypto.randomUUID())} />; }",
    "crypto.randomUUID()",
  ],
]) {
  semanticTest(
    `keeps actual or unproved render execution unresolved: ${name}`,
    (t) => {
      const f = fixture(t, source);
      f.add(
        HYDRATION_RULE,
        anchor,
        direct
          ? messages[HYDRATION_RULE].replace("reached from JSX", "in JSX")
          : messages[HYDRATION_RULE],
      );
      assertUnproved(f);
    },
  );
}

for (const rule of PROPAGATION_RULES) {
  for (const [name, handler] of [
    [
      "guarded native target",
      "event => { if (event.target instanceof Element && event.target.closest('button, input')) event.stopPropagation(); }",
    ],
    [
      "renamed event argument",
      "receipt => { if (receipt.target instanceof Element && receipt.target.closest('button')) receipt.stopPropagation(); }",
    ],
  ]) {
    semanticTest(
      `proves propagation-only boundary for ${rule}: ${name}`,
      (t) => {
        const f = fixture(
          t,
          `function Panel() { return <div onClick={${handler}}><button>Child</button></div>; }`,
        );
        const diagnostic = f.add(rule, "div onClick");
        assertProof(f, diagnostic, "propagation-only-boundary", [handler]);
      },
    );
  }
}
for (const [name, handler, extra] of [
  [
    "activation after propagation",
    "event => { event.stopPropagation(); activate(); }",
  ],
  ["state mutation", "event => { event.stopPropagation(); count++; }"],
  [
    "analytics side effect",
    "event => { if (ready) event.stopPropagation(); track(); }",
  ],
  ["opaque guard", "event => { if (opaque(event)) event.stopPropagation(); }"],
  ["different receiver", "event => { other.stopPropagation(); }"],
  [
    "preventDefault is a different effect",
    "event => { event.preventDefault(); }",
  ],
  [
    "unknown sibling activation handler",
    "event => { event.stopPropagation(); }",
    "onMouseDown={() => activate()}",
  ],
]) {
  semanticTest(
    `keeps activation or unproved wrapper unresolved: ${name}`,
    (t) => {
      const f = fixture(
        t,
        `function Panel() { return <div onClick={${handler}} ${extra || ""}><button>Child</button></div>; }`,
      );
      f.add("no-static-element-interactions", "div onClick");
      assertUnproved(f);
    },
  );
}
semanticTest(
  "a shadowed Element cannot establish the native target-read contract",
  (t) => {
    const f = fixture(
      t,
      "function Panel({ Element }) { return <div onClick={event => { if (event.target instanceof Element && event.target.closest('button')) event.stopPropagation(); }} />; }",
    );
    f.add("click-events-have-key-events", "div onClick");
    assertUnproved(f);
  },
);

for (const [name, source, children] of [
  [
    "SVG graphic",
    '<span role="img" aria-label="Result"><svg aria-hidden="true"><path d="M0 0" /></svg></span>',
    '<svg aria-hidden="true"><path d="M0 0" /></svg>',
  ],
  [
    "text graphic grouping",
    '<span role="img" aria-label="Four stars"><span>★★★★</span></span>',
    "<span>★★★★</span>",
  ],
  [
    "conditional icons",
    '<span role="img" aria-label={label}>{pending ? <svg /> : <svg />}</span>',
    "{pending ? <svg /> : <svg />}",
  ],
]) {
  semanticTest(
    `proves native img cannot preserve composite children: ${name}`,
    (t) => {
      const f = fixture(
        t,
        `function Panel({ label, pending }) { return ${source}; }`,
      );
      const diagnostic = f.add(GRAPHIC_RULE, 'role="img"');
      assertProof(f, diagnostic, "composite-graphic-container", [
        'role="img"',
        children,
      ]);
    },
  );
}
semanticTest(
  "proves an imported visual icon remains composite graphic content",
  (t) => {
    const f = fixture(
      t,
      `import Glyph from '@ant-design/icons/es/icons/CheckCircleFilled';
    function Panel() { return <span role="img" aria-label="Result"><Glyph /></span>; }`,
    );
    const diagnostic = f.add(GRAPHIC_RULE, 'role="img"');
    assertProof(f, diagnostic, "composite-graphic-container", [
      'role="img"',
      "<Glyph />",
    ]);
  },
);
for (const [name, jsx, message] of [
  ["empty image wrapper", '<span role="img" aria-label="Result" />'],
  [
    "whitespace children only",
    '<span role="img" aria-label="Result">   </span>',
  ],
  ["missing accessible name", '<span role="img"><svg /></span>'],
  ["empty accessible name", '<span role="img" aria-label=""><svg /></span>'],
  [
    "interactive descendant",
    '<span role="img" aria-label="Result"><button>Activate</button></span>',
  ],
  [
    "opaque composite child",
    '<span role="img" aria-label="Result"><UnknownWidget /></span>',
  ],
  [
    "unknown role or name override",
    '<span role="img" aria-label="Result" {...props}><svg /></span>',
  ],
  [
    "different native role recommendation",
    '<span role="button" aria-label="Activate">Activate</span>',
    'Screen reader users get more reliable semantics from `<button>` than `role="button"`, so use `<button>` instead.',
  ],
]) {
  semanticTest(
    `keeps genuine or unproved graphic case unresolved: ${name}`,
    (t) => {
      const f = fixture(t, `function Panel({ props }) { return ${jsx}; }`);
      f.add(
        GRAPHIC_RULE,
        name === "different native role recommendation"
          ? 'role="button"'
          : 'role="img"',
        message || messages[GRAPHIC_RULE],
      );
      assertUnproved(f);
    },
  );
}

function baseline(f, options = { target: "ES2022", lib: ["ES2022", "DOM"] }) {
  const child = '{"extends":"../tsconfig.base.json"}';
  const parent = JSON.stringify({ compilerOptions: options });
  f.write("web/app/tsconfig.json", child);
  f.write("web/tsconfig.base.json", parent);
  return [child, parent];
}
for (const [name, options] of [
  ["inherited ES2022 lib", { target: "ES2022", lib: ["ES2022", "DOM"] }],
  [
    "explicit lib overrides newer syntax target",
    { target: "ESNext", lib: ["ES2022", "DOM"] },
  ],
]) {
  semanticTest(
    `proves copy-sort recommendation exceeds declared API baseline: ${name}`,
    (t) => {
      const source =
        "export function order(values: readonly number[]) { return [...values].sort((a, b) => a - b); }";
      const f = fixture(t, source);
      const configs = baseline(f, options);
      const diagnostic = f.add(SORT_RULE, "[...values].sort");
      assertProof(f, diagnostic, "immutable-sort-baseline", [
        "[...values].sort((a, b) => a - b)",
        ...configs,
      ]);
    },
  );
}
for (const [name, declaration, expression] of [
  [
    "Set materialization",
    "const values = new Set([3, 1]);",
    "[...values].sort()",
  ],
  [
    "Map value iterator materialization",
    "const groups = new Map();",
    "[...groups.values()].sort((a, b) => a.rank - b.rank)",
  ],
]) {
  semanticTest(
    `proves direct toSorted replacement is not an iterable API: ${name}`,
    (t) => {
      const f = fixture(
        t,
        `function order() { ${declaration} return ${expression}; }`,
      );
      const diagnostic = f.add(SORT_RULE, expression);
      assertProof(f, diagnostic, "immutable-sort-baseline", [
        declaration,
        expression,
      ]);
    },
  );
}
for (const [name, source, options] of [
  [
    "modern Array baseline",
    "function order(values: number[]) { return [...values].sort(); }",
    { target: "ES2023", lib: ["ES2023", "DOM"] },
  ],
  [
    "newer lib overrides older target",
    "function order(values: number[]) { return [...values].sort(); }",
    { target: "ES2022", lib: ["ES2023", "DOM"] },
  ],
  [
    "missing configuration and unknown iterable",
    "function order(values) { return [...values].sort(); }",
  ],
  [
    "shadowed Set constructor",
    "function order(Set) { const values = new Set(); return [...values].sort(); }",
  ],
  [
    "shadowed Map constructor",
    "function order(Map) { const groups = new Map(); return [...groups.values()].sort(); }",
  ],
  [
    "in-place mutation is not this counterexample",
    "function order(values: number[]) { return values.sort(); }",
    { target: "ES2022", lib: ["ES2022"] },
  ],
]) {
  semanticTest(
    `keeps genuine or unproved sort case unresolved: ${name}`,
    (t) => {
      const f = fixture(t, source);
      if (options) baseline(f, options);
      const anchor =
        name === "in-place mutation is not this counterexample"
          ? "values.sort()"
          : name === "shadowed Map constructor"
            ? "[...groups.values()].sort()"
            : "[...values].sort()";
      f.add(SORT_RULE, anchor);
      assertUnproved(f);
    },
  );
}
for (const [name, config] of [
  ["malformed config", '{"compilerOptions":'],
  ["cyclic extends", '{"extends":"./tsconfig.json"}'],
  ["missing parent", '{"extends":"../missing-config.json"}'],
]) {
  semanticTest(`fails closed on ${name}`, (t) => {
    const f = fixture(
      t,
      "function order(values: number[]) { return [...values].sort(); }",
    );
    f.write("web/app/tsconfig.json", config);
    f.add(SORT_RULE, "[...values].sort()");
    assertUnproved(f);
  });
}
semanticTest(
  "changing the config source removes a proof without storing an identity exception",
  (t) => {
    const f = fixture(
      t,
      "function order(values: number[]) { return [...values].sort(); }",
    );
    baseline(f);
    const diagnostic = f.add(SORT_RULE, "[...values].sort()");
    assertProof(f, diagnostic, "immutable-sort-baseline", [
      "[...values].sort()",
    ]);
    baseline(f, { target: "ES2023", lib: ["ES2023", "DOM"] });
    assertUnproved(f);
  },
);
semanticTest(
  "fails closed on unknown tool, diagnostic revision, source or location",
  (t) => {
    const f = fixture(
      t,
      "function Panel() { return <button onClick={() => consume(crypto.randomUUID())}>Add</button>; }",
    );
    const diagnostic = f.add(HYDRATION_RULE, "crypto.randomUUID()");
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
    f.write(f.file, "export const invalid = (");
    assertUnproved(f);
  },
);

const INTL_CACHE_SOURCE = `let retained;
export function formatter() {
  return (retained ??= new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 }));
}`;
const GUARDED_INTL_CACHE_SOURCE = `let observedLanguage;
let retained;
export function formatter() {
  const language = typeof navigator === 'undefined' ? undefined : navigator.language;
  if (language !== observedLanguage) {
    observedLanguage = language;
    retained = undefined;
  }
  return (retained ??= new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 }));
}`;
const intlReference = `https://github.com/millionco/react-doctor/blob/${SOURCE_COMMIT}/packages/oxlint-plugin-react-doctor/src/plugin/rules/js-performance/js-hoist-intl.ts`;

for (const [name, argumentsSource] of [
  ["NumberFormat", "undefined, { maximumFractionDigits: 1 }"],
  ["DateTimeFormat", "'en-US', { timeZone: 'UTC' }"],
  ["Collator", "'de-DE'"],
  ["RelativeTimeFormat", "'en-US'"],
  ["ListFormat", "'en-US'"],
  ["PluralRules", "'en-US'"],
  ["Segmenter", "'en-US'"],
  ["DisplayNames", "'en-US', { type: 'language' }"],
]) {
  semanticTest(`proves a module-owned lazy Intl cache: ${name}`, (t) => {
    const construction = `new Intl.${name}(${argumentsSource})`;
    const assignment = `retained ??= ${construction}`;
    const source = `let retained; export function formatter() { return (${assignment}); }`;
    const f = fixture(t, source);
    const diagnostic = f.add(INTL_RULE, construction, intlMessage(name));
    const proof = assertProof(f, diagnostic, "module-owned-lazy-intl-cache", [
      "let retained;",
      construction,
      assignment,
    ]);
    assert.ok(proof.provenance.references.includes(intlReference));
  });
}
semanticTest("proves same-owner observed-language cache invalidation", (t) => {
  const f = fixture(t, GUARDED_INTL_CACHE_SOURCE);
  const diagnostic = f.add(INTL_RULE, "new Intl.NumberFormat");
  assertProof(f, diagnostic, "module-owned-lazy-intl-cache", [
    "let observedLanguage;",
    "let retained;",
    "const language = typeof navigator === 'undefined' ? undefined : navigator.language;",
    "observedLanguage = language",
    "retained = undefined",
    "retained ??= new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 })",
  ]);
});
semanticTest("proves a renamed module slot owned by an arrow helper", (t) => {
  const f = fixture(t, "let receipt; const acquire = () => (receipt ??= new Intl.NumberFormat()); export const printer = acquire;");
  const diagnostic = f.add(INTL_RULE, "new Intl.NumberFormat");
  assertProof(f, diagnostic, "module-owned-lazy-intl-cache", [
    "let receipt;",
    "receipt ??= new Intl.NumberFormat()",
  ]);
});
for (const precision of [0, 1]) {
  semanticTest(`projects actual observation formatter cache at precision ${precision}`, (t) => {
    const file = "web/app/src/features/settings/pages/events/runtime/statistics/observation-statistics.ts";
    const source = fs.readFileSync(path.resolve(__dirname, "../../../..", file), "utf8");
    const construction = new RegExp(`new Intl\\.NumberFormat\\(undefined,\\s*\\{\\s*maximumFractionDigits: ${precision}\\s*\\}\\)`).exec(source)?.[0];
    assert.ok(construction);
    const f = fixture(t, source, file);
    const diagnostic = f.add(INTL_RULE, construction);
    const cacheName = precision ? "fractionalFormatter" : "integerFormatter";
    assertProof(f, diagnostic, "module-owned-lazy-intl-cache", [
      "let formatterLanguage: string | undefined;",
      `let ${cacheName}: Intl.NumberFormat | undefined;`,
      "formatterLanguage = language",
      `${cacheName} = undefined`,
      construction,
    ]);
  });
}

for (const [name, source] of [
  ["unconditional construction per call", "export function formatter() { return new Intl.NumberFormat(); }"],
  ["a module slot type alone does not cache construction", "let retained: Intl.NumberFormat; export function formatter() { return new Intl.NumberFormat(); }"],
  ["function-local lazy slot", "export function formatter() { let retained; return (retained ??= new Intl.NumberFormat()); }"],
  ["shadowed local slot", "let retained; export function formatter() { let retained; return (retained ??= new Intl.NumberFormat()); }"],
  ["unconditional cache reset", "let retained; export function formatter() { retained = undefined; return (retained ??= new Intl.NumberFormat()); }"],
  ["reset after every cache read", "let retained; export function formatter() { const value = (retained ??= new Intl.NumberFormat()); retained = undefined; return value; }"],
  ["reset guard never advances its marker", GUARDED_INTL_CACHE_SOURCE.replace("observedLanguage = language;", "")],
  ["reset marker receives another value", GUARDED_INTL_CACHE_SOURCE.replace("observedLanguage = language;", "observedLanguage = 'different';")],
  ["reset condition is always true", GUARDED_INTL_CACHE_SOURCE.replace("language !== observedLanguage", "true")],
  ["guard compares fresh input each call", GUARDED_INTL_CACHE_SOURCE.replace("typeof navigator === 'undefined' ? undefined : navigator.language", "Math.random()")],
  ["reset marker has another writer", `${GUARDED_INTL_CACHE_SOURCE}\nexport function clearSignal() { observedLanguage = undefined; }`],
  ["cache has an exported reset writer", `${INTL_CACHE_SOURCE}\nexport function clearCache() { retained = undefined; }`],
  ["cache write escapes to an unknown callback owner", `${INTL_CACHE_SOURCE}\nregister(() => { retained = undefined; });`],
  ["cache write escapes through a callback alias", `${INTL_CACHE_SOURCE}\nconst clear = () => { retained = undefined; }; register(clear);`],
  ["cache has a destructuring writer", `${INTL_CACHE_SOURCE}\nexport function reset(value) { ({ retained } = value); }`],
  ["cache is exposed as an object property", "const retained = {}; export function formatter() { return (retained.value ??= new Intl.NumberFormat()); }"],
  ["cache slot is exported", INTL_CACHE_SOURCE.replace("let retained;", "export let retained;")],
  ["different assignment operator", INTL_CACHE_SOURCE.replace("??=", "=")],
  ["slot has a competing initializer", `${INTL_CACHE_SOURCE}\nexport function other() { return (retained ??= makeFormatter()); }`],
  ["Intl parameter shadows the native constructor", INTL_CACHE_SOURCE.replace("formatter()", "formatter(Intl)")],
  ["module Intl import shadows the native namespace", `import Intl from 'unknown';\n${INTL_CACHE_SOURCE}`],
  ["native constructor namespace is overwritten", `Intl.NumberFormat = replacement;\n${INTL_CACHE_SOURCE}`],
  ["opaque lexical eval can reset the slot", `${INTL_CACHE_SOURCE}\nexport function reset() { eval('retained = undefined'); }`],
]) {
  semanticTest(`keeps actual or unproved Intl allocation unresolved: ${name}`, (t) => {
    const f = fixture(t, source);
    f.add(INTL_RULE, "new Intl.NumberFormat");
    assertUnproved(f);
  });
}
semanticTest("new Intl writes invalidate prior source evidence without a review exception", (t) => {
  const f = fixture(t, INTL_CACHE_SOURCE);
  const diagnostic = f.add(INTL_RULE, "new Intl.NumberFormat");
  assertProof(f, diagnostic, "module-owned-lazy-intl-cache", ["let retained;"]);
  f.write(f.file, `${INTL_CACHE_SOURCE}\nexport function clear() { retained = undefined; }`);
  assertUnproved(f);
});
semanticTest("Intl correction requires the exact pinned constructor, message and location", (t) => {
  const f = fixture(t, INTL_CACHE_SOURCE);
  const diagnostic = f.add(INTL_RULE, "new Intl.NumberFormat");
  assert.equal(f.evaluate("react-doctor@future").status, "requires-review");
  const original = structuredClone(diagnostic);
  for (const mutation of [
    { plugin: "unknown" },
    { message: intlMessage("DateTimeFormat") },
    { filePath: "../../outside.ts" },
    { line: diagnostic.line + 1 },
    { column: diagnostic.column + 1 },
  ]) {
    Object.assign(diagnostic, original, mutation);
    assertUnproved(f);
  }
  Object.assign(diagnostic, original);
  f.write(f.file, "export const invalid = (");
  assertUnproved(f);
});
semanticTest("real Intl construction stays lazy until the observed language changes", () => {
  const { runInNewContext } = require("node:vm");
  let constructions = 0;
  const host = {
    navigator: { language: "en-US" },
    Intl: {
      NumberFormat: class extends Intl.NumberFormat {
        constructor(...args) {
          constructions++;
          super(...args);
        }
      },
    },
  };
  runInNewContext(`${GUARDED_INTL_CACHE_SOURCE.replace("export ", "")}\nglobalThis.read = formatter;`, host);
  assert.equal(constructions, 0);
  const first = host.read();
  for (let call = 0; call < 8; call++) assert.equal(host.read(), first);
  assert.equal(constructions, 1);
  assert.equal(first.format(1234.5), new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 }).format(1234.5));
  host.navigator.language = "de-DE";
  const next = host.read();
  assert.notEqual(next, first);
  for (let call = 0; call < 8; call++) assert.equal(host.read(), next);
  assert.equal(constructions, 2);
});

semanticTest(
  "actual server render does not invoke a native click callback but does execute an IIFE",
  () => {
    const requireWeb = createRequire(
      path.resolve(__dirname, "../../../../web/app/package.json"),
    );
    const React = requireWeb("react");
    const { renderToStaticMarkup } = requireWeb("react-dom/server");
    let calls = 0;
    let deferred;
    function Deferred() {
      deferred = () => {
        calls++;
      };
      return React.createElement("button", { onClick: deferred }, "Add");
    }
    renderToStaticMarkup(React.createElement(Deferred));
    assert.equal(calls, 0);
    deferred();
    assert.equal(calls, 1);
    function Immediate() {
      return React.createElement(
        "button",
        {
          onClick: (() => {
            calls++;
            return () => {};
          })(),
        },
        "Add",
      );
    }
    renderToStaticMarkup(React.createElement(Immediate));
    assert.equal(calls, 2);
  },
);
semanticTest(
  "actual HTML rendering preserves composite graphics and rejects children on native img",
  () => {
    const requireWeb = createRequire(
      path.resolve(__dirname, "../../../../web/app/package.json"),
    );
    const React = requireWeb("react");
    const { renderToStaticMarkup } = requireWeb("react-dom/server");
    const graphic = React.createElement("svg", { "aria-hidden": true });
    const markup = renderToStaticMarkup(
      React.createElement(
        "span",
        { role: "img", "aria-label": "Result" },
        graphic,
      ),
    );
    assert.match(markup, /role="img"/u);
    assert.match(markup, /<svg/u);
    assert.throws(
      () =>
        renderToStaticMarkup(
          React.createElement("img", { alt: "Result" }, graphic),
        ),
      /void element|children/u,
    );
  },
);
semanticTest(
  "copy-sort preserves input and an iterable does not expose the proposed Array method",
  () => {
    const original = [3, 1, 2];
    const sorted = [...original].sort((a, b) => a - b);
    assert.deepEqual(original, [3, 1, 2]);
    assert.deepEqual(sorted, [1, 2, 3]);
    assert.notEqual(sorted, original);
    const values = new Set(original);
    assert.equal(typeof values.toSorted, "undefined");
  },
);
