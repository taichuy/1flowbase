import { describe, expect, test } from 'vitest';
import * as antdFacade from '@1flowbase/block-renderer/antd-facade';

import { evaluateJsBlockSource, runJsBlockSource } from '../index';
import { compileJsBlockTsxSource } from '../js-block-tsx-compile';
import type { BlockContext } from '@1flowbase/page-protocol';

const context = {
  currentUser: null,
  workspace: { id: 'workspace-1' },
  application: { id: 'application-1' },
  page: { id: 'page-1', route: '/demo' },
  inputs: {},
  params: {},
  props: { visible: true },
  state: {},
  patch() {},
  interfaces: { async call() {} },
  events: { emit() {} },
  theme: { mode: 'light', tokens: {} },
  ui: { locale: 'en_US' }
} as BlockContext;

describe('compileJsBlockTsxSource', () => {
  test('returns source maps even when ordinary JavaScript needs no JSX rewrite', () => {
    const source =
      'async function main(){return {view:null,outputs:{}}} export default {main};';
    expect(compileJsBlockTsxSource(source)).toMatchObject({
      ok: true,
      changed: false,
      code: source,
      sourceMap: { version: 3, sources: ['block.tsx'] }
    });
  });

  test('compiles a typed TSX BlockModule and executes main end to end', async () => {
    const source = `
import type { BlockModule } from '@1flowbase/block-sdk';
import { Stack, Text } from '@1flowbase/block-renderer/antd-facade';

async function main(ctx: BlockContext) {
  const title: string = 'Ready';
  return {
    view: <Stack>{ctx.props.visible ? <Text>{title}</Text> : null}</Stack>,
    outputs: { title }
  };
}

export default { main } satisfies BlockModule;
`;
    const modules = {
      '@1flowbase/block-renderer/antd-facade': antdFacade as Record<
        string,
        unknown
      >
    };
    expect(evaluateJsBlockSource({ source, modules }).ok).toBe(true);
    await expect(
      runJsBlockSource({ source, modules, context })
    ).resolves.toMatchObject({
      ok: true,
      result: { outputs: { title: 'Ready' } }
    });
  });

  test('reports malformed TSX as a compile error', () => {
    expect(compileJsBlockTsxSource('export default <Stack;')).toMatchObject({
      ok: false,
      errors: [{ code: 'transform_failed' }]
    });
  });
});
