import { describe, expect, test } from 'vitest';

import {
  attachNativeReactCompilerWorker,
  canonicalizeNativeReactComponentArtifact,
  handleNativeReactCompilerRequest,
  type NativeReactCompilerResponse,
  type NativeReactCompilerWorkerScope
} from '../../index';

const coreModuleDefinitions = [
  { module_source: 'react', exports: ['Fragment', 'useState'] },
  {
    module_source: 'react/jsx-runtime',
    exports: ['Fragment', 'jsx', 'jsxs']
  },
  { module_source: 'tailwindcss', exports: [] }
];

const standardReactComponentFixture = `
import { Fragment, useState, type CSSProperties } from 'react';

interface BlockProps {
  ctx: { title: string };
}

throw new Error('Compiler Worker must not execute component modules.');

export default function Block({ ctx }: BlockProps) {
  const [count, setCount] = useState<number>(0);
  const style: CSSProperties = { color: 'rebeccapurple' };
  return (
    <>
      <style>{\`:root { --native-tone: rebeccapurple; }
        @keyframes native-pulse { from { opacity: 0.8; } to { opacity: 1; } }
        .native-counter { color: var(--native-tone); animation: native-pulse 1s; }\`}</style>
      <Fragment>
        <button
          className="native-counter"
          style={style}
          onClick={() => setCount((current) => current + 1)}
        >
          {ctx.title}: {count}
        </button>
      </Fragment>
    </>
  );
}
`;

describe('Native React compiler Worker contract', () => {
  test('treats tailwindcss as a compile-only capability import', () => {
    const response = handleNativeReactCompilerRequest({
      direction: 'host_to_worker',
      type: 'compile_native_react_component',
      requestId: 'compile-tailwind',
      source: `import 'tailwindcss'; export default () => <div className="p-4" />;`,
      moduleDefinitions: coreModuleDefinitions
    });
    expect(response.type).toBe('native_react_component_compiled');
    if (response.type !== 'native_react_component_compiled') return;
    expect(
      response.artifact.program.injectedModules.map(({ source }) => source)
    ).not.toContain('tailwindcss');
  });

  test('D1-AC-001 compiles a standard default-export TSX component into a serializable artifact', () => {
    const response = handleNativeReactCompilerRequest({
      direction: 'host_to_worker',
      type: 'compile_native_react_component',
      requestId: 'compile-1',
      source: standardReactComponentFixture,
      moduleDefinitions: coreModuleDefinitions
    });

    expect(response.type).toBe('native_react_component_compiled');
    if (response.type !== 'native_react_component_compiled') return;

    expect(response.diagnostics).toEqual([]);
    expect(response.artifact.sourceMap).toMatchObject({
      version: 3,
      sources: ['native-react-block.tsx']
    });
    expect(
      response.artifact.program.injectedModules.map(({ source }) => source)
    ).toEqual(expect.arrayContaining(['react', 'react/jsx-runtime']));
    expect(response.artifact.program.executableBody).toContain("'button'");
    expect(response.artifact.program.executableBody).not.toContain(
      'BlockUiSchema'
    );

    const structuredCloneArtifact = structuredClone(response.artifact);
    expect(
      canonicalizeNativeReactComponentArtifact(structuredCloneArtifact)
    ).toEqual(response.artifact);
    expect(
      canonicalizeNativeReactComponentArtifact(
        JSON.parse(JSON.stringify(response.artifact))
      )
    ).toEqual(response.artifact);
  });

  test('D1-AC-002 posts one artifact and closes without executing component code', () => {
    const responses: NativeReactCompilerResponse[] = [];
    let closeCount = 0;
    const scope: NativeReactCompilerWorkerScope = {
      onmessage: null,
      postMessage(message) {
        responses.push(message);
      },
      close() {
        closeCount += 1;
      }
    };

    attachNativeReactCompilerWorker(scope);
    scope.onmessage?.({
      data: {
        direction: 'host_to_worker',
        type: 'compile_native_react_component',
        requestId: 'compile-2',
        source: standardReactComponentFixture,
        moduleDefinitions: coreModuleDefinitions
      }
    });

    expect(responses).toHaveLength(1);
    expect(responses[0]).toMatchObject({
      type: 'native_react_component_compiled',
      requestId: 'compile-2'
    });
    expect(scope.onmessage).toBeNull();
    expect(closeCount).toBe(1);
  });

  test('D1-AC-002 returns stable compile diagnostics for malformed TSX', () => {
    const response = handleNativeReactCompilerRequest({
      direction: 'host_to_worker',
      type: 'compile_native_react_component',
      requestId: 'compile-invalid',
      source: 'export default function Block() { return <div>; }',
      moduleDefinitions: coreModuleDefinitions
    });

    expect(response).toMatchObject({
      direction: 'worker_to_host',
      type: 'native_react_component_compile_failed',
      requestId: 'compile-invalid',
      diagnostics: [
        {
          phase: 'compile',
          code: 'transform_failed',
          path: 'source.tsx'
        }
      ]
    });
  });

  test('AC-002/008 validates imports against frontend module definitions without storing them', () => {
    const moduleDefinitions = [
      ...coreModuleDefinitions,
      {
        module_source: '@1flowbase/native-components',
        exports: ['Surface']
      }
    ];
    const response = handleNativeReactCompilerRequest({
      direction: 'host_to_worker',
      type: 'compile_native_react_component',
      requestId: 'compile-catalog',
      source:
        "import { Surface } from '@1flowbase/native-components'; export default function Block() { return <Surface />; }",
      moduleDefinitions
    });

    expect(response.type).toBe('native_react_component_compiled');
    if (response.type === 'native_react_component_compiled') {
      expect(response.artifact).not.toHaveProperty('dependencyLock');
    }

    expect(
      handleNativeReactCompilerRequest({
        direction: 'host_to_worker',
        type: 'compile_native_react_component',
        requestId: 'compile-missing-export',
        source:
          "import { Missing } from '@1flowbase/native-components'; export default Missing;",
        moduleDefinitions
      })
    ).toMatchObject({
      type: 'native_react_component_compile_failed',
      diagnostics: [{ path: expect.stringContaining('Missing') }]
    });
  });
});
