import { describe, expect, test } from 'vitest';

import {
  canonicalizeCompiledBlockArtifact,
  compileAndTransformJsBlockSource,
  createCompiledBlockArtifact,
  createCompiledBlockRuntimeFingerprint,
  sha256Text
} from '../index';

const source = `
async function main() {
  return { view: { primitive: 'Text', props: { children: 'Ready' } }, outputs: {} };
}
export default { main };
`;

function artifact() {
  const transformed = compileAndTransformJsBlockSource(source);
  if (!transformed.ok) throw new Error('fixture transform failed');
  return createCompiledBlockArtifact({
    source,
    runtimeFingerprint: createCompiledBlockRuntimeFingerprint('/worker-a.js'),
    allowedImports: [],
    transformed
  });
}

describe('CompiledBlockArtifact contract', () => {
  test('AC-021 D5-001 survives structured clone and JSON round trips', () => {
    const canonical = artifact();
    expect(canonicalizeCompiledBlockArtifact(structuredClone(canonical))).toEqual(
      canonical
    );
    expect(
      canonicalizeCompiledBlockArtifact(JSON.parse(JSON.stringify(canonical)))
    ).toEqual(canonical);
    expect(canonical.sourceSha256).toBe(sha256Text(source));
  });

  test('AC-021 D5-002 discards unknown canary fields at every artifact boundary', () => {
    const value = artifact() as unknown as Record<string, unknown>;
    value.context = { authorization: 'Bearer secret' };
    value.token = 'secret';
    value.blockResult = { outputs: { secret: true } };
    value.logs = ['secret'];
    (value.program as Record<string, unknown>).apiResponse = { secret: true };
    (value.manifest as Record<string, unknown>).userState = { secret: true };

    const canonical = canonicalizeCompiledBlockArtifact(value);
    expect(canonical).not.toBeNull();
    expect(JSON.stringify(canonical)).not.toMatch(
      /authorization|token|blockResult|logs|apiResponse|userState|secret/
    );
    expect(Object.keys(canonical!)).toEqual([
      'format',
      'version',
      'runtimeFingerprint',
      'sourceSha256',
      'program',
      'manifest',
      'sourceMap'
    ]);
  });

  test('D5-007 changes the runtime fingerprint when the Worker asset changes', () => {
    expect(createCompiledBlockRuntimeFingerprint('/worker-a.js')).not.toBe(
      createCompiledBlockRuntimeFingerprint('/worker-b.js')
    );
  });
});
