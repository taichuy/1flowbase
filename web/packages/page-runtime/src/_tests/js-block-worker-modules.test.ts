import { describe, expect, test } from 'vitest';

import {
  attachDefaultJsBlockWorkerRuntime,
  createDefaultJsBlockInjectedModules,
  createDefaultJsBlockWorkerExecutor,
  createCatalogJsBlockRuntimeCapabilities,
  JS_BLOCK_ALLOWED_IMPORTS,
  JS_BLOCK_DEFAULT_MODULE_SOURCES
} from '../index';
import type {
  JsBlockWorkerRuntimeScope,
  JsBlockWorkerToHostMessage
} from '../index';

const source = `
import { defineBlock } from '@1flowbase/block-sdk';
import { Stack, Text } from '@1flowbase/block-renderer/antd-facade';

export default defineBlock({
  render() {
    return Stack({
      children: [Text({ children: 'Ready' })]
    });
  }
});
`;

describe('JS block default worker modules', () => {
  test('uses catalog modules for editor types, import allowlist, and worker injection', () => {
    const capabilities = createCatalogJsBlockRuntimeCapabilities({
      code_template: 'export default {}',
      code_template_version: '1.0.0',
      code_template_language: 'tsx',
      code_modules: [
        {
          source: '@1flowbase/block-sdk',
          type_declarations: 'export declare function defineBlock(): unknown;'
        }
      ]
    });

    expect(capabilities.allowedImports).toEqual(['@1flowbase/block-sdk']);
    expect(capabilities.monacoExtraLibs[0]?.content).toContain('defineBlock');
    expect(Object.keys(capabilities.workerModules)).toEqual([
      '@1flowbase/block-sdk'
    ]);
  });
  test('keeps default module sources aligned with the source policy allowlist', () => {
    expect(JS_BLOCK_DEFAULT_MODULE_SOURCES).toEqual(JS_BLOCK_ALLOWED_IMPORTS);
  });

  test('creates a default first-party injected module map', () => {
    const modules = createDefaultJsBlockInjectedModules();

    expect(Object.keys(modules).sort()).toEqual(
      [...JS_BLOCK_ALLOWED_IMPORTS].sort()
    );
    expect(modules['@1flowbase/block-sdk']).toMatchObject({
      defineBlock: expect.any(Function)
    });
    expect(modules['@1flowbase/block-renderer/antd-facade']).toMatchObject({
      Stack: expect.any(Function),
      Text: expect.any(Function)
    });
  });

  test('lets host code override a default module for tests or future runtime injection', async () => {
    const executor = createDefaultJsBlockWorkerExecutor({
      moduleOverrides: {
        '@1flowbase/block-renderer/antd-facade': {
          Stack(input: { children?: unknown }) {
            return { primitive: 'Grid', children: input.children };
          },
          Text(input: { children?: unknown }) {
            return { primitive: 'Caption', props: { children: input.children } };
          }
        }
      }
    });

    const messages = await executor.handleMessage({
      direction: 'host_to_worker',
      type: 'run',
      request: createRunRequest()
    });

    expect(messages).toEqual([
      {
        direction: 'worker_to_host',
        type: 'rendered',
        requestId: 'request-1',
        schema: {
          primitive: 'Grid',
          children: [
            {
              primitive: 'Caption',
              props: { children: 'Ready' }
            }
          ]
        }
      }
    ]);
  });

  test('runs a JS block through the default first-party modules', async () => {
    const executor = createDefaultJsBlockWorkerExecutor();

    const messages = await executor.handleMessage({
      direction: 'host_to_worker',
      type: 'run',
      request: createRunRequest()
    });

    expect(messages).toEqual([
      {
        direction: 'worker_to_host',
        type: 'rendered',
        requestId: 'request-1',
        schema: {
          primitive: 'Stack',
          children: [
            {
              primitive: 'Text',
              props: { children: 'Ready' }
            }
          ]
        }
      }
    ]);
  });

  test('attaches the default runtime to a worker-like scope', async () => {
    const messages: JsBlockWorkerToHostMessage[] = [];
    let listener: ((event: { data: unknown }) => void) | null = null;
    const scope: JsBlockWorkerRuntimeScope = {
      postMessage: (message) => messages.push(message),
      addEventListener: (_type, nextListener) => {
        listener = nextListener;
      },
      removeEventListener: (_type, nextListener) => {
        if (listener === nextListener) {
          listener = null;
        }
      }
    };

    const attached = attachDefaultJsBlockWorkerRuntime(scope);
    listener?.({ data: { direction: 'host_to_worker', type: 'init' } });
    listener?.({
      data: {
        direction: 'host_to_worker',
        type: 'run',
        request: createRunRequest()
      }
    });
    await attached.flush();

    attached.dispose();

    expect(messages).toEqual([
      { direction: 'worker_to_host', type: 'ready' },
      {
        direction: 'worker_to_host',
        type: 'rendered',
        requestId: 'request-1',
        schema: {
          primitive: 'Stack',
          children: [
            {
              primitive: 'Text',
              props: { children: 'Ready' }
            }
          ]
        }
      }
    ]);
    expect(listener).toBeNull();
  });
});

function createRunRequest() {
  return {
    requestId: 'request-1',
    blockId: 'block-1',
    source,
    props: {},
    state: {},
    contextSnapshot: {
      workspace: { id: 'workspace-1' },
      application: { id: 'app-1' },
      page: { id: 'page-1', route: '/frontstage/page-1' }
    },
    limits: {
      timeoutMs: 1000,
      maxRenderDepth: 8,
      maxRenderNodes: 250
    }
  };
}
