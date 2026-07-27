import { describe, expect, test, vi } from 'vitest';

import {
  compileNativeReactComponent,
  evaluateNativeReactComponentArtifact
} from '../../index';

function compile(source: string) {
  const result = compileNativeReactComponent(source);
  if (!result.ok) throw new Error('Expected compiler fixture to succeed.');
  return result.artifact;
}

const modules = {
  react: { useState: vi.fn() },
  'react/jsx-runtime': {
    Fragment: Symbol('Fragment'),
    jsx: vi.fn(),
    jsxs: vi.fn()
  }
};

describe('Native React artifact evaluator', () => {
  test('D1-AC-002 canonicalizes the Worker artifact before main-thread evaluation', () => {
    const artifact = compile(`
import { useState } from 'react';
export default function Block() {
  const [count] = useState(0);
  return <div>{count}</div>;
}
`);
    const evaluated = evaluateNativeReactComponentArtifact(
      JSON.parse(JSON.stringify(artifact)),
      modules
    );

    expect(evaluated.ok).toBe(true);
    if (evaluated.ok) expect(typeof evaluated.component).toBe('function');
    expect(modules.react.useState).not.toHaveBeenCalled();
  });

  test('rejects a corrupt guard contract before executable code can run', () => {
    const artifact = compile(
      'export default function Block() { return null; }'
    );
    artifact.program.runtimeCapabilityGuardBindingIdentifiers = [];
    artifact.program.executableBody = 'globalThis.__artifactExecuted = true;';

    expect(
      evaluateNativeReactComponentArtifact(artifact, modules)
    ).toMatchObject({
      ok: false,
      diagnostics: [{ phase: 'runtime', path: 'artifact' }]
    });
    expect('__artifactExecuted' in globalThis).toBe(false);
  });
});
