const fs = require('node:fs');
const path = require('node:path');
const { execFileSync } = require('node:child_process');

const REPORT_FILE = 'hotspot-review.json';
const OUTPUT_ROOT = path.join('tmp', 'test-governance');
const DEFAULT_SINCE = '2 days ago';
const DEFAULT_MIN_TOUCHES = 3;
const DEFAULT_LINE_WARNING = 1200;
const DEFAULT_LINE_ERROR = 1500;

function getRepoRoot() {
  return path.resolve(__dirname, '..', '..', '..');
}

function parseIntegerOption(name, value) {
  const parsed = Number.parseInt(value, 10);

  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`${name} must be a positive integer`);
  }

  return parsed;
}

function readOptionValue(argv, index, name) {
  const value = argv[index + 1];

  if (!value || value.startsWith('--')) {
    throw new Error(`${name} requires a value`);
  }

  return value;
}

function parseHotspotCliArgs(argv = []) {
  const options = {
    help: false,
    since: DEFAULT_SINCE,
    minTouches: DEFAULT_MIN_TOUCHES,
    lineWarning: DEFAULT_LINE_WARNING,
    lineError: DEFAULT_LINE_ERROR,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];

    if (arg === '-h' || arg === '--help') {
      return { ...options, help: true };
    }

    if (arg === '--since') {
      options.since = readOptionValue(argv, index, '--since');
      index += 1;
      continue;
    }

    if (arg.startsWith('--since=')) {
      options.since = arg.slice('--since='.length);
      continue;
    }

    if (arg === '--min-touches') {
      options.minTouches = parseIntegerOption(
        '--min-touches',
        readOptionValue(argv, index, '--min-touches')
      );
      index += 1;
      continue;
    }

    if (arg.startsWith('--min-touches=')) {
      options.minTouches = parseIntegerOption(
        '--min-touches',
        arg.slice('--min-touches='.length)
      );
      continue;
    }

    if (arg === '--line-warning') {
      options.lineWarning = parseIntegerOption(
        '--line-warning',
        readOptionValue(argv, index, '--line-warning')
      );
      index += 1;
      continue;
    }

    if (arg === '--line-error') {
      options.lineError = parseIntegerOption(
        '--line-error',
        readOptionValue(argv, index, '--line-error')
      );
      index += 1;
      continue;
    }

    throw new Error(`Unknown hotspot-review option: ${arg}`);
  }

  return options;
}

function countFileLines(repoRoot, file) {
  const absolutePath = path.join(repoRoot, file);

  if (!fs.existsSync(absolutePath) || !fs.statSync(absolutePath).isFile()) {
    return null;
  }

  const content = fs.readFileSync(absolutePath, 'utf8');

  if (!content) {
    return 0;
  }

  return content.endsWith('\n')
    ? content.split('\n').length - 1
    : content.split('\n').length;
}

function includesAny(value, patterns) {
  return patterns.some((pattern) => pattern.test(value));
}

function classifyHotspot({ file, subjects = [], lines = null }) {
  const subjectText = subjects.join(' ').toLowerCase();
  const fileText = file.toLowerCase();
  const runtimeSignals = [
    /runtime/u,
    /run scope/u,
    /run-scope/u,
    /last[- ]run/u,
    /snapshot/u,
    /cache/u,
    /latest/u,
    /debug stream/u,
  ];
  const uiSignals = [
    /modal/u,
    /tab/u,
    /status/u,
    /header/u,
    /card/u,
    /compact/u,
    /move/u,
    /docs/u,
    /page/u,
    /dock/u,
    /overlay/u,
    /reorder/u,
    /history/u,
  ];
  const qualitySignals = [
    /quality gate/u,
    /lint/u,
    /clippy/u,
    /rustfmt/u,
    /coverage/u,
    /telemetry/u,
    /verify/u,
  ];

  if (
    fileText.includes('features/agent-flow/api/runtime')
    || fileText.includes('features/agent-flow/hooks/runtime')
    || fileText.includes('application_runtime')
    || includesAny(subjectText, runtimeSignals)
  ) {
    return {
      type: 'runtime-truth-churn',
      suggestedGate: 'backend state consistency gate',
      preventionTarget: 'backend-development state-and-consistency',
    };
  }

  if (
    fileText.startsWith('web/app/src/')
    && includesAny(subjectText, uiSignals)
  ) {
    return {
      type: 'frontend-ui-churn',
      suggestedGate: 'frontend interaction architecture gate',
      preventionTarget: 'frontend-development interaction-architecture-gate',
    };
  }

  if (
    fileText.startsWith('scripts/node/')
    || fileText.startsWith('.github/')
    || includesAny(subjectText, qualitySignals)
  ) {
    return {
      type: 'quality-gate-churn',
      suggestedGate: 'qa-evaluation quality gate routing',
      preventionTarget: 'qa-evaluation / scripts-node tooling',
    };
  }

  if (lines !== null && lines >= DEFAULT_LINE_WARNING) {
    return {
      type: 'file-size-pressure',
      suggestedGate: 'directory and owner split review',
      preventionTarget: 'AGENTS directory rules / size report',
    };
  }

  return {
    type: 'general-hotspot',
    suggestedGate: 'hotspot prevention review',
    preventionTarget: 'qa-evaluation hotspot-prevention',
  };
}

