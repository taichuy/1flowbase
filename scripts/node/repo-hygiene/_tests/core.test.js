const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const {
  OUTBOUND_HTTP_CLIENT_OWNER_ALLOWLIST,
  collectRepoHygieneFindings,
  collectSourceFiles,
  main,
  partitionTrackedWarnings,
  scanSourceFile,
} = require("../core.js");

test("scanSourceFile rejects outbound HTTP clients outside explicit Host owners", () => {
  const findings = scanSourceFile({
    relativePath: "api/apps/api-server/src/example_catalog.rs",
    content: "let client = reqwest::Client::builder().build()?;\n",
  });

  assert.deepEqual(
    findings.map((finding) => [finding.rule, finding.severity]),
    [["unowned-outbound-http-client", "error"]],
  );
});

test("outbound HTTP client owner exceptions are narrow and documented", () => {
  assert.equal(
    [...OUTBOUND_HTTP_CLIENT_OWNER_ALLOWLIST.values()].every(
      (reason) => reason.trim().length > 0,
    ),
    true,
  );
  const findings = scanSourceFile({
    relativePath: "api/apps/api-server/src/network_egress_client.rs",
    content: "let client = Client::builder().build()?;\n",
  });
  assert.equal(
    findings.some((finding) => finding.rule === "unowned-outbound-http-client"),
    false,
  );
});

function writeFile(repoRoot, relativePath, content) {
  const absolutePath = path.join(repoRoot, relativePath);
  fs.mkdirSync(path.dirname(absolutePath), { recursive: true });
  fs.writeFileSync(absolutePath, content, "utf8");
}

test("repository scan preserves the complete ordered multi-file finding contract", () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), "oneflowbase-hygiene-ordered-"));
  try {
    const sources = new Map([
      ["web/_tests/c.test.ts", "test('beta title', () => {});\n"],
      ["web/_tests/b.test.ts", "test('alpha title', () => {});\n"],
      ["web/_tests/a.test.ts", [
        "test('beta   title', () => {});",
        "test('alpha title', () => {});",
        "test('alpha title', () => {});",
        "test('local title', () => {});",
        "test('local title', () => {});",
        "test.only('focused', () => {});",
        "test.skip('pending', () => {});",
        "expect(value).toBeDefined();",
        "expect(value).not.toBeTruthy();",
      ].join("\n")],
      ["web/Rows.tsx", "const row = <div key={`${index}-${row.key}`} />;\n"],
      ["api/apps/api-server/src/client.rs", "let client = reqwest::Client::new();\n"],
      ["api/src/large.rs", "// stable source\n".repeat(1501)],
      ["api/src/debt.rs", "// FIXME: preserve failure owner\n"],
      ["docker/local.env", "VALUE=example\n"],
      ["test_dir.txt", "scratch\n"],
      ["web/app/tsconfig.tsbuildinfo", "{}\n"],
      ["web/clean.ts", "const text = 'test.only( TODO';\n"],
      ["docker/local.env.example", "VALUE=\n"],
    ]);
    for (let index = 0; index < 16; index += 1) {
      sources.set(`scripts/dense/file-${index}.js`, "module.exports = {};\n");
    }
    for (const [file, content] of sources) writeFile(repoRoot, file, content);

    const finding = (rule, file, message, snippet = "", line = null, severity = "warning") =>
      ({ severity, rule, file, line, message, snippet });
    const expected = [
      finding("tracked-env-artifact", "docker/local.env",
        "local env files should be untracked or converted to an example/template file"),
      finding("root-scratch-artifact", "test_dir.txt",
        "root-level scratch files should not be tracked as source"),
      finding("tracked-build-artifact", "web/app/tsconfig.tsbuildinfo",
        "generated build metadata should not be tracked as source"),
      finding("unowned-outbound-http-client", "api/apps/api-server/src/client.rs",
        "production modules must obtain HTTP clients from the Host-owned Network Egress boundary",
        "let client = reqwest::Client::new();", 1, "error"),
      finding("source-debt-marker", "api/src/debt.rs",
        "source declares actionable TODO/FIXME/HACK debt or an explicit @deprecated API; review its owner and removal path",
        "// FIXME: preserve failure owner", 1),
      finding("file-size-pressure", "api/src/large.rs",
        "file is at or over the repository split pressure line", "1501 lines", null, "error"),
      finding("focused-test", "web/_tests/a.test.ts",
        "focused test would make CI execute an incomplete test set",
        "test.only('focused', () => {});", 6, "error"),
      finding("skipped-test", "web/_tests/a.test.ts",
        "skipped or todo test needs an explicit owner and removal path",
        "test.skip('pending', () => {});", 7),
      finding("weak-test-assertion", "web/_tests/a.test.ts",
        "weak assertion should be replaced with a behavior-specific expectation",
        "expect(value).toBeDefined();", 8),
      finding("mutable-jsx-list-key", "web/Rows.tsx",
        "JSX list key uses editable-looking data plus position; use a stable row id to avoid remounting focused controls",
        "const row = <div key={`${index}-${row.key}`} />;", 1, "error"),
      finding("directory-pressure", "scripts/dense",
        "directory has more than 15 source files and should be reviewed for owner subfolders",
        "16 files", null, "error"),
      finding("duplicate-test-title", "web/_tests/a.test.ts, web/_tests/c.test.ts",
        "duplicate test title makes failure triage less precise", "beta title"),
      finding("duplicate-test-title", "web/_tests/a.test.ts, web/_tests/b.test.ts",
        "duplicate test title makes failure triage less precise", "alpha title"),
    ];
    // Null means unavailable Git inventory: artifact reporting must fail closed.
    assert.deepEqual(collectRepoHygieneFindings({ repoRoot, trackedFiles: null }), expected);
    assert.deepEqual(
      collectRepoHygieneFindings({ repoRoot, trackedFiles: new Set(sources.keys()) }),
      expected,
    );
    assert.deepEqual(
      collectRepoHygieneFindings({ repoRoot, trackedFiles: new Set() }),
      expected.slice(3),
    );
  } finally {
    fs.rmSync(repoRoot, { recursive: true, force: true });
  }
});

