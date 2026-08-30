#!/usr/bin/env node

const path = require('node:path');
const { inspectInterfaceLifecycleBoundary } = require('./core');

const repoRoot = path.resolve(__dirname, '../../..');
const violations = inspectInterfaceLifecycleBoundary(repoRoot);
if (violations.length > 0) {
  process.stderr.write(`${violations.join('\n')}\n`);
  process.exitCode = 1;
} else {
  process.stdout.write('interface lifecycle boundary: PASS\n');
}
