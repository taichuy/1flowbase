import { describe, expect, test } from 'vitest';

import {
  evaluateNativeTrustedBlockSource,
  type NativeTrustedBlockInjectedModuleMap
} from '../../index';

function Button(): null {
  return null;
}

function Surface(): null {
  return null;
}

function createModules(
  overrides: NativeTrustedBlockInjectedModuleMap = {}
): NativeTrustedBlockInjectedModuleMap {
  const React = {
    createElement(type: unknown, props: unknown, ...children: unknown[]) {
      return { type, props, children };
    }
  };

  return {
    react: {
      default: React,
      createElement: React.createElement
    },
    antd: {
      Button
    },
    '@1flowbase/ui': {
      Surface
    },
    ...overrides
  };
}

describe('Native trusted block source evaluator', () => {
  test('AC-001/AC-002 evaluates runtime JavaScript with explicit imported dependencies', () => {
    const result = evaluateNativeTrustedBlockSource({
      source: `
import { Button } from 'antd';

const createValue = new Function('Component', 'return { Component, value: 2 + 2 };');

export default function Block() {
  return createValue(Button);
}
`,
      modules: createModules()
    });

    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.component({})).toEqual({ Component: Button, value: 4 });
  });

  test('evaluates non-JSX source through injected React, AntD, and UI modules', () => {
    const result = evaluateNativeTrustedBlockSource({
      source: `
import React from 'react';
import { Button } from 'antd';
import { Surface } from '@1flowbase/ui';

export default function Block(props) {
  return React.createElement(Surface, null, React.createElement(Button, null, props.props.title));
}
`,
      modules: createModules()
    });

    expect(result.ok).toBe(true);
    if (!result.ok) {
      return;
    }

    expect(typeof result.component).toBe('function');
    expect(result.compiledSource.injectedModules).toEqual([
      {
        source: 'react',
        bindings: [{ kind: 'default', source: 'react', local: 'React' }]
      },
      {
        source: 'antd',
        bindings: [
          { kind: 'named', source: 'antd', imported: 'Button', local: 'Button' }
        ]
      },
      {
        source: '@1flowbase/ui',
        bindings: [
          {
            kind: 'named',
            source: '@1flowbase/ui',
            imported: 'Surface',
            local: 'Surface'
          }
        ]
      }
    ]);

    const rendered = result.component({ props: { title: 'Ready' } });

    expect(rendered).toEqual({
      type: Surface,
      props: null,
      children: [
        {
          type: Button,
          props: null,
          children: ['Ready']
        }
      ]
    });
  });

  test('evaluates a default-exported local component binding', () => {
    const result = evaluateNativeTrustedBlockSource({
      source: `
import React from 'react';
import { Button } from 'antd';

const Block = (props) => React.createElement(Button, null, props.props.title);
export default Block;
`,
      modules: createModules()
    });

    expect(result.ok).toBe(true);
    if (!result.ok) {
      return;
    }

    expect(result.component({ props: { title: 'Ready' } })).toEqual({
      type: Button,
      props: null,
      children: ['Ready']
    });
  });

  test('returns runtime_error for a missing injected module', () => {
    const result = evaluateNativeTrustedBlockSource({
      source: `
import { Button } from 'antd';

export default function Block() {
  return Button;
}
`,
      modules: createModules({ antd: undefined })
    });

    expect(result).toMatchObject({
      ok: false,
      error: {
        kind: 'runtime_error',
        errors: [{ code: 'runtime_error', path: 'modules.antd' }]
      }
    });
  });

  test('returns runtime_error for a missing imported binding', () => {
    const result = evaluateNativeTrustedBlockSource({
      source: `
import { Button } from 'antd';

export default function Block() {
  return Button;
}
`,
      modules: createModules({ antd: {} })
    });

    expect(result).toMatchObject({
      ok: false,
      error: {
        kind: 'runtime_error',
        errors: [{ code: 'runtime_error', path: 'modules.antd.Button' }]
      }
    });
  });

  test('returns source_policy_failed before module access for denied source', () => {
    const modules = {};
    Object.defineProperty(modules, 'react', {
      get() {
        throw new Error('module map should not be read');
      }
    });

    const result = evaluateNativeTrustedBlockSource({
      source: `
import React from 'react';

eval('2 + 2');

export default function Block() {
  return React.createElement('div');
}
`,
      modules
    });

    expect(result).toMatchObject({
      ok: false,
      error: {
        kind: 'source_policy_failed',
        errors: [{ code: 'transform_failed', path: 'source.identifiers.eval' }]
      }
    });
  });

  test('returns source_policy_failed before runtime guard access for denied source', () => {
    const result = evaluateNativeTrustedBlockSource({
      source: `
import React from 'react';

eval('2 + 2');
const guarded = w\\u0069ndow.location;

export default function Block() {
  return React.createElement('div');
}
`,
      modules: createModules()
    });

    expect(result).toMatchObject({
      ok: false,
      error: {
        kind: 'source_policy_failed',
        errors: [{ code: 'transform_failed', path: 'source.identifiers.eval' }]
      }
    });
    expect(result.ok ? '' : result.error.errors[0]?.path).not.toBe(
      'runtime.capability.window'
    );
  });

  test.each([
    ['fetch calls', "const denied = f\\u0065tch('/api');", 'fetch'],
    [
      'XMLHttpRequest construction',
      'const denied = new X\\u004dLHttpRequest();',
      'XMLHttpRequest'
    ],
    [
      'WebSocket construction',
      "const denied = new W\\u0065bSocket('ws://example.test');",
      'WebSocket'
    ],
    [
      'navigator.sendBeacon calls',
      "const denied = n\\u0061vigator['send' + 'Beacon']('/metrics');",
      'navigator.sendBeacon'
    ],
    [
      'localStorage reads',
      "const denied = l\\u006fcalStorage.getItem('key');",
      'localStorage'
    ],
    [
      'sessionStorage reads',
      "const denied = s\\u0065ssionStorage.getItem('key');",
      'sessionStorage'
    ],
    [
      'document.cookie reads',
      "const denied = d\\u006fcument['coo' + 'kie'];",
      'document.cookie'
    ],
    ['window reads', 'const denied = w\\u0069ndow.location;', 'window'],
    ['document reads', 'const denied = d\\u006fcument.body;', 'document'],
    [
      'globalThis reads',
      'const denied = g\\u006cobalThis.location;',
      'globalThis'
    ],
    ['self reads', 'const denied = s\\u0065lf.location;', 'self']
  ])(
    'returns runtime_error for guarded %s',
    (_label, statement, capability) => {
      const result = evaluateNativeTrustedBlockSource({
        source: `
import React from 'react';

${statement}

export default function Block() {
  return React.createElement('div');
}
`,
        modules: createModules()
      });

      expect(result).toMatchObject({
        ok: false,
        error: {
          kind: 'runtime_error',
          errors: [
            {
              code: 'runtime_error',
              path: `runtime.capability.${capability}`
            }
          ]
        }
      });
      expect(result.ok ? '' : result.error.message).toContain(capability);
    }
  );

  test('maps document.cookie writes to a structured runtime_error', () => {
    const result = evaluateNativeTrustedBlockSource({
      source: `
import React from 'react';

d\\u006fcument['coo' + 'kie'] = 'session=denied';

export default function Block() {
  return React.createElement('div');
}
`,
      modules: createModules()
    });

    expect(result).toMatchObject({
      ok: false,
      error: {
        kind: 'runtime_error',
        errors: [
          {
            code: 'runtime_error',
            path: 'runtime.capability.document.cookie'
          }
        ]
      }
    });
  });

  test('maps escaped fetch calls during component invocation to a structured runtime_error', () => {
    const result = evaluateNativeTrustedBlockSource({
      source: `
import React from 'react';

export default function Block() {
  f\\u0065tch('/api/native-trusted-block');
  return React.createElement('div');
}
`,
      modules: createModules()
    });

    expect(result.ok).toBe(true);
    if (!result.ok) {
      return;
    }

    expect(() => result.component({ props: {} })).toThrowError(
      expect.objectContaining({
        name: 'NativeTrustedBlockRuntimeError',
        kind: 'runtime_error',
        errors: [
          expect.objectContaining({
            code: 'runtime_error',
            path: 'runtime.capability.fetch'
          })
        ]
      })
    );
  });

  test('returns runtime_error when the source has no default export', () => {
    const result = evaluateNativeTrustedBlockSource({
      source: 'const Block = () => null;',
      modules: createModules()
    });

    expect(result).toMatchObject({
      ok: false,
      error: {
        kind: 'runtime_error',
        errors: [{ code: 'runtime_error', path: 'source.defaultExport' }]
      }
    });
  });

  test('returns runtime_error when the default export is not a function', () => {
    const result = evaluateNativeTrustedBlockSource({
      source: 'export default { render() { return null; } };',
      modules: createModules()
    });

    expect(result).toMatchObject({
      ok: false,
      error: {
        kind: 'runtime_error',
        errors: [{ code: 'runtime_error', path: 'source.defaultExport' }]
      }
    });
  });
});