test("repository scan reads each selected file once and scans it before the next read", (t) => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), "oneflowbase-hygiene-single-pass-"));
  try {
    const sources = new Map([
      ["api/src/source.rs", "// TODO: keep this advisory visible\n"],
      ["docker/local.env", "VALUE=example\n"],
      ["web/_tests/a.test.ts", "test('shared title', () => {});\n"],
      ["web/_tests/b.test.ts", "it('shared title', () => {});\n"],
      ["web/empty.ts", ""],
    ]);
    for (const [file, content] of sources) writeFile(repoRoot, file, content);
    writeFile(repoRoot, "web/node_modules/ignored.js", "test.only('ignored', () => {});\n");
    writeFile(repoRoot, "web/ignored.bin", "binary\n");
    const selected = collectSourceFiles(repoRoot).map(({ relativePath }) => relativePath);
    assert.deepEqual(selected, [...sources.keys()]);
    const reads = [];
    const scanned = new Set();
    const readFileSync = fs.readFileSync;
    const split = String.prototype.split;
    // Observe the real scanner's line traversal without replacing scan behavior.
    t.mock.method(String.prototype, "split", function (...args) {
      if (args[0] instanceof RegExp && args[0].source === "\\r?\\n") {
        scanned.add(String(this));
      }
      return Reflect.apply(split, this, args);
    });
    t.mock.method(fs, "readFileSync", function (file, ...args) {
      const relativePath = path.relative(repoRoot, file).split(path.sep).join("/");
      if (reads.length > 0) {
        assert.ok(scanned.has(sources.get(reads.at(-1))), "previous file must be scanned before reading another body");
      }
      reads.push(relativePath);
      assert.equal(args[0], "utf8");
      return Reflect.apply(readFileSync, this, [file, ...args]);
    });

    const findings = collectRepoHygieneFindings({ repoRoot, trackedFiles: null });
    assert.deepEqual(reads, selected);
    assert.deepEqual(scanned, new Set(sources.values()));
    assert.deepEqual(findings.map(({ rule }) => rule), [
      "tracked-env-artifact", "source-debt-marker", "duplicate-test-title",
    ]);
  } finally {
    t.mock.restoreAll();
    fs.rmSync(repoRoot, { recursive: true, force: true });
  }
});

