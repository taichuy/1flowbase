import { describe, expect, test } from 'vitest';

import {
  attachDefaultJsBlockWorkerRuntime,
  createDefaultJsBlockInjectedModules,
  createDefaultJsBlockWorkerExecutor,
  JS_BLOCK_ALLOWED_IMPORTS,
  type JsBlockWorkerToHostMessage
} from '../index';

const source = `
import { Text } from '@1flowbase/block-renderer/antd-facade';
async function main() {
  return { view: Text({ children: 'Ready' }), outputs: { ready: true } };
}
export default { main };
`;

const request = {
  requestId: 'request-1',
  blockId: 'block-1',
  source,
  props: {},
  state: {},
  contextSnapshot: {},
  limits: { timeoutMs: 1_000 },
  allowedImports: [...JS_BLOCK_ALLOWED_IMPORTS]
};

describe('JS block default worker modules', () => {
  test('exposes the current BlockModule guards and controlled renderer facade', () => {
    const modules = createDefaultJsBlockInjectedModules();
    expect(modules['@1flowbase/block-sdk']).toMatchObject({
      isBlockModule: expect.any(Function),
      isBlockResult: expect.any(Function)
    });
    expect(modules['@1flowbase/block-renderer/antd-facade']).toMatchObject({
      Text: expect.any(Function),
      h: expect.any(Function)
    });
  });

  test('runs BlockModule.main through the default injected modules', async () => {
    const messages = await createDefaultJsBlockWorkerExecutor().handleMessage({
      direction: 'host_to_worker',
      type: 'run',
      request
    });
    expect(messages).toContainEqual({
      direction: 'worker_to_host',
      type: 'completed',
      requestId: 'request-1',
      view: { primitive: 'Text', props: { children: 'Ready' } },
      outputs: { ready: true }
    });
  });

  test('attaches the same runtime contract to a worker-like scope', async () => {
    const messages: JsBlockWorkerToHostMessage[] = [];
    let listener: ((event: { data: unknown }) => void) | undefined;
    const attached = attachDefaultJsBlockWorkerRuntime({
      postMessage: (message) => messages.push(message),
      addEventListener: (_type, next) => {
        listener = next;
      },
      removeEventListener: () => undefined
    });
    listener?.({ data: { direction: 'host_to_worker', type: 'init' } });
    listener?.({ data: { direction: 'host_to_worker', type: 'run', request } });
    await attached.flush();
    expect(messages.map((message) => message.type)).toEqual(
      expect.arrayContaining(['ready', 'completed'])
    );
    attached.dispose();
  });
});
