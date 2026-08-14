import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';
import { describe, expect, test } from 'vitest';

import {
  compileTailwindExecutableArtifact,
  TAILWIND_4_3_3_ARTIFACT_IDENTITY,
  TAILWIND_4_3_3_ARTIFACT_SHA256,
  TAILWIND_BLOCK_PRESET_ASSET,
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
        mode: 'block-preset',
        sha256:
          '41e1b1cefc721fa2889683134f896f1bafa9907d9057800343b2b7376f7d36a1'
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
      const tailwind = browserResult.dependency_lock.find(
        (entry) => entry.module_source === 'tailwindcss'
      );
      if (tailwind) {
        const { path: _path, ...presetAsset } = TAILWIND_BLOCK_PRESET_ASSET;
        expect(tailwind.assets).toContainEqual(
          expect.objectContaining(presetAsset)
        );
      }
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

  test.each([
    ['malformed', [{ module_source: 'tailwindcss' }]],
    [
      'duplicate',
      [
        executableCompilerFixtures[0].request.dependency_lock[0],
        executableCompilerFixtures[0].request.dependency_lock[0]
      ]
    ],
    [
      'noncanonical host binding',
      [
        {
          module_source: 'react',
          module_version: '19.2.5',
          binding: 'fetched',
          assets: [
            {
              role: 'browser_module',
              media_type: 'text/javascript',
              sha256: 'd'.repeat(64),
              url: '/fixture-assets/react'
            }
          ],
          exports: ['default']
        }
      ]
    ]
  ])(
    'AC-compiler-lock rejects %s dependency locks',
    async (_, dependencyLock) => {
      const result = await compileTailwindExecutableArtifact({
        ...executableCompilerFixtures[0].request,
        dependency_lock: dependencyLock
      });
      expect(result).toMatchObject({
        ok: false,
        error: { code: 'invalid_dependency_lock' }
      });
    }
  );
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