test("repository scan propagates file read failures without returning partial findings", (t) => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), "oneflowbase-hygiene-read-error-"));
  try {
    for (const name of ["a", "b", "c"]) {
      writeFile(repoRoot, `web/${name}.ts`, "// TODO: must not become a partial report\n");
    }
    const readFileSync = fs.readFileSync;
    for (const code of ["EACCES", "ENOENT", "EIO"]) {
      const failure = Object.assign(new Error(`fixture ${code}`), { code });
      const reads = [];
      const mock = t.mock.method(fs, "readFileSync", function (file, ...args) {
        reads.push(path.basename(file));
        if (path.basename(file) === "b.ts") throw failure;
        return Reflect.apply(readFileSync, this, [file, ...args]);
      });
      try {
        assert.throws(
          () => collectRepoHygieneFindings({ repoRoot, trackedFiles: null }),
          (error) => error === failure,
        );
        assert.deepEqual(reads, ["a.ts", "b.ts"]);
      } finally {
        mock.mock.restore();
      }
    }
  } finally {
    fs.rmSync(repoRoot, { recursive: true, force: true });
  }
});

test("scanSourceFile reports debt markers and weak assertions without failing the gate", () => {
  const findings = scanSourceFile({
    relativePath: "web/app/src/features/settings/_tests/settings-page.test.tsx",
    content: [
      "test('renders settings shell', () => {",
      "  // TODO: replace legacy assertion",
      '  expect(screen.getByText("Settings")).toBeTruthy();',
      "});",
    ].join("\n"),
  });

  assert.deepEqual(
    findings.map((finding) => finding.rule),
    ["source-debt-marker", "weak-test-assertion"],
  );
  assert.equal(
    findings.every((finding) => finding.severity === "warning"),
    true,
  );
});

test("scanSourceFile ignores debt words in protocol fields, module paths, and fixture strings", () => {
  const findings = scanSourceFile({
    relativePath: "scripts/node/repo-hygiene/_tests/protocol-fixture.test.ts",
    content: [
      "import legacyFixture from './fixtures/legacy/compatibility-sample.json';",
      "",
      "const protocol = {",
      "  deprecated: 'protocol flag name, not source debt',",
      "  compatibility: 'v2',",
      "  compat: 'wire-format field',",
      "};",
      "",
      "const fixture = 'legacy compatibility deprecated compat text from imported payload';",
      "",
      "test('keeps legacy compatibility fixture text', () => {",
      "  expect(protocol).toEqual({",
      "    deprecated: 'protocol flag name, not source debt',",
      "    compatibility: 'v2',",
      "    compat: 'wire-format field',",
      "  });",
      "});",
      "",
      "// TODO: remove deprecated compatibility shim after source migration",
    ].join("\n"),
  });

  assert.deepEqual(
    findings.map((finding) => finding.rule),
    ["source-debt-marker"],
  );
  assert.equal(findings[0].line, 19);
});

test("scanSourceFile distinguishes protocol history from actionable maintenance markers", () => {
  const findings = scanSourceFile({
    relativePath: "api/crates/domain/src/protocol.rs",
    content: [
      "// Rejects obsolete requests; legacy ACKs do not prove delivery.",
      "/// Compatibility preserves already stored user documents.",
      "// A deprecated protocol field is a wire-format fact, not a removal instruction.",
      "// FIXME: cleanup loses the completion owner on abort.",
      "// HACK: replace this fallback with the canonical registry.",
      "/// @deprecated use the canonical interface instead.",
    ].join("\n"),
  });

  assert.deepEqual(findings.map(({ rule, line }) => ({ rule, line })), [
    { rule: "source-debt-marker", line: 4 },
    { rule: "source-debt-marker", line: 5 },
    { rule: "source-debt-marker", line: 6 },
  ]);
});

