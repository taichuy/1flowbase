const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  classifyHotspot,
  collectHotspotReport,
  main,
  parseHotspotCliArgs,
} = require('../core.js');

test('parseHotspotCliArgs defaults to two days and warning thresholds', () => {
  assert.deepEqual(parseHotspotCliArgs([]), {
    help: false,
    since: '2 days ago',
    minTouches: 3,
    lineWarning: 1200,
    lineError: 1500,
  });
});

test('classifyHotspot maps frontend UI churn to frontend interaction gate', () => {
  assert.deepEqual(
    classifyHotspot({
      file: 'web/app/src/features/applications/pages/ApplicationApiPage.tsx',
      subjects: ['Move API key list into modal', 'Compact application API status bar'],
      lines: 107,
    }),
    {
      type: 'frontend-ui-churn',
      suggestedGate: 'frontend interaction architecture gate',
      preventionTarget: 'frontend-development interaction-architecture-gate',
    }
  );
});

test('classifyHotspot maps runtime truth churn to state consistency gate', () => {
  assert.deepEqual(
    classifyHotspot({
      file: 'web/app/src/features/agent-flow/api/runtime.ts',
      subjects: ['Seed scoped node last-run cache', 'unify agent flow runtime panels by run scope'],
      lines: 630,
    }),
    {
      type: 'runtime-truth-churn',
      suggestedGate: 'backend state consistency gate',
      preventionTarget: 'backend-development state-and-consistency',
    }
  );
});

test('classifyHotspot does not treat every agent-flow UI edit as runtime truth churn', () => {
  assert.deepEqual(
    classifyHotspot({
      file: 'web/app/src/features/agent-flow/_tests/editor/agent-flow-editor-page.test.tsx',
      subjects: ['Use shared canvas dock for history versions', 'Reorder agent flow overlay actions'],
      lines: 812,
    }),
    {
      type: 'frontend-ui-churn',
      suggestedGate: 'frontend interaction architecture gate',
      preventionTarget: 'frontend-development interaction-architecture-gate',
    }
  );
});

test('collectHotspotReport aggregates git touches and file line pressure', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-hotspot-'));
  const filePath = path.join(repoRoot, 'web', 'app', 'src', 'features', 'agent-flow', 'api');
  fs.mkdirSync(filePath, { recursive: true });
  fs.writeFileSync(
    path.join(filePath, 'runtime.ts'),
    Array.from({ length: 1300 }, (_, index) => `line ${index + 1}`).join('\n')
  );

  const report = collectHotspotReport({
    repoRoot,
    since: '2 days ago',
    minTouches: 2,
    execFileSyncImpl(command, args) {
      assert.equal(command, 'git');
      assert.deepEqual(args.slice(0, 4), ['log', '--since=2 days ago', '--name-only', '--pretty=format:COMMIT%x09%H%x09%s']);

      return [
        'COMMIT\t1111111\tSeed scoped node last-run cache',
        'web/app/src/features/agent-flow/api/runtime.ts',
        'COMMIT\t2222222\tunify agent flow runtime panels by run scope',
        'web/app/src/features/agent-flow/api/runtime.ts',
        'COMMIT\t3333333\tTouch docs',
        'docs/example.md',
      ].join('\n');
    },
  });

  assert.equal(report.hotspots.length, 1);
  assert.equal(report.hotspots[0].file, 'web/app/src/features/agent-flow/api/runtime.ts');
  assert.equal(report.hotspots[0].touches, 2);
  assert.equal(report.hotspots[0].lines, 1300);
  assert.equal(report.hotspots[0].type, 'runtime-truth-churn');
  assert.equal(report.hotspots[0].severity, 'warning');
});

test('main writes hotspot review report to test governance output', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-hotspot-main-'));
  fs.mkdirSync(path.join(repoRoot, 'web', 'app', 'src', 'features', 'applications', 'pages'), { recursive: true });
  fs.writeFileSync(
    path.join(repoRoot, 'web', 'app', 'src', 'features', 'applications', 'pages', 'ApplicationApiPage.tsx'),
    'export function ApplicationApiPage() { return null; }\n'
  );

  const status = await main(['--min-touches', '1'], {
    repoRoot,
    execFileSyncImpl() {
      return [
        'COMMIT\t1111111\tMove API key list into modal',
        'web/app/src/features/applications/pages/ApplicationApiPage.tsx',
      ].join('\n');
    },
    writeStdout() {},
  });

  assert.equal(status, 0);

  const reportPath = path.join(repoRoot, 'tmp', 'test-governance', 'hotspot-review.json');
  assert.equal(fs.existsSync(reportPath), true);
  const report = JSON.parse(fs.readFileSync(reportPath, 'utf8'));

  assert.equal(report.hotspots[0].type, 'frontend-ui-churn');
  assert.equal(report.hotspots[0].suggestedGate, 'frontend interaction architecture gate');
});
