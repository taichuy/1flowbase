import { describe, expect, test } from 'vitest';

import {
  NATIVE_REACT_COMPILER_ABI,
  NATIVE_REACT_RUNTIME_ABI,
  canonicalizeNativeReactComponentArtifact,
  compileNativeReactComponent,
  evaluateNativeReactComponentArtifact,
  type NativeReactComponentArtifact,
  type NativeReactModuleDefinition
} from '../../index';

const source =
  "import { Surface } from '@1flowbase/native-components'; export default () => <Surface />;";

describe('Native React content-addressed artifact identity', () => {
  test('I1967-AC-001 compiles quoted regular expression character classes', () => {
    const result = compileNativeReactComponent(
      `import React from 'react';
import { Select } from 'antd';

const tokenize = (input: string): string[] => {
  const tokens: string[] = [];
  const regex = /"([^"]*)"|([^,\\n]+)/g;
  let match: RegExpExecArray | null = regex.exec(input);
  while (match) {
    tokens.push((match[1] ?? match[2]).trim());
    match = regex.exec(input);
  }
  return tokens.filter(Boolean);
};

const App: React.FC = () => (
  <Select mode="tags" tokenSeparators={tokenize} />
);

export default App;`,
      [
        { module_source: 'react', exports: ['default'] },
        {
          module_source: 'react/jsx-runtime',
          exports: ['Fragment', 'jsx', 'jsxs']
        },
        { module_source: 'antd', exports: ['Select'] }
      ]
    );

    expect(result).toMatchObject({ ok: true, diagnostics: [] });
    if (!result.ok) return;
    expect(
      evaluateNativeReactComponentArtifact(result.artifact, {
        react: { default: {} },
        'react/jsx-runtime': { jsx: () => null },
        antd: { Select: () => null }
      })
    ).toMatchObject({ ok: true, diagnostics: [] });
  });

  test.each([
    [
      'escaped slash and flags',
      `const matcher = /\\/api\\/[a-z]+/gi;
       export default () => matcher.test('/api/items');`
    ],
    [
      'division expression',
      `const ratio = (total: number, count: number) => total / count;
       export default () => ratio(6, 2);`
    ],
    [
      'regular expression beside template and JSX expressions',
      `const matcher = /"[^"]+"/;
       const label = \`matched: \${matcher.test('"value"')}\`;
       export default () => <div>{label}</div>;`
    ]
  ])('I1967-AC-002 accepts %s', (_label, currentSource) => {
    expect(
      compileNativeReactComponent(currentSource, [
        {
          module_source: 'react/jsx-runtime',
          exports: ['Fragment', 'jsx', 'jsxs']
        }
      ])
    ).toMatchObject({ ok: true, diagnostics: [] });
  });

  test.each([
    [
      'denied static import',
      `import dayjs from 'dayjs'; export default () => dayjs();`,
      'import_denied'
    ],
    [
      'dynamic import',
      `export default async () => import('antd');`,
      'import_denied'
    ],
    [
      'require invocation',
      `export default () => require('antd');`,
      'import_denied'
    ],
    [
      'portal ownership',
      `export default () => createPortal(node, target);`,
      'transform_failed'
    ],
    [
      'AntD privileged static API',
      `import { Modal } from 'antd'; export default () => Modal.confirm({});`,
      'transform_failed'
    ],
    [
      'prototype-chain escape',
      `export default () => value['constructor'];`,
      'transform_failed'
    ]
  ])(
    'I1967-AC-003 preserves policy denial for %s',
    (_label, currentSource, code) => {
      expect(
        compileNativeReactComponent(currentSource, [
          { module_source: 'antd', exports: ['Modal'] }
        ])
      ).toMatchObject({
        ok: false,
        diagnostics: [{ code }]
      });
    }
  );

  test('I1967-AC-003 keeps browser capabilities in the runtime-guard lane', () => {
    expect(
      compileNativeReactComponent(
        `export default () => {
          fetch('/api/example');
          document.querySelector('#root');
          localStorage.getItem('token');
          return null;
        };`
      )
    ).toMatchObject({ ok: true, diagnostics: [] });
  });

  test('AC-004/006/007 binds only source and compiler/runtime ABI', () => {
    const first = compile(definitions());
    const same = compile(definitions());
    const sourceChanged = compile(definitions(), `${source}\n// changed`);
    const registryExpanded = compile(
      definitions(['Surface', 'ScrollableSurface'])
    );

    expect(first.identity).toEqual({
      source_sha256: expect.stringMatching(/^[a-f0-9]{64}$/),
      compiler_abi: NATIVE_REACT_COMPILER_ABI,
      runtime_abi: NATIVE_REACT_RUNTIME_ABI
    });
    expect(same.identity).toEqual(first.identity);
    expect(registryExpanded.identity).toEqual(first.identity);
    expect(sourceChanged.identity.source_sha256).not.toBe(
      first.identity.source_sha256
    );
    expect(first).not.toHaveProperty('dependencyLock');
    expect(first.identity).not.toHaveProperty('dependency_lock_sha256');
    expect(first.identity).not.toHaveProperty('runtime_fingerprint');
  });

  test('AC-004 canonicalizer rejects old format, old ABI and corrupt program bytes', () => {
    const artifact = compile(definitions());
    const transportedArtifact = structuredClone(artifact);
    expect(
      transportedArtifact.program.importBindings.some(
        (binding) =>
          binding.kind === 'named' &&
          binding.source === '@1flowbase/native-components' &&
          binding.imported === 'Surface'
      )
    ).toBe(true);
    expect(
      canonicalizeNativeReactComponentArtifact(transportedArtifact)
    ).toEqual(artifact);

    expect(
      canonicalizeNativeReactComponentArtifact({ ...artifact, version: 2 })
    ).toBeNull();
    expect(
      canonicalizeNativeReactComponentArtifact({
        ...artifact,
        identity: { ...artifact.identity, runtime_abi: 'old-runtime' }
      })
    ).toBeNull();
    expect(
      canonicalizeNativeReactComponentArtifact({
        ...artifact,
        program: { ...artifact.program, executableBody: 'corrupt' }
      })
    ).toBeNull();
  });
});

function compile(
  moduleDefinitions: NativeReactModuleDefinition[],
  currentSource = source
): NativeReactComponentArtifact {
  const result = compileNativeReactComponent(currentSource, moduleDefinitions);
  if (!result.ok) throw new Error('Expected artifact compilation to succeed.');
  return result.artifact;
}

function definitions(
  exports: string[] = ['Surface']
): NativeReactModuleDefinition[] {
  return [
    {
      module_source: 'react/jsx-runtime',
      exports: ['Fragment', 'jsx', 'jsxs']
    },
    {
      module_source: '@1flowbase/native-components',
      exports
    }
  ];
}
