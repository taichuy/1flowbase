const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const { parseCliArgs, buildCommands, main } = require("../../test-frontend.js");

const passingReport = {
  success: true, numTotalTests: 2, numPassedTests: 1, numFailedTests: 0,
  numFailedTestSuites: 0, numPendingTests: 1,
};

function selectionReportPath(args) {
  const reportArg = args.find((arg) => arg.startsWith("--outputFile.json="));
  assert.ok(reportArg, "the current invocation requests a native JSON report");
  return reportArg.slice("--outputFile.json=".length);
}

function writeSelectionReport(args, report = passingReport) {
  const reportPath = selectionReportPath(args);
  assert.equal(fs.existsSync(reportPath), false, "a report must not exist before this invocation writes it");
  if (report !== null) {
    fs.writeFileSync(reportPath, typeof report === "string" ? report : JSON.stringify(report));
  }
  return reportPath;
}

test("parseCliArgs defaults to full frontend gate", () => {
  assert.deepEqual(parseCliArgs([]), {
    help: false,
    layer: "full",
    passThroughArgs: [],
    fileFilters: [],
  });
});

test("parseCliArgs accepts page-regression frontend gate", () => {
  assert.deepEqual(parseCliArgs(["page-regression"]), {
    help: false,
    layer: "page-regression",
    passThroughArgs: [],
    fileFilters: [],
  });
});

test("parseCliArgs accepts pr frontend gate", () => {
  assert.deepEqual(parseCliArgs(["pr"]), {
    help: false,
    layer: "pr",
    passThroughArgs: [],
    fileFilters: [],
  });
});

test("frontend coverage reuses the stable full-test exclusion inventory", () => {
  const packageJson = JSON.parse(
    fs.readFileSync(
      path.join(__dirname, "../../../../web/app/package.json"),
      "utf8",
    ),
  );
  const stableArgs = packageJson.scripts.test.split(" run ")[1];
  const coverageArgs =
    packageJson.scripts["test:coverage"].split(" coverage ")[1];

  assert.ok(stableArgs);
  assert.equal(coverageArgs, stableArgs);
});

test("buildCommands maps full layer to lint, test, build and all style boundaries", () => {
  const repoRoot = "/repo-root";
  const commands = buildCommands({ layer: "full", repoRoot, env: {} });
  assert.deepEqual(commands.map((command) => command.label), [
    "frontend-lint", "frontend-test", "frontend-build", "frontend-style-boundary",
  ]);
  assert.deepEqual(commands[1].args, ["--dir", "web", "test"]);
  assert.deepEqual(commands.at(-1).args, [
    path.join(repoRoot, "scripts", "node", "tooling.js"),
    "check-style-boundary", "all",
  ]);
});

test("pr layer runs lint, PR smoke tests, and build without style-boundary", () => {
  const repoRoot = "/repo-root";

  const commands = buildCommands({ layer: "pr", repoRoot });
  assert.deepEqual(commands.map((command) => command.label), [
    "frontend-lint", "frontend-pr-smoke-test", "frontend-build",
  ]);
  assert.deepEqual(commands[1].args, ["--dir", "web/app", "test:pr"]);
});

test("page-regression layer runs the long-term page regression suite", () => {
  const repoRoot = "/repo-root";

  assert.deepEqual(buildCommands({ layer: "page-regression", repoRoot }), [
    {
      label: "frontend-page-regression",
      command: "pnpm",
      args: ["--dir", "web/app", "test:page-regression"],
      cwd: ".",
    },
  ]);
});

test("fast layer only runs app vitest and writes advisory warnings", async (t) => {
  const repoRoot = fs.mkdtempSync(
    path.join(os.tmpdir(), "oneflowbase-test-frontend-"),
  );
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  const calls = [];

  const status = await main(["fast"], {
    repoRoot,
    env: {},
    writeStdout() {},
    writeStderr() {},
    spawnSyncImpl(command, args, options) {
      calls.push({ command, args, options });

      return {
        status: 0,
        stdout: "",
        stderr: "warning: vitest advisory\n",
      };
    },
  });

  assert.equal(status, 0);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].command, "pnpm");
  assert.deepEqual(calls[0].args, ["--dir", "web/app", "test"]);

  const fastLogPath = path.join(
    repoRoot,
    "tmp",
    "test-governance",
    "frontend-fast.log",
  );
  const warningLogPath = path.join(
    repoRoot,
    "tmp",
    "test-governance",
    "frontend-fast.warnings.log",
  );
  assert.equal(fs.existsSync(fastLogPath), true);
  assert.match(
    fs.readFileSync(fastLogPath, "utf8"),
    /warning: vitest advisory/u,
  );
  assert.equal(fs.existsSync(warningLogPath), true);
  assert.match(
    fs.readFileSync(warningLogPath, "utf8"),
    /warning: vitest advisory/u,
  );
});