test("scanSourceFile reports low-value test smells as advisory findings", () => {
  const findings = scanSourceFile({
    relativePath:
      "web/packages/api-client/src/_tests/console/modeling/frontstage.test.ts",
    content: [
      "test('frontstage transport spy is active', () => {",
      "  expect(apiFetchSpy).toHaveBeenCalledTimes(0);",
      "});",
      "",
      "test('createEmbedContext returns the provided context', () => {",
      "  expect(createEmbedContext({ applicationId: 'app-1' })).toEqual({ applicationId: 'app-1' });",
      "});",
      "",
      "test('keeps account before settings', () => {",
      "  expect(",
      "    accountLabel.compareDocumentPosition(settingsTrigger) &",
      "      Node.DOCUMENT_POSITION_FOLLOWING",
      "  ).toBeTruthy();",
      "});",
    ].join("\n"),
  });

  assert.deepEqual(
    findings.map((finding) => finding.rule),
    ["setup-only-test", "identity-wrapper-test", "weak-test-assertion"],
  );
  assert.equal(
    findings.every((finding) => finding.severity === "warning"),
    true,
  );
});

test("scanSourceFile reports front-back field contract compatibility markers as warnings", () => {
  const findings = scanSourceFile({
    today: "2026-06-01",
    relativePath: "web/app/src/features/example/api/example.ts",
    content: [
      "export function adaptExample(payload) {",
      "  // @field-contract-compat source=display_name alias=displayName remove_by=2026-06-30",
      "  return { displayName: payload.display_name };",
      "}",
    ].join("\n"),
  });

  assert.deepEqual(
    findings.map((finding) => finding.rule),
    ["field-contract-compat-marker"],
  );
  assert.equal(findings[0].severity, "warning");
});

test("compatibility warnings identify comments rather than marker literals", () => {
  const findings = scanSourceFile({
    today: "2026-10-19",
    relativePath: "scripts/node/example.js",
    content: [
      "const pattern =",
      String.raw`  /(?:\/\/|#|\/\*|\*)\s*@field-contract-compat\b/u;`,
      "const text = '// @field-contract-compat source=a alias=b';",
      "/*",
      " * @field-contract-compat source=a alias=b remove_by=2026-10-30",
      " */",
      "const value = source.a; // @field-contract-compat source=a alias=b remove_by=2026-10-30",
    ].join("\n"),
  });
  assert.deepEqual(findings.map(({ rule, line }) => ({ rule, line })), [
    { rule: "field-contract-compat-marker", line: 5 },
    { rule: "field-contract-compat-marker", line: 7 },
  ]);
});

test("scanSourceFile reports editable-looking values in JSX list keys", () => {
  const findings = scanSourceFile({
    relativePath: "web/app/src/features/example/components/EditableRows.tsx",
    content: [
      "export function EditableRows({ rows }) {",
      "  return rows.map((row, index) => (",
      "    <div key={`${index}-${row.key}`}>",
      "      <Input value={row.key} onChange={() => undefined} />",
      "    </div>",
      "  ));",
      "}",
    ].join("\n"),
  });

  assert.deepEqual(
    findings.map((finding) => finding.rule),
    ["mutable-jsx-list-key"],
  );
  assert.equal(findings[0].severity, "error");
});

