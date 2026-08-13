import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';
import { describe, expect, test } from 'vitest';

import {
  compileTailwindExecutableArtifact,
  TAILWIND_4_3_3_ARTIFACT_IDENTITY,
  TAILWIND_4_3_3_ARTIFACT_SHA256,
  TAILWIND_4_3_3_STYLESHEET_IDENTITY,
  type TailwindExecutableCompilerResult
} from '../executable-contract';
import { executableCompilerFixtures } from './fixtures/executable-contract-fixtures';

describe('versioned executable Tailwind compiler contract', () => {
  test('AC-compiler-artifact freezes canonical 4.3.3 artifact and stylesheet digests', () => {
    expect(TAILWIND_4_3_3_ARTIFACT_SHA256).toBe(
      createHash('sha256')
        .update(
          JSON.stringify(canonicalValue(TAILWIND_4_3_3_ARTIFACT_IDENTITY))
        )
        .digest('hex')
    );
    expect(TAILWIND_4_3_3_STYLESHEET_IDENTITY).toEqual(
      expect.objectContaining({
        version: '4.3.3',
        mode: 'theme-and-utilities',
        sha256:
          '14dcde35d39129464213fc7736ea90d719ecee5953c5cf836f6c89baa9a3fd10'
      })
    );
  });

  test.each(executableCompilerFixtures)(
    'AC-compiler-parity gives browser and Node runner the same result: $name',
    async ({ request, expectedDiagnostic, expectedCss, excludedCss = [] }) => {
      const browserResult = await compileTailwindExecutableArtifact(request);
      const runner = await runCompiler(request);

      expect(runner.result).toEqual(browserResult);
      expect(runner.exitCode).toBe(expectedDiagnostic ? 2 : 0);
      if (expectedDiagnostic) {
        expect(browserResult).toMatchObject({
          ok: false,
          validation_diagnostics: [{ code: expectedDiagnostic }]
        });
        return;
      }

      expect(browserResult.ok).toBe(true);
      if (!browserResult.ok) return;
      for (const value of expectedCss) {
        expect(browserResult.generated_css).toContain(value);
      }
      for (const value of excludedCss) {
        expect(browserResult.generated_css).not.toContain(value);
      }
      expect(browserResult.generated_css_sha256).toBe(
        createHash('sha256').update(browserResult.generated_css).digest('hex')
      );
      expect(browserResult.source_sha256).toBe(
        createHash('sha256').update(request.source_code).digest('hex')
      );
    }
  );

  test('AC-compiler-lock fails closed for an unknown exact lock', async () => {
    const fixture = executableCompilerFixtures[0];
    const result = await compileTailwindExecutableArtifact({
      ...fixture.request,
      toolchain_lock: { ...fixture.request.toolchain_lock, version: '4.3.4' }
    });

    expect(result).toMatchObject({
      ok: false,
      error: { code: 'unknown_toolchain_lock' }
    });
  });
});

async function runCompiler(request: unknown): Promise<{
  exitCode: number | null;
  result: TailwindExecutableCompilerResult;
}> {
  const child = spawn(
    process.execPath,
    [new URL('../../bin/compiler-4.3.3.mjs', import.meta.url).pathname],
    { stdio: ['pipe', 'pipe', 'pipe'] }
  );
  let stdout = '';
  let stderr = '';
  child.stdout.setEncoding('utf8');
  child.stderr.setEncoding('utf8');
  child.stdout.on('data', (chunk: string) => (stdout += chunk));
  child.stderr.on('data', (chunk: string) => (stderr += chunk));
  child.stdin.end(JSON.stringify(request));
  const exitCode = await new Promise<number | null>((resolve, reject) => {
    child.once('error', reject);
    child.once('close', resolve);
  });
  expect(stderr).toBe('');
  return {
    exitCode,
    result: JSON.parse(stdout) as TailwindExecutableCompilerResult
  };
}

function canonicalValue(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonicalValue);
  if (typeof value !== 'object' || value === null) return value;
  return Object.fromEntries(
    Object.entries(value)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, entry]) => [key, canonicalValue(entry)])
  );
}
