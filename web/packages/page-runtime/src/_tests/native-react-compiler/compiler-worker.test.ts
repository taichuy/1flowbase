import { describe, expect, test } from 'vitest';

import {
  attachNativeReactCompilerWorker,
  canonicalizeNativeReactComponentArtifact,
  handleNativeReactCompilerRequest,
  type NativeReactCompilerResponse,
  type NativeReactCompilerWorkerScope
} from '../../index';

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
  test('D1-AC-001 compiles a standard default-export TSX component into a serializable artifact', () => {
    const response = handleNativeReactCompilerRequest({
      direction: 'host_to_worker',
      type: 'compile_native_react_component',
      requestId: 'compile-1',
      source: standardReactComponentFixture
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
        source: standardReactComponentFixture
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
      source: 'export default function Block() { return <div>; }'
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

  test('D2-AC-003 records the exact Catalog dependency lock and rejects an unlocked export', () => {
    const dependencyLock = [
      {
        module_source: '@1flowbase/native-components',
        module_version: '1.0.0',
        binding: 'fetched' as const,
        assets: [
          {
            role: 'browser_module' as const,
            media_type: 'text/javascript; charset=utf-8',
            sha256: '0'.repeat(64),
            url: `/api/console/frontstage/workspace-1/component-module-assets/${'0'.repeat(64)}`
          }
        ],
        exports: ['Surface']
      }
    ];
    const response = handleNativeReactCompilerRequest({
      direction: 'host_to_worker',
      type: 'compile_native_react_component',
      requestId: 'compile-catalog',
      source:
        "import { Surface } from '@1flowbase/native-components'; export default function Block() { return <Surface />; }",
      dependencyLock
    });

    expect(response.type).toBe('native_react_component_compiled');
    if (response.type === 'native_react_component_compiled') {
      expect(response.artifact.dependencyLock).toEqual(dependencyLock);
    }

    expect(
      handleNativeReactCompilerRequest({
        direction: 'host_to_worker',
        type: 'compile_native_react_component',
        requestId: 'compile-missing-export',
        source:
          "import { Missing } from '@1flowbase/native-components'; export default Missing;",
        dependencyLock
      })
    ).toMatchObject({
      type: 'native_react_component_compile_failed',
      diagnostics: [{ path: expect.stringContaining('Missing') }]
    });
  });
});
