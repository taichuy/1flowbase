const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");
const assert = require("node:assert/strict");

const {
  parseCliArgs,
  buildVitestCommand,
  main,
} = require("../../cli/run-frontend-vitest.js");

test("parseCliArgs defaults to run mode", () => {
  assert.deepEqual(parseCliArgs([]), {
    mode: "run",
    passThroughArgs: [],
  });
});

test("parseCliArgs strips leading passthrough separator", () => {
  assert.deepEqual(parseCliArgs(["run", "--", "--help"]), {
    mode: "run",
    passThroughArgs: ["--help"],
  });
});

test("parseCliArgs strips package-script passthrough separator after default flags", () => {
  assert.deepEqual(
    parseCliArgs(["run", "--exclude", "a.test.ts", "--", "llm-node-defaults"]),
    {
      mode: "run",
      passThroughArgs: ["--exclude", "a.test.ts", "llm-node-defaults"],
    },
  );
});

test("buildVitestCommand uses the runtime-configured worker option", () => {
  assert.deepEqual(
    buildVitestCommand({
      mode: "run",
      runtimeConfig: {
        frontend: {
          vitestMaxWorkers: 2,
        },
      },
      passThroughArgs: ["src/example.test.ts"],
    }),
    {
      command: "pnpm",
      args: [
        "--dir",
        "web/app",
        "exec",
        "vitest",
        "run",
        "--maxWorkers=2",
        "src/example.test.ts",
      ],
      cwd: ".",
    },
  );
});

test("buildVitestCommand adds coverage flag in coverage mode", () => {
  assert.deepEqual(
    buildVitestCommand({
      mode: "coverage",
      runtimeConfig: {
        frontend: {
          vitestMaxWorkers: 1,
        },
      },
      passThroughArgs: [],
    }).args,
    [
      "--dir",
      "web/app",
      "exec",
      "vitest",
      "run",
      "--coverage",
      "--maxWorkers=1",
    ],
  );
});

test("main loads runtime config and spawns vitest wrapper command", () => {
  let captured = null;

  const status = main(["run", "src/example.test.ts"], {
    repoRoot: "/repo-root",
    env: {},
    runtimeConfig: {
      frontend: {
        vitestMaxWorkers: 3,
      },
    },
    spawnSyncImpl(command, args, options) {
      captured = { command, args, options };
      return { status: 0 };
    },
  });

  assert.equal(status, 0);
  assert.equal(captured.command, "pnpm");
  assert.deepEqual(captured.args, [
    "--dir",
    "web/app",
    "exec",
    "vitest",
    "run",
    "--maxWorkers=3",
    "src/example.test.ts",
  ]);
  assert.equal(captured.options.cwd, "/repo-root");
});

test("main prepends pnpm sibling node binary to PATH before spawning vitest", () => {
  const tempDir = fs.mkdtempSync(
    path.join(os.tmpdir(), "oneflowbase-run-frontend-vitest-"),
  );
  const binDir = path.join(tempDir, "bin");
  fs.mkdirSync(binDir, { recursive: true });
  fs.writeFileSync(path.join(binDir, "pnpm"), "", "utf8");
  fs.writeFileSync(path.join(binDir, "node"), "", "utf8");
  fs.chmodSync(path.join(binDir, "pnpm"), 0o755);
  fs.chmodSync(path.join(binDir, "node"), 0o755);

  let captured = null;
  const status = main(["run"], {
    repoRoot: "/repo-root",
    env: {
      PATH: binDir,
    },
    runtimeConfig: {
      frontend: {
        vitestMaxWorkers: 1,
      },
    },
    spawnSyncImpl(command, args, options) {
      captured = { command, args, options };
      return { status: 0 };
    },
  });

  assert.equal(status, 0);
  assert.equal(captured.options.env.PATH.split(path.delimiter)[0], binDir);
  assert.equal(captured.options.env.npm_execpath, path.join(binDir, "pnpm"));
  assert.equal(
    captured.options.env.npm_node_execpath,
    path.join(binDir, "node"),
  );
  assert.equal(captured.options.env.NODE, path.join(binDir, "node"));
});
