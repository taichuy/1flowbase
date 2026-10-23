#!/usr/bin/env node

const fs = require("node:fs");
const path = require("node:path");

const {
  getRepoRoot,
  runCommandSequence,
  runManagedCommandSequence,
} = require("../testing/warning-capture.js");
const { loadVerifyRuntimeConfig } = require("../testing/verify-runtime.js");
const { resolveNodeBinaryFromPath } = require("../testing/node-runtime.js");
const {
  buildBackendTestCommands: buildBackendCommands,
  buildRustBackendStaticGateCommand,
  runBackendCommandSequence,
} = require("../verify/backend.js");

const FRONTEND_LAYERS = new Set(["fast", "pr", "full", "page-regression"]);
const TEST_COMMANDS = new Set(["backend", "contracts", "frontend", "scripts"]);
const CONTRACT_TEST_FILES = [
  "src/features/settings/api/_tests/settings-api/settings-api.test.ts",
  "src/style-boundary/_tests/registry.test.tsx",
  "src/features/agent-flow/_tests/llm/llm-model-provider-field.test.tsx",
  "src/features/agent-flow/_tests/llm/llm-model-provider-field-panel.test.tsx",
  "src/features/agent-flow/_tests/llm/llm-model-provider-field-parameters.test.tsx",
];

function resolveScriptsNodeEntry(repoRoot, entryName) {
  return path.join(repoRoot, "scripts", "node", entryName);
}

function resolveScriptsNodeCliEntry(repoRoot, entryName) {
  return `${resolveScriptsNodeEntry(repoRoot, entryName)}.js`;
}

function usageBackend(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(
    "Usage: node scripts/node/test-backend.js [--package <crate> ...] [--lib | --test <target> | --bin <target>] [--filter <name>] [--exact]\n" +
      "No arguments: Rust static gate and full Cargo workspace tests.\n" +
      "Selection requires --package (or -p); --filter is a substring, --exact matches its full test name.\n" +
      "Cargo jobs and test threads always use the repository runtime configuration.\n",
  );
}

function readCliOptionValue(argv, index) {
  const value = argv[index + 1];
  if (!value?.trim() || value.startsWith("-")) {
    throw new Error(`Missing value for ${argv[index]}`);
  }
  return value;
}

function parseBackendTestCliArgs(argv) {
  if (argv.includes("-h") || argv.includes("--help")) {
    return { help: true, selection: null };
  }
  if (argv.length === 0) return { help: false, selection: null };

  const selection = { packages: [], targetArgs: [], filter: null, exact: false };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--package" || arg === "-p") {
      selection.packages.push(readCliOptionValue(argv, index));
      index += 1;
    } else if (["--lib", "--test", "--bin"].includes(arg)) {
      if (selection.targetArgs.length > 0) {
        throw new Error("Only one backend test target may be selected");
      }
      selection.targetArgs = arg === "--lib" ? [arg] : [arg, readCliOptionValue(argv, index)];
      if (arg !== "--lib") index += 1;
    } else if (arg === "--filter") {
      if (selection.filter !== null) {
        throw new Error("Only one backend test filter may be selected");
      }
      selection.filter = readCliOptionValue(argv, index);
      index += 1;
    } else if (arg === "--exact") {
      selection.exact = true;
    } else {
      throw new Error(`Unsupported backend test argument: ${arg}; see --help`);
    }
  }
  if (selection.packages.length === 0) {
    throw new Error("Backend test selection requires --package <crate>");
  }
  if (selection.exact && selection.filter === null) {
    throw new Error("--exact requires --filter <full-test-name>");
  }
  return { help: false, selection };
}

