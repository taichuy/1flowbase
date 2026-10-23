const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const { parseCliArgs, buildCommands, main } = require("../../test-frontend.js");
const { main: runVitest } = require("../../cli/run-frontend-vitest.js");

test("parseCliArgs defaults to full frontend gate", () => {
  assert.deepEqual(parseCliArgs([]), {
    help: false,
    layer: "full",
    passThroughArgs: [],
  });
});

test("parseCliArgs accepts page-regression frontend gate", () => {
  assert.deepEqual(parseCliArgs(["page-regression"]), {
    help: false,
    layer: "page-regression",
    passThroughArgs: [],
  });
});

test("parseCliArgs accepts pr frontend gate", () => {
  assert.deepEqual(parseCliArgs(["pr"]), {
    help: false,
    layer: "pr",
    passThroughArgs: [],
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

test("fast file and name filters reach the Vitest runner while preserving its worker budget", async (t) => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), "test-frontend-selected-"));
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  const file = "src/shared/i18n/_tests/format.test.ts";
  const runtimeConfig = { frontend: { vitestMaxWorkers: 3 } };
  for (const selection of [[file, "-t", "decimal units"], ["--", file, "--testNamePattern=decimal units"]]) {
    const calls = [];
    const status = await main(["fast", ...selection], {
      repoRoot, env: {}, runtimeConfig, writeStdout() {}, writeStderr() {},
      spawnSyncImpl(command, args) {
        calls.push({ command, args });
        // Model pnpm's dispatch to the existing app test runner without a child process.
        const runnerStatus = runVitest(["run", ...args.slice(3)], {
          repoRoot, env: {}, runtimeConfig,
          spawnSyncImpl(vitestCommand, vitestArgs) {
            calls.push({ command: vitestCommand, args: vitestArgs });
            return { status: 0 };
          },
        });
        return { status: runnerStatus, stdout: "", stderr: "" };
      },
    });
    assert.equal(status, 0);
    assert.equal(calls.length, 2);
    assert.ok(calls.every(({ command }) => command === "pnpm"));
    assert.deepEqual(calls[0].args.slice(0, 3), ["--dir", "web/app", "test"]);
    assert.deepEqual(calls[1].args.slice(0, 5), ["--dir", "web/app", "exec", "vitest", "run"]);
    assert.ok(calls[1].args.includes(file));
    assert.ok(calls[1].args.includes("--maxWorkers=3"));
    assert.deepEqual(calls[1].args.slice(6), selection.filter((arg) => arg !== "--"));
  }
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