test("main marks full frontend gate as heavy lock mode", async () => {
  let capturedLockMode = null;

  const status = await main(["full"], {
    repoRoot: "/repo-root",
    env: {},
    managedRunnerImpl(options) {
      capturedLockMode = options.lockMode;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.equal(capturedLockMode, "heavy");
});

test("main keeps fast frontend gate outside heavy lock mode", async (t) => {
  let capturedLockMode = null;
  const repoRoot = fs.mkdtempSync(
    path.join(os.tmpdir(), "oneflowbase-test-frontend-lock-"),
  );
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));

  const status = await main(["fast"], {
    repoRoot,
    env: {},
    managedRunnerImpl(options) {
      capturedLockMode = options.lockMode;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.equal(capturedLockMode, "none");
});

test("main names pr frontend gate as a lightweight merge scope", async () => {
  let capturedOptions = null;

  const status = await main(["pr"], {
    repoRoot: "/repo-root",
    env: {},
    managedRunnerImpl(options) {
      capturedOptions = options;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.equal(capturedOptions.scope, "frontend-pr");
  assert.equal(capturedOptions.lockMode, "none");
  assert.equal(
    capturedOptions.commandDisplay,
    "node scripts/node/test-frontend.js pr",
  );
});

test("main names page-regression as its own frontend scope", async () => {
  let capturedOptions = null;

  const status = await main(["page-regression"], {
    repoRoot: "/repo-root",
    env: {},
    managedRunnerImpl(options) {
      capturedOptions = options;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.equal(capturedOptions.scope, "frontend-page-regression");
  assert.equal(capturedOptions.lockMode, "none");
  assert.equal(
    capturedOptions.commandDisplay,
    "node scripts/node/test-frontend.js page-regression",
  );
});

test("explicit files bypass the fast exclusion list and reach budgeted Vitest with name filters", async (t) => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), "test-frontend-selected-"));
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  const file = "src/shared/i18n/_tests/format.test.ts";
  const excludedFile = "src/features/settings/pages/settings-page/model-providers/_tests/use-model-provider-data.test.tsx";
  const packageJson = JSON.parse(fs.readFileSync(path.join(__dirname, "../../../../web/app/package.json"), "utf8"));
  assert.ok(packageJson.scripts.test.includes(`--exclude ${excludedFile}`));
  const runtimeConfig = { frontend: { vitestMaxWorkers: 3 } };
  for (const selection of [[excludedFile], ["--", file, excludedFile, "-t", "decimal units"], [file, "--testNamePattern=decimal units"]]) {
    const calls = [];
    const status = await main(["fast", ...selection], {
      repoRoot, env: {}, runtimeConfig, writeStdout() {}, writeStderr() {},
      spawnSyncImpl(command, args, options) {
        calls.push({ command, args, options });
        const reportPath = writeSelectionReport(args);
        assert.ok(reportPath.startsWith(path.join(repoRoot, "tmp", "test-governance", "frontend-selections") + path.sep));
        return { status: 0, stdout: "", stderr: "" };
      },
    });
    assert.equal(status, 0);
    assert.equal(calls.length, 1);
    assert.equal(calls[0].command, "pnpm");
    const { args, options } = calls[0];
    assert.deepEqual(args.slice(0, 5), ["--dir", "web/app", "exec", "vitest", "run"]);
    assert.equal(args[5], "--maxWorkers=3");
    assert.deepEqual(args.slice(6, -3), selection.filter((arg) => arg !== "--"));
    assert.deepEqual(args.slice(-3, -1), ["--reporter=default", "--reporter=json"]);
    assert.equal(args.includes("--exclude"), false);
    assert.equal(options.env.ONEFLOWBASE_VITEST_MODE, "run");
  }
});

test("name-only selection retains the fast package suite and requires its own report", async (t) => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), "test-frontend-name-only-"));
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  for (const passed of [0, 1]) {
    const status = await main(["fast", "--", "-t", "decimal"], {
      repoRoot, env: {}, writeStdout() {}, writeStderr() {},
      spawnSyncImpl(command, args) {
        assert.equal(command, "pnpm");
        assert.deepEqual(args.slice(0, 5), ["--dir", "web/app", "test", "-t", "decimal"]);
        writeSelectionReport(args, { ...passingReport, numPassedTests: passed });
        return { status: 0, stdout: "", stderr: "" };
      },
    });
    assert.equal(status, passed === 0 ? 1 : 0);
  }
});

test("selected frontend runs reject invalid execution evidence and retain process and act failures", async (t) => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), "test-frontend-evidence-"));
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  const cases = [
    { name: "all skipped", report: { ...passingReport, numPassedTests: 0, numPendingTests: 2 } },
    { name: "empty", report: { ...passingReport, numTotalTests: 0, numPassedTests: 0, numPendingTests: 0 } },
    { name: "missing", report: null },
    { name: "malformed", report: "{unfinished" },
    { name: "failed tests", report: { ...passingReport, numFailedTests: 1 } },
    { name: "failed suite", report: { ...passingReport, numFailedTestSuites: 1 } },
    { name: "process failure", report: passingReport, processStatus: 23, expected: 23 },
    { name: "act failure", report: passingReport, stderr: "An update to Widget inside a test was not wrapped in act(...)\n" },
    { name: "passing with filtered tests", report: passingReport, stderr: "warning: vitest advisory\n", expected: 0 },
  ];
  for (const fixture of cases) {
    let stderr = "";
    const status = await main(["fast", "src/example.test.ts", "-t", "selected"], {
      repoRoot, env: {}, runtimeConfig: { frontend: { vitestMaxWorkers: 2 } },
      writeStdout() {}, writeStderr(text) { stderr += text; },
      spawnSyncImpl(_command, args) {
        writeSelectionReport(args, fixture.report);
        return { status: fixture.processStatus ?? 0, stdout: "", stderr: fixture.stderr ?? "" };
      },
    });
    assert.equal(status, fixture.expected ?? 1, fixture.name);
    if (fixture.name === "act failure") assert.match(stderr, /escaped the awaited act boundary/u);
    else if (fixture.expected === undefined) assert.match(stderr, /no valid executed passing tests/u);
  }
});