async function runBackend(argv = [], deps = {}) {
  const options = parseBackendTestCliArgs(argv);
  if (options.help) {
    usageBackend(deps.writeStdout);
    return 0;
  }

  const repoRoot = deps.repoRoot || getRepoRoot();
  const env = deps.env || process.env;
  const runtimeConfig =
    deps.runtimeConfig || loadVerifyRuntimeConfig({ repoRoot, env });
  const managedRunner = deps.managedRunnerImpl || runManagedCommandSequence;

  return managedRunner({
    repoRoot,
    env,
    runCommandSequenceImpl: runBackendCommandSequence,
    scope: "test-backend",
    lockMode: "heavy",
    commandDisplay: ["node scripts/node/test-backend.js", ...argv].join(" "),
    runtimeConfig,
    commands: buildBackendCommands({
      cargoJobs: runtimeConfig.backend.cargoJobs,
      cargoTestThreads: runtimeConfig.backend.cargoTestThreads,
      incremental: runtimeConfig.backend.incremental,
      repoRoot,
      env,
      selection: options.selection,
    }),
    spawnSyncImpl: deps.spawnSyncImpl,
    writeStdout: deps.writeStdout,
    writeStderr: deps.writeStderr,
  });
}

function buildContractsCommands({ repoRoot }) {
  return [
    {
      label: "model-provider-contract-tests",
      command: "pnpm",
      args: [
        "--dir",
        "web/app",
        "exec",
        "vitest",
        "run",
        ...CONTRACT_TEST_FILES,
      ],
      cwd: repoRoot,
    },
  ];
}

function usageContracts(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(
    "Usage: node scripts/node/cli/quality/test-contracts.js\n" +
      "Runs targeted model provider contract tests across shared consumers\n",
  );
}

async function runContracts(argv = [], deps = {}) {
  if (argv.includes("-h") || argv.includes("--help")) {
    usageContracts(deps.writeStdout);
    return 0;
  }

  const repoRoot = deps.repoRoot || getRepoRoot();
  const env = deps.env || process.env;
  const runtimeConfig =
    deps.runtimeConfig || loadVerifyRuntimeConfig({ repoRoot, env });
  const managedRunner = deps.managedRunnerImpl || runManagedCommandSequence;

  return managedRunner({
    repoRoot,
    env,
    scope: "test-contracts",
    lockMode: "heavy",
    commandDisplay: "node scripts/node/cli/quality/test-contracts.js",
    runtimeConfig,
    commands: buildContractsCommands({ repoRoot }),
    spawnSyncImpl: deps.spawnSyncImpl,
    writeStdout: deps.writeStdout,
    writeStderr: deps.writeStderr,
  });
}

function parseFrontendCliArgs(argv) {
  if (argv.includes("-h") || argv.includes("--help")) {
    return {
      help: true,
      layer: "full",
      passThroughArgs: [],
    };
  }

  const [layer = "full", ...args] = argv;

  if (!FRONTEND_LAYERS.has(layer)) {
    throw new Error(`Unknown frontend test layer: ${layer}`);
  }

  if (layer !== "fast" && args.length > 0) {
    throw new Error(`Unsupported frontend arguments for ${layer}; selectors are only supported by fast`);
  }
  const passThroughArgs = [];
  let hasNamePattern = false;
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--") continue;
    if (arg === "-t" || arg === "--testNamePattern" || arg.startsWith("--testNamePattern=")) {
      if (hasNamePattern) throw new Error("Only one frontend test name pattern may be selected");
      hasNamePattern = true;
      if (arg.startsWith("--testNamePattern=")) {
        if (!arg.slice("--testNamePattern=".length).trim()) {
          throw new Error("Missing value for --testNamePattern");
        }
        passThroughArgs.push(arg);
      } else {
        passThroughArgs.push(arg, readCliOptionValue(args, index));
        index += 1;
      }
    } else if (arg.startsWith("-") || !arg.trim()) {
      throw new Error(`Unsupported frontend fast argument: ${arg}; use file paths and -t/--testNamePattern`);
    } else {
      passThroughArgs.push(arg);
    }
  }

  return {
    help: false,
    layer,
    passThroughArgs,
  };
}