test("collectRepoHygieneFindings reports duplicate test titles and oversized files", () => {
  const repoRoot = fs.mkdtempSync(
    path.join(os.tmpdir(), "oneflowbase-repo-hygiene-"),
  );
  writeFile(
    repoRoot,
    "web/app/src/features/example/_tests/example-a.test.ts",
    "test('duplicates title', () => {});\n",
  );
  writeFile(
    repoRoot,
    "web/app/src/features/example/_tests/example-b.test.ts",
    "test('duplicates title', () => {});\n",
  );
  writeFile(
    repoRoot,
    "api/crates/control-plane/src/large.rs",
    `${Array.from({ length: 1501 }, (_, index) => `pub const LINE_${index}: usize = ${index};`).join("\n")}\n`,
  );
  writeFile(
    repoRoot,
    "web/app/src/features/example/_tests/large-page.test.tsx",
    `${Array.from({ length: 1501 }, (_, index) => `expect(row${index}).toBeInTheDocument();`).join("\n")}\n`,
  );

  const findings = collectRepoHygieneFindings({ repoRoot });

  assert.equal(
    findings.some((finding) => finding.rule === "duplicate-test-title"),
    true,
  );
  assert.equal(
    findings.some((finding) => finding.rule === "file-size-pressure"),
    true,
  );
  assert.equal(
    findings.some((finding) => finding.rule === "test-file-size-pressure"),
    true,
  );
});

test("duplicate title review allows shared suite names in split scenario files", () => {
  const repoRoot = fs.mkdtempSync(
    path.join(os.tmpdir(), "oneflowbase-repo-hygiene-suite-titles-"),
  );
  try {
    for (const [file, scenario] of [["list", "lists current records"], ["editor", "edits the selected record"]]) {
      writeFile(
        repoRoot,
        `web/app/src/features/example/_tests/${file}.test.ts`,
        `describe('ResourcePage', () => { test('${scenario}', () => {}); });\n`,
      );
    }
    const findings = collectRepoHygieneFindings({ repoRoot });
    assert.deepEqual(
      findings.filter((finding) => finding.rule === "duplicate-test-title"),
      [],
    );
  } finally {
    fs.rmSync(repoRoot, { recursive: true, force: true });
  }
});

test("file size pressure excludes installed plugin copies", () => {
  const repoRoot = fs.mkdtempSync(
    path.join(os.tmpdir(), "oneflowbase-repo-hygiene-size-boundary-"),
  );
  const oversizedSource = `${Array.from(
    { length: 1501 },
    (_, index) => `export const LINE_${index} = ${index};`,
  ).join("\n")}\n`;

  writeFile(
    repoRoot,
    "api/plugins/capability-plugins/1flowbase/browser-assets/charts.js",
    oversizedSource,
  );
  writeFile(
    repoRoot,
    "api/plugins/installed/vendor/example/src/copied.js",
    oversizedSource,
  );
  writeFile(
    repoRoot,
    "api/plugins/capability-plugins/1flowbase/src/runtime.js",
    oversizedSource,
  );
  writeFile(
    repoRoot,
    "api/plugins/capability-plugins/vendor/browser-assets/custom.js",
    oversizedSource,
  );

  const pressureFiles = collectRepoHygieneFindings({ repoRoot })
    .filter((finding) => finding.rule === "file-size-pressure")
    .map((finding) => finding.file)
    .sort();

  assert.deepEqual(pressureFiles, [
    "api/plugins/capability-plugins/1flowbase/browser-assets/charts.js",
    "api/plugins/capability-plugins/1flowbase/src/runtime.js",
    "api/plugins/capability-plugins/vendor/browser-assets/custom.js",
  ]);
});

test("partitionTrackedWarnings suppresses only issue-tracked advisory findings", () => {
  const findings = [
    {
      severity: "warning",
      rule: "file-size-pressure",
      file: "api/crates/control-plane/src/large.rs",
      line: null,
      message: "file is approaching the repository split pressure line",
      snippet: "1200 lines",
    },
    {
      severity: "error",
      rule: "focused-test",
      file: "web/app/src/features/example/_tests/example.test.ts",
      line: 1,
      message: "focused test would make CI execute an incomplete test set",
      snippet: "test.only('example', () => {})",
    },
  ];

  const partitioned = partitionTrackedWarnings(findings, [
    {
      rule: "file-size-pressure",
      file: "api/crates/control-plane/src/large.rs",
      issue: "#901",
      reason: "tracked as repo-hygiene debt",
    },
  ]);

  assert.deepEqual(
    partitioned.active.map((finding) => finding.rule),
    ["focused-test"],
  );
  assert.equal(partitioned.suppressed.length, 1);
  assert.equal(partitioned.suppressed[0].issue, "#901");
  assert.equal(
    partitioned.suppressed[0].reason,
    "tracked as repo-hygiene debt",
  );
});

