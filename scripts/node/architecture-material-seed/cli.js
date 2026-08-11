#!/usr/bin/env node

const path = require("node:path");

const {
  DEFAULT_API_BASE_URL,
  resolveAcceptanceZipPath,
  seedArchitectureMaterials,
} = require("./core.js");

function requireValue(args, option) {
  const value = args.shift();
  if (!value || value.startsWith("--")) throw new Error(`${option} 需要值`);
  return value;
}

function parseArgs(argv) {
  const repoRoot = path.resolve(__dirname, "..", "..", "..");
  const options = {
    help: false,
    dryRun: false,
    zipPath: null,
    apiBaseUrl: DEFAULT_API_BASE_URL,
    account: null,
    password: null,
    workspaceId: null,
  };
  const args = [...argv];
  while (args.length > 0) {
    const arg = args.shift();
    if (arg === "--help" || arg === "-h") options.help = true;
    else if (arg === "--dry-run") options.dryRun = true;
    else if (arg === "--zip") options.zipPath = requireValue(args, arg);
    else if (arg === "--api-base-url")
      options.apiBaseUrl = requireValue(args, arg);
    else if (arg === "--account") options.account = requireValue(args, arg);
    else if (arg === "--password") options.password = requireValue(args, arg);
    else if (arg === "--workspace-id")
      options.workspaceId = requireValue(args, arg);
    else throw new Error(`未知参数：${arg}`);
  }
  options.zipPath = path.resolve(
    options.zipPath ||
      resolveAcceptanceZipPath({ repoRoot, sourceEnv: process.env }),
  );
  return options;
}

function usage() {
  process.stdout
    .write(`用法：node scripts/node/architecture-material-seed/cli.js [options]

校验授权源而不写入：
  node scripts/node/architecture-material-seed/cli.js --dry-run --zip <xitonjiagoushi.zip>

幂等导入本地运行态：
  node scripts/node/architecture-material-seed/cli.js --zip <xitonjiagoushi.zip>

选项：
  --zip <path>             授权 zip；也可用 ARCHITECTURE_MATERIAL_ZIP
  --api-base-url <url>     默认 ${DEFAULT_API_BASE_URL}
  --account <account>      覆盖本地 root account
  --password <password>    覆盖本地 root password
  --workspace-id <uuid>    覆盖当前 session workspace
  --dry-run                仅校验 hash、成员和派生树，不访问 API
`);
}

async function main(argv = process.argv.slice(2)) {
  const options = parseArgs(argv);
  if (options.help) {
    usage();
    return 0;
  }
  const result = await seedArchitectureMaterials(options);
  process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
  return 0;
}

if (require.main === module) {
  main()
    .then((status) => {
      process.exitCode = status;
    })
    .catch((error) => {
      process.stderr.write(`[architecture-material-seed] ${error.message}\n`);
      process.exitCode = 1;
    });
}

module.exports = { main, parseArgs, usage };