function buildFrontendCommands({ layer, repoRoot, env = process.env, passThroughArgs = [] }) {
  if (layer === "fast") {
    return [
      {
        label: "frontend-fast-test",
        command: "pnpm",
        args: ["--dir", "web/app", "test", ...passThroughArgs],
        cwd: ".",
      },
    ];
  }

  if (layer === "pr") {
    return [
      {
        label: "frontend-lint",
        command: "pnpm",
        args: ["--dir", "web", "lint"],
        cwd: ".",
      },
      {
        label: "frontend-pr-smoke-test",
        command: "pnpm",
        args: ["--dir", "web/app", "test:pr"],
        cwd: ".",
      },
      {
        label: "frontend-build",
        command: "pnpm",
        args: ["--dir", "web/app", "build"],
        cwd: ".",
      },
    ];
  }

  if (layer === "page-regression") {
    return [
      {
        label: "frontend-page-regression",
        command: "pnpm",
        args: ["--dir", "web/app", "test:page-regression"],
        cwd: ".",
      },
    ];
  }

  const nodeBinary = resolveNodeBinaryFromPath(env);

  return [
    {
      label: "frontend-lint",
      command: "pnpm",
      args: ["--dir", "web", "lint"],
      cwd: ".",
    },
    {
      label: "frontend-test",
      command: "pnpm",
      args: ["--dir", "web", "test"],
      cwd: ".",
    },
    {
      label: "frontend-build",
      command: "pnpm",
      args: ["--dir", "web/app", "build"],
      cwd: ".",
    },
    {
      label: "frontend-style-boundary",
      command: nodeBinary,
      args: [
        resolveScriptsNodeCliEntry(repoRoot, "tooling"),
        "check-style-boundary",
        "all",
      ],
      cwd: repoRoot,
    },
  ];
}

function usageFrontend(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(
    "Usage: node scripts/node/test-frontend.js [fast|pr|full|page-regression]\n" +
      "Defaults to full. Only fast accepts: [--] [file ...] [-t <pattern> | --testNamePattern <pattern>].\n" +
      "Example: node scripts/node/test-frontend.js fast -- src/shared/i18n/_tests/format.test.ts -t decimal\n",
  );
}

async function runFrontend(argv = [], deps = {}) {
  const options = parseFrontendCliArgs(argv);

  if (options.help) {
    usageFrontend(deps.writeStdout);
    return 0;
  }

  const repoRoot = deps.repoRoot || getRepoRoot();
  const env = deps.env || process.env;
  const runtimeConfig =
    deps.runtimeConfig || loadVerifyRuntimeConfig({ repoRoot, env });
  const managedRunner = deps.managedRunnerImpl || runManagedCommandSequence;

  return managedRunner({
    repoRoot,
    env,
    scope: `frontend-${options.layer}`,
    lockMode: options.layer === "full" ? "heavy" : "none",
    commandDisplay:
      ["node scripts/node/test-frontend.js", options.layer, ...options.passThroughArgs].join(" "),
    runtimeConfig,
    commands: buildFrontendCommands({
      layer: options.layer,
      repoRoot,
      env,
      passThroughArgs: options.passThroughArgs,
    }),
    spawnSyncImpl: deps.spawnSyncImpl,
    writeStdout: deps.writeStdout,
    writeStderr: deps.writeStderr,
  });
}

function parseScriptCliArgs(argv) {
  if (argv.includes("-h") || argv.includes("--help")) {
    return {
      help: true,
      filters: [],
    };
  }

  return {
    help: false,
    filters: argv,
  };
}

function walkScriptTests(currentDir, collected) {
  const entries = fs.readdirSync(currentDir, { withFileTypes: true });

  for (const entry of entries) {
    const absolutePath = path.join(currentDir, entry.name);

    if (entry.isDirectory()) {
      walkScriptTests(absolutePath, collected);
      continue;
    }

    if (
      entry.isFile() &&
      entry.name.endsWith(".test.js") &&
      absolutePath.includes(`${path.sep}_tests${path.sep}`)
    ) {
      collected.push(absolutePath);
    }
  }
}

