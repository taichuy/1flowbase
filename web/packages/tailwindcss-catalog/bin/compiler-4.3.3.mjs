#!/usr/bin/env node

import { compileTailwindExecutableArtifact } from '../src/executable-contract.ts';

let input = '';
process.stdin.setEncoding('utf8');
for await (const chunk of process.stdin) input += chunk;

let request;
try {
  request = JSON.parse(input);
} catch {
  writeFailure({
    ok: false,
    error: {
      code: 'invalid_request',
      message: 'Compiler stdin must contain exactly one JSON request.'
    },
    validation_diagnostics: [
      {
        phase: 'validation',
        code: 'invalid_request',
        path: 'request',
        message: 'Compiler stdin must contain exactly one JSON request.'
      }
    ]
  });
}

try {
  const result = await compileTailwindExecutableArtifact(request);
  process.stdout.write(`${JSON.stringify(result)}\n`);
  if (!result.ok) process.exitCode = 2;
} catch {
  writeFailure({
    ok: false,
    error: {
      code: 'compiler_failed',
      message: 'Executable Tailwind compiler failed.'
    },
    validation_diagnostics: []
  });
}

function writeFailure(result) {
  process.stdout.write(`${JSON.stringify(result)}\n`);
  process.exit(2);
}