test("overlapping selections cannot accept another invocation's successful report", async (t) => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), "test-frontend-isolated-reports-"));
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  const pending = [];
  const invoke = () => main(["fast", "src/example.test.ts"], {
    repoRoot, env: {}, runtimeConfig: { frontend: { vitestMaxWorkers: 2 } },
    writeStdout() {}, writeStderr() {},
    managedRunnerImpl(options) {
      return new Promise((resolve) => pending.push({ options, resolve }));
    },
  });
  const invocations = [invoke(), invoke()];
  const paths = pending.map(({ options }) => selectionReportPath(options.commands[0].args));
  assert.equal(new Set(paths).size, 2);
  for (const [index, { options, resolve }] of pending.entries()) {
    resolve(options.runCommandSequenceImpl({
      ...options,
      spawnSyncImpl(_command, args) {
        if (index === 0) writeSelectionReport(args);
        else assert.equal(fs.existsSync(paths[0]), true, "the other run already has a successful report");
        return { status: 0, stdout: "", stderr: "" };
      },
    }));
  }
  assert.deepEqual(await Promise.all(invocations), [0, 1]);
});

test("frontend rejects unsupported arguments before any runner starts", async () => {
  const invalid = [
    ["full", "src/one.test.ts"],
    ["pr", "--", "src/one.test.ts"],
    ["page-regression", "-t", "one test"],
    ["fast", "-t"],
    ["fast", "--testNamePattern="],
    ["fast", "-t", "one", "-t", "two"],
    ["fast", "--maxWorkers=99"],
    ["fast", "--", "--maxWorkers", "99"],
    ["fast", "--passWithNoTests"],
    ["fast", "--watch"],
    ["fast", "--config", "other.config.ts"],
  ];
  for (const argv of invalid) {
    let started = false;
    await assert.rejects(() => main(argv, {
      managedRunnerImpl() { started = true; return 0; },
      spawnSyncImpl() { started = true; return { status: 0 }; },
    }), /Unsupported frontend|Missing value|Only one frontend/u);
    assert.equal(started, false, argv.join(" "));
  }
});

test("frontend help retains no-argument full usage and documents fast selectors", async () => {
  let output = "";
  let started = false;
  assert.equal(await main(["--help"], {
    writeStdout(text) { output += text; },
    managedRunnerImpl() { started = true; return 0; },
  }), 0);
  assert.equal(started, false);
  assert.match(output, /\[fast\|pr\|full\|page-regression\]/u);
  assert.match(output, /--testNamePattern/u);
});