function listTestFiles(repoRoot) {
  const collected = [];
  walkScriptTests(resolveScriptsNodeEntry(repoRoot, ""), collected);
  return collected;
}

function selectTestFiles(files, filters) {
  const sorted = [...files].sort((left, right) => left.localeCompare(right));

  if (filters.length === 0) {
    return sorted;
  }

  return sorted.filter((file) =>
    filters.some((filter) => file.includes(filter)),
  );
}

function buildScriptTestCommand({ repoRoot, files, env = process.env }) {
  return {
    label: "scripts-node-tests",
    command: resolveNodeBinaryFromPath(env),
    args: ["--test", ...files],
    cwd: repoRoot,
  };
}

function usageScripts(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(
    "Usage: node scripts/node/test-scripts.js [filter ...]\n" +
      "Examples:\n" +
      "  node scripts/node/test-scripts.js\n" +
      "  node scripts/node/test-scripts.js page-debug\n" +
      "  node scripts/node/test-scripts.js verify-backend runtime-gate\n",
  );
}

function runScripts(argv = [], deps = {}) {
  const options = parseScriptCliArgs(argv);

  if (options.help) {
    usageScripts(deps.writeStdout);
    return 0;
  }

  const repoRoot = deps.repoRoot || getRepoRoot();
  const discoveredFiles = (
    deps.listTestFilesImpl || (() => listTestFiles(repoRoot))
  )();
  const selectedFiles = selectTestFiles(discoveredFiles, options.filters);

  if (selectedFiles.length === 0) {
    throw new Error(
      `No script tests matched filters: ${options.filters.join(", ")}`,
    );
  }

  return runCommandSequence({
    repoRoot,
    env: deps.env || process.env,
    scope: "test-scripts",
    commands: [
      buildScriptTestCommand({
        repoRoot,
        files: selectedFiles,
        env: deps.env || process.env,
      }),
    ],
    spawnSyncImpl: deps.spawnSyncImpl,
    writeStdout: deps.writeStdout,
    writeStderr: deps.writeStderr,
  });
}

function parseTestCliArgs(argv) {
  if (argv.includes("-h") || argv.includes("--help") || argv.length === 0) {
    return {
      help: true,
      command: null,
      rest: [],
    };
  }

  const [command, ...rest] = argv;
  return {
    help: false,
    command,
    rest,
  };
}

function usage(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(
    "Usage: node scripts/node/test <backend|contracts|frontend|scripts> [args]\n",
  );
}

async function main(argv = [], deps = {}) {
  const options = parseTestCliArgs(argv);

  if (options.help) {
    usage(deps.writeStdout);
    return 0;
  }

  if (!TEST_COMMANDS.has(options.command)) {
    throw new Error(`Unknown test command: ${options.command}`);
  }

  if (options.command === "backend") {
    return (deps.runBackendImpl || runBackend)(options.rest, deps);
  }

  if (options.command === "contracts") {
    return (deps.runContractsImpl || runContracts)(options.rest, deps);
  }

  if (options.command === "frontend") {
    return (deps.runFrontendImpl || runFrontend)(options.rest, deps);
  }

  return (deps.runScriptsImpl || runScripts)(options.rest, deps);
}

module.exports = {
  CONTRACT_TEST_FILES,
  buildBackendCommands,
  buildRustBackendStaticGateCommand,
  buildContractsCommands,
  buildFrontendCommands,
  buildScriptTestCommand,
  listTestFiles,
  main,
  parseFrontendCliArgs,
  parseScriptCliArgs,
  parseTestCliArgs,
  runBackend,
  runContracts,
  runFrontend,
  runScripts,
  selectTestFiles,
};