function severityForHotspot({ touches, minTouches, lines, lineWarning, lineError }) {
  if ((lines !== null && lines >= lineError) || touches >= minTouches * 3) {
    return 'error';
  }

  if (touches >= minTouches || (lines !== null && lines >= lineWarning)) {
    return 'warning';
  }

  return 'info';
}

function parseGitLogNameOnly(output) {
  const files = new Map();
  let currentCommit = null;

  for (const rawLine of output.split(/\r?\n/u)) {
    const line = rawLine.trim();

    if (!line) {
      continue;
    }

    if (line.startsWith('COMMIT\t')) {
      const [, sha, subject = ''] = line.split('\t');
      currentCommit = {
        sha,
        subject,
      };
      continue;
    }

    if (!currentCommit) {
      continue;
    }

    const current = files.get(line) || {
      file: line,
      commits: new Set(),
      subjects: new Set(),
    };

    current.commits.add(currentCommit.sha);
    current.subjects.add(currentCommit.subject);
    files.set(line, current);
  }

  return [...files.values()].map((entry) => ({
    file: entry.file,
    touches: entry.commits.size,
    commits: [...entry.commits],
    subjects: [...entry.subjects],
  }));
}

function collectHotspotReport({
  repoRoot = getRepoRoot(),
  since = DEFAULT_SINCE,
  minTouches = DEFAULT_MIN_TOUCHES,
  lineWarning = DEFAULT_LINE_WARNING,
  lineError = DEFAULT_LINE_ERROR,
  execFileSyncImpl = execFileSync,
} = {}) {
  const output = execFileSyncImpl(
    'git',
    [
      'log',
      `--since=${since}`,
      '--name-only',
      '--pretty=format:COMMIT%x09%H%x09%s',
    ],
    {
      cwd: repoRoot,
      encoding: 'utf8',
      maxBuffer: 64 * 1024 * 1024,
    }
  );
  const candidates = parseGitLogNameOnly(output);
  const hotspots = candidates
    .map((entry) => {
      const lines = countFileLines(repoRoot, entry.file);
      const classification = classifyHotspot({
        file: entry.file,
        subjects: entry.subjects,
        lines,
      });
      const severity = severityForHotspot({
        touches: entry.touches,
        minTouches,
        lines,
        lineWarning,
        lineError,
      });

      return {
        ...entry,
        lines,
        severity,
        ...classification,
      };
    })
    .filter((entry) => entry.touches >= minTouches || entry.severity !== 'info')
    .sort((left, right) => {
      if (right.touches !== left.touches) {
        return right.touches - left.touches;
      }

      return (right.lines || 0) - (left.lines || 0);
    });

  return {
    generatedAt: new Date().toISOString(),
    since,
    thresholds: {
      minTouches,
      lineWarning,
      lineError,
    },
    hotspots,
  };
}

function ensureOutputDir(repoRoot) {
  const outputDir = path.join(repoRoot, OUTPUT_ROOT);
  fs.mkdirSync(outputDir, { recursive: true });
  return outputDir;
}

function writeReport(repoRoot, report) {
  const outputDir = ensureOutputDir(repoRoot);
  const reportPath = path.join(outputDir, REPORT_FILE);
  fs.writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);
  return reportPath;
}

function usage(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(
    'Usage: node scripts/node/cli/hotspot-review.js [--since <git-date>] [--min-touches <n>] [--line-warning <n>] [--line-error <n>]\n'
  );
}

async function main(argv = [], deps = {}) {
  const options = parseHotspotCliArgs(argv);

  if (options.help) {
    usage(deps.writeStdout);
    return 0;
  }

  const repoRoot = deps.repoRoot || getRepoRoot();
  const report = collectHotspotReport({
    repoRoot,
    since: options.since,
    minTouches: options.minTouches,
    lineWarning: options.lineWarning,
    lineError: options.lineError,
    execFileSyncImpl: deps.execFileSyncImpl,
  });
  const reportPath = writeReport(repoRoot, report);
  const relativeReportPath = path.relative(repoRoot, reportPath).replace(/\\/gu, '/');
  const writeStdout = deps.writeStdout || ((text) => process.stdout.write(text));

  writeStdout(
    `[hotspot-review] ${report.hotspots.length} hotspots written to ${relativeReportPath}\n`
  );

  return 0;
}

module.exports = {
  classifyHotspot,
  collectHotspotReport,
  main,
  parseGitLogNameOnly,
  parseHotspotCliArgs,
  severityForHotspot,
};
