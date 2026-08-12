#!/usr/bin/env node

const path = require("node:path");

const {
  EXPLICIT_DUMP_ENV,
  EXPLICIT_RESTORE_ENV,
  resolvePostgresToolchain,
} = require("./resolver.js");

async function main() {
  const repoRoot = path.resolve(__dirname, "..", "..", "..");
  const result = await resolvePostgresToolchain({
    repoRoot,
    logImpl: (message) =>
      process.stderr.write(`[1flowbase-postgres-toolchain] ${message}\n`),
  });
  if (!result) return 1;
  process.stdout.write(`${EXPLICIT_DUMP_ENV}=${result.pgDumpPath}\n`);
  process.stdout.write(`${EXPLICIT_RESTORE_ENV}=${result.pgRestorePath}\n`);
  process.stdout.write(`source=${result.source}\n`);
  if (result.target) process.stdout.write(`target=${result.target}\n`);
  return 0;
}

main()
  .then((code) => {
    process.exitCode = code;
  })
  .catch((error) => {
    process.stderr.write(
      `[1flowbase-postgres-toolchain] ERROR: ${error.message}\n`,
    );
    process.exitCode = 1;
  });