test("collectRepoHygieneFindings excludes tmp sandbox tests from formal quality assets", () => {
  const repoRoot = fs.mkdtempSync(
    path.join(os.tmpdir(), "oneflowbase-repo-hygiene-tmp-"),
  );
  writeFile(
    repoRoot,
    "tmp/demo/app/src/app/_tests/demo.test.tsx",
    [
      "test.only('tmp sandbox test should not block repo hygiene', () => {",
      '  expect(screen.getByText("Demo")).toBeTruthy();',
      "});",
    ].join("\n"),
  );
  writeFile(
    repoRoot,
    "web/app/src/features/example/_tests/example.test.ts",
    "test('formal test stays visible', () => {});\n",
  );

  const findings = collectRepoHygieneFindings({ repoRoot });

  assert.equal(
    findings.some((finding) => finding.file.startsWith("tmp/demo/")),
    false,
  );
  assert.equal(
    findings.some(
      (finding) =>
        finding.file === "web/app/src/features/example/_tests/example.test.ts",
    ),
    false,
  );
});

test("collectRepoHygieneFindings skips unreadable runtime artifact directories", () => {
  const repoRoot = fs.mkdtempSync(
    path.join(os.tmpdir(), "oneflowbase-repo-hygiene-permission-"),
  );
  const unreadableDirectory = path.join(
    repoRoot,
    "docker",
    "volumes",
    "postgres",
  );
  writeFile(
    repoRoot,
    "web/app/src/features/example/_tests/example.test.ts",
    "test('formal test stays visible', () => {});\n",
  );
  fs.mkdirSync(unreadableDirectory, { recursive: true });

  let restored = false;
  try {
    fs.chmodSync(unreadableDirectory, 0);
    const findings = collectRepoHygieneFindings({ repoRoot });
    assert.equal(
      findings.some((finding) =>
        finding.file.startsWith("docker/volumes/postgres"),
      ),
      false,
    );
  } finally {
    fs.chmodSync(unreadableDirectory, 0o700);
    restored = true;
  }

  assert.equal(restored, true);
});

test("collectRepoHygieneFindings reports tracked env generated and scratch artifacts", () => {
  const repoRoot = fs.mkdtempSync(
    path.join(os.tmpdir(), "oneflowbase-repo-hygiene-artifacts-"),
  );
  writeFile(
    repoRoot,
    "docker/middleware.env",
    "POSTGRES_DB=1flowbase\nPOSTGRES_USER=postgres\nPOSTGRES_PASSWORD=1flowbase\n",
  );
  writeFile(repoRoot, "test_dir.txt", "scratch\n");
  writeFile(repoRoot, "web/app/tsconfig.tsbuildinfo", "{}\n");

  const untrackedFindings = collectRepoHygieneFindings({
    repoRoot,
    trackedFiles: new Set(),
  });

  assert.equal(
    untrackedFindings.some(
      (finding) =>
        finding.rule === "tracked-env-artifact" &&
        finding.file === "docker/middleware.env",
    ),
    false,
  );
  assert.equal(
    untrackedFindings.some(
      (finding) =>
        finding.rule === "tracked-build-artifact" &&
        finding.file === "web/app/tsconfig.tsbuildinfo",
    ),
    false,
  );
  assert.equal(
    untrackedFindings.some(
      (finding) =>
        finding.rule === "root-scratch-artifact" &&
        finding.file === "test_dir.txt",
    ),
    false,
  );

  const findings = collectRepoHygieneFindings({
    repoRoot,
    trackedFiles: new Set([
      "docker/middleware.env",
      "test_dir.txt",
      "web/app/tsconfig.tsbuildinfo",
    ]),
  });

  assert.equal(
    findings.some(
      (finding) =>
        finding.rule === "tracked-env-artifact" &&
        finding.file === "docker/middleware.env",
    ),
    true,
  );
  assert.equal(
    findings.some(
      (finding) =>
        finding.rule === "tracked-build-artifact" &&
        finding.file === "web/app/tsconfig.tsbuildinfo",
    ),
    true,
  );
  assert.equal(
    findings.some(
      (finding) =>
        finding.rule === "root-scratch-artifact" &&
        finding.file === "test_dir.txt",
    ),
    true,
  );
});

