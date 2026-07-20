import { describe, expect, test, vi } from 'vitest';

import type { BlockContext } from '@1flowbase/page-protocol';

import {
  evaluateJsBlockSource,
  runJsBlockSource,
  type JsBlockInjectedModuleMap
} from '../index';

const source = `
import { Text } from '@1flowbase/block-renderer/antd-facade';

async function main(ctx) {
  return {
    view: Text({ children: ctx.props.title }),
    outputs: { title: ctx.props.title }
  };
}

export default { main };
`;

function modules(
  overrides: JsBlockInjectedModuleMap = {}
): JsBlockInjectedModuleMap {
  return {
    '@1flowbase/block-renderer/antd-facade': {
      Text(input: { children?: unknown }) {
        return { primitive: 'Text', props: { children: input.children } };
      }
    },
    ...overrides
  };
}

function context(): BlockContext {
  return {
    currentUser: { id: 'user-1', displayName: 'Ada' },
    workspace: { id: 'workspace-1' },
    application: { id: 'application-1' },
    page: { id: 'page-1', route: '/demo' },
    inputs: {},
    params: {},
    props: { title: 'Ready' },
    state: {},
    patch: vi.fn(),
    interfaces: { call: vi.fn() },
    events: { emit: vi.fn() },
    theme: { mode: 'light', tokens: {} },
    ui: { locale: 'en_US' }
  };
}

describe('JS block source evaluator', () => {
  test('evaluates BlockModule.main and returns a validated BlockResult', async () => {
    await expect(
      runJsBlockSource({ source, modules: modules(), context: context() })
    ).resolves.toMatchObject({
      ok: true,
      result: {
        view: { primitive: 'Text', props: { children: 'Ready' } },
        outputs: { title: 'Ready' }
      }
    });
  });

  test('reuses a compiled source object without transforming it again', () => {
    const first = evaluateJsBlockSource({ source, modules: modules() });
    expect(first.ok).toBe(true);
    if (!first.ok) return;
    const second = evaluateJsBlockSource({
      source: first.compiledSource,
      modules: modules()
    });
    expect(second.ok).toBe(true);
    if (second.ok) expect(second.compiledSource).toBe(first.compiledSource);
  });

  test('fails closed when an imported facade binding is unavailable', () => {
    expect(
      evaluateJsBlockSource({
        source,
        modules: modules({
          '@1flowbase/block-renderer/antd-facade': {}
        })
      })
    ).toMatchObject({
      ok: false,
      error: {
        kind: 'runtime_error',
        errors: [{ path: 'modules.@1flowbase/block-renderer/antd-facade.Text' }]
      }
    });
  });

  test('rejects a default export without main', () => {
    expect(
      evaluateJsBlockSource({ source: 'export default {};', modules: {} })
    ).toMatchObject({
      ok: false,
      error: { errors: [{ path: 'source.defaultExport' }] }
    });
  });

  test('maps main failures and invalid BlockResult values to stable paths', async () => {
    await expect(
      runJsBlockSource({
        source:
          'async function main(){ throw new Error("boom"); } export default { main };',
        modules: {},
        context: context()
      })
    ).resolves.toMatchObject({
      ok: false,
      error: { errors: [{ path: 'runtime.main' }] }
    });
    await expect(
      runJsBlockSource({
        source:
          'async function main(){ return { primitive: "Text" }; } export default { main };',
        modules: {},
        context: context()
      })
    ).resolves.toMatchObject({
      ok: false,
      error: { errors: [{ path: 'runtime.result' }] }
    });
  });
});
