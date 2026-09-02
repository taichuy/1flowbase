import { afterEach, describe, expect, test, vi } from 'vitest';

import {
  compileNativeReactComponent,
  evaluateNativeReactComponentArtifact
} from '../../index';

function compile(source: string) {
  const result = compileNativeReactComponent(source, [
    { module_source: 'react', exports: ['useState'] },
    {
      module_source: 'react/jsx-runtime',
      exports: ['Fragment', 'jsx', 'jsxs']
    }
  ]);
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

afterEach(() => vi.unstubAllGlobals());

describe('Native React artifact evaluator', () => {
  test('AC-003 evaluates runtime JavaScript from a serialized artifact', () => {
    const artifact = compile(`
const createValue = Function('value', 'return value + 1;');

export default function Block() {
  return createValue(1);
}
`);
    const evaluated = evaluateNativeReactComponentArtifact(
      JSON.parse(JSON.stringify(artifact)),
      modules
    );

    expect(evaluated.ok).toBe(true);
    if (!evaluated.ok) return;
    expect(evaluated.component({})).toBe(2);
  });

  test('AC-005 binds the browser fetch capability into native_react artifacts', () => {
    const browserFetch = vi.fn();
    vi.stubGlobal('fetch', browserFetch);
    const artifact = compile(`
export default function Block() {
  void fetch('https://api.example.test/value');
  return null;
}
`);
    const evaluated = evaluateNativeReactComponentArtifact(artifact, modules);

    expect(evaluated.ok).toBe(true);
    if (!evaluated.ok) return;
    evaluated.component();
    expect(browserFetch).toHaveBeenCalledWith('https://api.example.test/value');
  });

  test('I1923-AC-002 resolves a real ShadowRoot selection before the retargeted window range', () => {
    const zeroRect = {
      x: 0,
      y: 0,
      width: 0,
      height: 0,
      top: 0,
      right: 0,
      bottom: 0,
      left: 0,
      toJSON: () => ({})
    };
    const selectionRect = {
      x: 250,
      y: 30,
      width: 50,
      height: 20,
      top: 30,
      right: 300,
      bottom: 50,
      left: 250,
      toJSON: () => ({})
    };
    const windowSelection = {
      rangeCount: 1,
      getRangeAt: () => ({ getBoundingClientRect: () => zeroRect }),
      toString: () => 'Select'
    } as unknown as Selection;
    const shadowSelection = {
      rangeCount: 1,
      getRangeAt: () => ({ getBoundingClientRect: () => selectionRect }),
      toString: () => 'Select'
    } as unknown as Selection;
    const browserWindow = { getSelection: () => windowSelection };
    const browserDocument = {
      querySelectorAll: () => [
        { shadowRoot: { getSelection: () => shadowSelection } }
      ]
    };
    vi.stubGlobal('window', browserWindow);
    vi.stubGlobal('document', browserDocument);
    const artifact = compile(`
export default function Block() {
  return window.getSelection().getRangeAt(0).getBoundingClientRect().width;
}
`);

    const evaluated = evaluateNativeReactComponentArtifact(artifact, modules);

    expect(evaluated.ok).toBe(true);
    if (!evaluated.ok) return;
    expect(evaluated.component()).toBe(50);
  });

  test('R7-AC-001 binds a Host-owned console into the evaluated component closure', () => {
    const artifact = compile(`
export default function Block() {
  console.log('rendered', 1);
  return null;
}
`);
    const runtimeConsole = {
      debug: vi.fn(),
      error: vi.fn(),
      info: vi.fn(),
      log: vi.fn(),
      warn: vi.fn()
    };
    const evaluated = evaluateNativeReactComponentArtifact(artifact, modules, {
      console: runtimeConsole
    });

    expect(evaluated.ok).toBe(true);
    if (!evaluated.ok) return;
    evaluated.component();
    expect(runtimeConsole.log).toHaveBeenCalledWith('rendered', 1);
  });

  test('R7-AC-001 preserves a source-owned console binding', () => {
    const artifact = compile(`
const console = { log() { return undefined; } };
export default function Block() {
  console.log('source-owned');
  return null;
}
`);
    const runtimeLog = vi.fn();
    const evaluated = evaluateNativeReactComponentArtifact(artifact, modules, {
      console: {
        debug: vi.fn(),
        error: vi.fn(),
        info: vi.fn(),
        log: runtimeLog,
        warn: vi.fn()
      }
    });

    expect(evaluated.ok).toBe(true);
    if (!evaluated.ok) return;
    evaluated.component();
    expect(runtimeLog).not.toHaveBeenCalled();
  });

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
