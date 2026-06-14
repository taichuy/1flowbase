const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  collectRepoHygieneFindings,
  main,
  partitionTrackedWarnings,
  scanSourceFile,
} = require('../core.js');

function writeFile(repoRoot, relativePath, content) {
  const absolutePath = path.join(repoRoot, relativePath);
  fs.mkdirSync(path.dirname(absolutePath), { recursive: true });
  fs.writeFileSync(absolutePath, content, 'utf8');
}

test('scanSourceFile reports debt markers and weak assertions without failing the gate', () => {
  const findings = scanSourceFile({
    relativePath: 'web/app/src/features/settings/_tests/settings-page.test.tsx',
    content: [
      "test('renders settings shell', () => {",
      '  // TODO: replace legacy assertion',
      '  expect(screen.getByText("Settings")).toBeTruthy();',
      '});',
    ].join('\n'),
  });

  assert.deepEqual(
    findings.map((finding) => finding.rule),
    ['source-debt-marker', 'weak-test-assertion']
  );
  assert.equal(findings.every((finding) => finding.severity === 'warning'), true);
});

test('scanSourceFile ignores debt words in protocol fields, module paths, and fixture strings', () => {
  const findings = scanSourceFile({
    relativePath: 'scripts/node/repo-hygiene/_tests/protocol-fixture.test.ts',
    content: [
      "import legacyFixture from './fixtures/legacy/compatibility-sample.json';",
      '',
      'const protocol = {',
      "  deprecated: 'protocol flag name, not source debt',",
      "  compatibility: 'v2',",
      "  compat: 'wire-format field',",
      '};',
      '',
      "const fixture = 'legacy compatibility deprecated compat text from imported payload';",
      '',
      "test('keeps legacy compatibility fixture text', () => {",
      '  expect(protocol).toEqual({',
      "    deprecated: 'protocol flag name, not source debt',",
      "    compatibility: 'v2',",
      "    compat: 'wire-format field',",
      '  });',
      '});',
      '',
      '// TODO: remove deprecated compatibility shim after source migration',
    ].join('\n'),
  });

  assert.deepEqual(
    findings.map((finding) => finding.rule),
    ['source-debt-marker']
  );
  assert.equal(findings[0].line, 19);
});

test('scanSourceFile reports low-value test smells as advisory findings', () => {
  const findings = scanSourceFile({
    relativePath: 'web/packages/api-client/src/_tests/console-frontstage.test.ts',
    content: [
      "test('frontstage transport spy is active', () => {",
      '  expect(apiFetchSpy).toHaveBeenCalledTimes(0);',
      '});',
      '',
      "test('createEmbedContext returns the provided context', () => {",
      "  expect(createEmbedContext({ applicationId: 'app-1' })).toEqual({ applicationId: 'app-1' });",
      '});',
      '',
      "test('keeps account before settings', () => {",
      '  expect(',
      '    accountLabel.compareDocumentPosition(settingsTrigger) &',
      '      Node.DOCUMENT_POSITION_FOLLOWING',
      '  ).toBeTruthy();',
      '});',
    ].join('\n'),
  });

  assert.deepEqual(
    findings.map((finding) => finding.rule),
    [
      'setup-only-test',
      'identity-wrapper-test',
      'weak-test-assertion',
    ]
  );
  assert.equal(findings.every((finding) => finding.severity === 'warning'), true);
});

test('scanSourceFile reports front-back field contract compatibility markers as warnings', () => {
  const findings = scanSourceFile({
    relativePath: 'web/app/src/features/example/api/example.ts',
    content: [
      'export function adaptExample(payload) {',
      '  // @field-contract-compat source=display_name alias=displayName remove_by=2026-06-30',
      '  return { displayName: payload.display_name };',
      '}',
    ].join('\n'),
  });

  assert.deepEqual(
    findings.map((finding) => finding.rule),
    ['field-contract-compat-marker']
  );
  assert.equal(findings[0].severity, 'warning');
});

test('scanSourceFile reports editable-looking values in JSX list keys', () => {
  const findings = scanSourceFile({
    relativePath: 'web/app/src/features/example/components/EditableRows.tsx',
    content: [
      'export function EditableRows({ rows }) {',
      '  return rows.map((row, index) => (',
      '    <div key={`${index}-${row.key}`}>',
      '      <Input value={row.key} onChange={() => undefined} />',
      '    </div>',
      '  ));',
      '}',
    ].join('\n'),
  });

  assert.deepEqual(
    findings.map((finding) => finding.rule),
    ['mutable-jsx-list-key']
  );
  assert.equal(findings[0].severity, 'error');
});