test("main writes an advisory report and only fails on blocking findings", async () => {
  const repoRoot = fs.mkdtempSync(
    path.join(os.tmpdir(), "oneflowbase-repo-hygiene-main-"),
  );
  writeFile(
    repoRoot,
    "web/app/src/features/example/_tests/example.test.ts",
    [
      "test.only('focused test must block CI', () => {});",
      "test('legacy marker is advisory', () => {});",
    ].join("\n"),
  );

  let stderr = "";
  const status = await main([], {
    repoRoot,
    writeStdout() {},
    writeStderr(text) {
      stderr += text;
    },
  });

  assert.equal(status, 1);
  assert.match(stderr, /focused-test/u);
  assert.equal(
    fs.existsSync(
      path.join(repoRoot, "tmp", "test-governance", "repo-hygiene.json"),
    ),
    true,
  );
});


test("compatibility deadlines reject expired and malformed markers without hiding live contracts", () => {
  const findings = scanSourceFile({
    relativePath: "web/app/src/features/example/api/compat.ts",
    today: "2026-10-19",
    content: [
      "// @field-contract-compat source=a alias=b remove_by=2026-10-18",
      "// @field-contract-compat source=a alias=b remove_by=2026-10-19",
      "// @field-contract-compat source=a alias=b remove_by=2026-10-20",
      "// @field-contract-compat source=a alias=b remove_by=2026-02-30",
      "// @field-contract-compat source=a alias=b",
    ].join("\n"),
  });
  assert.deepEqual(findings.map(({ rule, severity }) => [rule, severity]), [
    ["expired-field-contract-compat-marker", "error"],
    ["field-contract-compat-marker", "warning"],
    ["field-contract-compat-marker", "warning"],
    ["invalid-field-contract-compat-marker", "error"],
    ["invalid-field-contract-compat-marker", "error"],
  ]);
});

test("structure hard limits preserve early warnings and exact vendor ownership", () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), "oneflowbase-structure-boundary-"));
  try {
    const source = (count) => "export const value = 1;\n".repeat(count);
    writeFile(repoRoot, "web/app/src/early.ts", source(1200));
    writeFile(repoRoot, "web/app/src/boundary.ts", source(1500));
    writeFile(repoRoot, "web/app/src/too_large.ts", source(1501));
    writeFile(repoRoot, "api/vendor/upstream/source.rs", "// upstream\n".repeat(1501));
    writeFile(repoRoot, "api/crates/vendor_named/src/source.rs", "// first party\n".repeat(1501));
    for (const [directory, count] of [["at_limit", 15], ["over_limit", 16]]) {
      for (let index = 0; index < count; index += 1) {
        writeFile(repoRoot, `web/app/src/${directory}/file_${index}.ts`, "export {};\n");
      }
    }
    const findings = collectRepoHygieneFindings({ repoRoot });
    const size = new Map(findings.filter(({ rule }) => rule === "file-size-pressure")
      .map(({ file, severity }) => [file, severity]));
    assert.equal(size.get("web/app/src/early.ts"), "warning");
    assert.equal(size.get("web/app/src/boundary.ts"), "warning");
    assert.equal(size.get("web/app/src/too_large.ts"), "error");
    assert.equal(size.get("api/vendor/upstream/source.rs"), "warning");
    assert.equal(size.get("api/crates/vendor_named/src/source.rs"), "error");
    assert.deepEqual(findings.filter(({ rule }) => rule === "directory-pressure")
      .map(({ file, severity }) => [file, severity]), [["web/app/src/over_limit", "error"]]);
  } finally {
    fs.rmSync(repoRoot, { recursive: true, force: true });
  }
});