test('collectRepoHygieneFindings reports duplicate test titles and oversized files', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-repo-hygiene-'));
  writeFile(
    repoRoot,
    'web/app/src/features/example/_tests/example-a.test.ts',
    "test('duplicates title', () => {});\n"
  );
  writeFile(
    repoRoot,
    'web/app/src/features/example/_tests/example-b.test.ts',
    "test('duplicates title', () => {});\n"
  );
  writeFile(
    repoRoot,
    'api/crates/control-plane/src/large.rs',
    `${Array.from({ length: 1501 }, (_, index) => `pub const LINE_${index}: usize = ${index};`).join('\n')}\n`
  );
  writeFile(
    repoRoot,
    'web/app/src/features/example/_tests/large-page.test.tsx',
    `${Array.from({ length: 1501 }, (_, index) => `expect(row${index}).toBeInTheDocument();`).join('\n')}\n`
  );

  const findings = collectRepoHygieneFindings({ repoRoot });

  assert.equal(
    findings.some((finding) => finding.rule === 'duplicate-test-title'),
    true
  );
  assert.equal(
    findings.some((finding) => finding.rule === 'file-size-pressure'),
    true
  );
  assert.equal(
    findings.some((finding) => finding.rule === 'test-file-size-pressure'),
    true
  );
});

test('partitionTrackedWarnings suppresses only issue-tracked advisory findings', () => {
  const findings = [
    {
      severity: 'warning',
      rule: 'file-size-pressure',
      file: 'api/crates/control-plane/src/large.rs',
      line: null,
      message: 'file is approaching the repository split pressure line',
      snippet: '1200 lines',
    },
    {
      severity: 'error',
      rule: 'focused-test',
      file: 'web/app/src/features/example/_tests/example.test.ts',
      line: 1,
      message: 'focused test would make CI execute an incomplete test set',
      snippet: "test.only('example', () => {})",
    },
  ];

  const partitioned = partitionTrackedWarnings(findings, [
    {
      rule: 'file-size-pressure',
      file: 'api/crates/control-plane/src/large.rs',
      issue: '#901',
      reason: 'tracked as repo-hygiene debt',
    },
  ]);

  assert.deepEqual(
    partitioned.active.map((finding) => finding.rule),
    ['focused-test']
  );
  assert.equal(partitioned.suppressed.length, 1);
  assert.equal(partitioned.suppressed[0].issue, '#901');
  assert.equal(partitioned.suppressed[0].reason, 'tracked as repo-hygiene debt');
});

test('collectRepoHygieneFindings excludes tmp sandbox tests from formal quality assets', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-repo-hygiene-tmp-'));
  writeFile(
    repoRoot,
    'tmp/demo/app/src/app/_tests/demo.test.tsx',
    [
      "test.only('tmp sandbox test should not block repo hygiene', () => {",
      '  expect(screen.getByText("Demo")).toBeTruthy();',
      '});',
    ].join('\n')
  );
  writeFile(
    repoRoot,
    'web/app/src/features/example/_tests/example.test.ts',
    "test('formal test stays visible', () => {});\n"
  );

  const findings = collectRepoHygieneFindings({ repoRoot });

  assert.equal(
    findings.some((finding) => finding.file.startsWith('tmp/demo/')),
    false
  );
  assert.equal(
    findings.some((finding) => finding.file === 'web/app/src/features/example/_tests/example.test.ts'),
    false
  );
});

test('collectRepoHygieneFindings skips unreadable runtime artifact directories', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-repo-hygiene-permission-'));
  const unreadableDirectory = path.join(repoRoot, 'docker', 'volumes', 'postgres');
  writeFile(
    repoRoot,
    'web/app/src/features/example/_tests/example.test.ts',
    "test('formal test stays visible', () => {});\n"
  );
  fs.mkdirSync(unreadableDirectory, { recursive: true });

  let restored = false;
  try {
    fs.chmodSync(unreadableDirectory, 0);
    const findings = collectRepoHygieneFindings({ repoRoot });
    assert.equal(
      findings.some((finding) => finding.file.startsWith('docker/volumes/postgres')),
      false
    );
  } finally {
    fs.chmodSync(unreadableDirectory, 0o700);
    restored = true;
  }

  assert.equal(restored, true);
});

test('collectRepoHygieneFindings reports local env generated and scratch artifacts', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-repo-hygiene-artifacts-'));
  writeFile(
    repoRoot,
    'docker/middleware.env',
    'POSTGRES_DB=1flowbase\nPOSTGRES_USER=postgres\nPOSTGRES_PASSWORD=1flowbase\n'
  );
  writeFile(repoRoot, 'test_dir.txt', 'scratch\n');
  writeFile(repoRoot, 'web/app/tsconfig.tsbuildinfo', '{}\n');

  const findings = collectRepoHygieneFindings({ repoRoot });

  assert.equal(
    findings.some((finding) => finding.rule === 'tracked-env-artifact'
      && finding.file === 'docker/middleware.env'),
    true
  );
  assert.equal(
    findings.some((finding) => finding.rule === 'tracked-build-artifact'
      && finding.file === 'web/app/tsconfig.tsbuildinfo'),
    true
  );
  assert.equal(
    findings.some((finding) => finding.rule === 'root-scratch-artifact'
      && finding.file === 'test_dir.txt'),
    true
  );
});

test('main writes an advisory report and only fails on blocking findings', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-repo-hygiene-main-'));
  writeFile(
    repoRoot,
    'web/app/src/features/example/_tests/example.test.ts',
    [
      "test.only('focused test must block CI', () => {});",
      "test('legacy marker is advisory', () => {});",
    ].join('\n')
  );

  let stderr = '';
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
    fs.existsSync(path.join(repoRoot, 'tmp', 'test-governance', 'repo-hygiene.json')),
    true
  );
});
