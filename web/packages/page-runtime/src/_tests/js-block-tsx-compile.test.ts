import { describe, expect, test } from 'vitest';

import * as antdFacade from '@1flowbase/block-renderer/antd-facade';
import * as blockSdk from '@1flowbase/block-sdk';

import { compileJsBlockTsxSource } from '../js-block-tsx-compile';
import { evaluateJsBlockSource } from '../js-block-source-evaluator';

const modules = {
  '@1flowbase/block-sdk': blockSdk as Record<string, unknown>,
  '@1flowbase/block-renderer/antd-facade': antdFacade as Record<string, unknown>
};

describe('compileJsBlockTsxSource', () => {
  test('AC-001 strips pure TypeScript syntax even when the source has no JSX', () => {
    const source = [
      'type Greeting = { message: string };',
      "const greeting: Greeting = { message: 'hello' };",
      'export default { render() { return greeting.message; } };'
    ].join('\n');

    const result = compileJsBlockTsxSource(source);
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.changed).toBe(true);
      expect(result.code).not.toContain('type Greeting');
      expect(result.code).not.toContain(': Greeting');
      expect(result.code).toContain("const greeting = { message: 'hello' }");
    }
  });

  test('preserves plain JavaScript semantics without injecting a JSX runtime', () => {
    const source = 'const value = 1 < 2;\nexport default value;';
    const result = compileJsBlockTsxSource(source);
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.code).not.toContain(
        "from '@1flowbase/block-renderer/antd-facade'"
      );
    }
  });

  test('preserves imports for the source policy', () => {
    const source = "import React from 'react';\nexport default {};";

    const result = compileJsBlockTsxSource(source);

    expect(result).toEqual({ ok: true, code: source, changed: false });
  });

  test('compiles JSX into h() calls and injects the runtime import', () => {
    const source = [
      "import { defineBlock } from '@1flowbase/block-sdk';",
      '',
      'export default defineBlock({',
      '  render() {',
      '    return <Stack />;',
      '  }',
      '});'
    ].join('\n');

    const result = compileJsBlockTsxSource(source);
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.changed).toBe(true);
      expect(result.code).toContain('h(');
      expect(
        result.code.startsWith(
          "import { h, Fragment } from '@1flowbase/block-renderer/antd-facade';"
        )
      ).toBe(true);
    }
  });

  test('does not duplicate runtime import when h is already imported', () => {
    const source = [
      "import { h, Fragment, Stack } from '@1flowbase/block-renderer/antd-facade';",
      'export default { render() { return <Stack />; } };'
    ].join('\n');

    const result = compileJsBlockTsxSource(source);
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(
        result.code.match(/from '@1flowbase\/block-renderer\/antd-facade'/g)
      ).toHaveLength(1);
    }
  });

  test('reports a structured error for malformed JSX', () => {
    const result = compileJsBlockTsxSource('export default <Stack;');
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.errors[0]?.code).toBe('transform_failed');
      expect(result.errors[0]?.path).toBe('source.tsx');
    }
  });
});

describe('evaluateJsBlockSource with TSX and JSX', () => {
  test('evaluates a JSX block end-to-end into a UI schema', async () => {
    const source = [
      "import { defineBlock } from '@1flowbase/block-sdk';",
      "import { Stack, Text, Title } from '@1flowbase/block-renderer/antd-facade';",
      '',
      'export default defineBlock({',
      "  id: 'jsx-demo',",
      '  render(ctx) {',
      '    return (',
      '      <Stack>',
      '        <Title>JSX Demo</Title>',
      "        <Text>{'hello ' + ctx.props.name}</Text>",
      '      </Stack>',
      '    );',
      '  }',
      '});'
    ].join('\n');

    const evaluation = evaluateJsBlockSource({ source, modules });
    expect(evaluation.ok).toBe(true);
    if (!evaluation.ok) return;

    const schema = await evaluation.block.render({
      props: { name: 'world' },
      state: {}
    } as never);

    expect(schema).toMatchObject({
      primitive: 'Stack',
      children: [
        { primitive: 'Title', props: { children: 'JSX Demo' } },
        { primitive: 'Text', props: { children: 'hello world' } }
      ]
    });
  });

  test('JSX props and conditional children survive compilation', async () => {
    const source = [
      "import { defineBlock } from '@1flowbase/block-sdk';",
      "import { Alert, Button, Stack } from '@1flowbase/block-renderer/antd-facade';",
      '',
      'export default defineBlock({',
      '  render(ctx) {',
      '    const error = ctx.state.error;',
      '    return (',
      '      <Stack>',
      '        {error ? <Alert type="error" message={error} /> : null}',
      '        <Button actionId="data_model.orders.list">Refresh</Button>',
      '      </Stack>',
      '    );',
      '  }',
      '});'
    ].join('\n');

    const evaluation = evaluateJsBlockSource({ source, modules });
    expect(evaluation.ok).toBe(true);
    if (!evaluation.ok) return;

    const schema = await evaluation.block.render({
      props: {},
      state: { error: 'boom' }
    } as never);

    expect(schema).toMatchObject({
      primitive: 'Stack',
      children: [
        { primitive: 'Alert', props: { type: 'error', message: 'boom' } },
        {
          primitive: 'Button',
          props: { actionId: 'data_model.orders.list', children: 'Refresh' }
        }
      ]
    });
  });

  test('rejects JSX referencing components outside the facade whitelist', () => {
    const source = [
      "import { defineBlock } from '@1flowbase/block-sdk';",
      "import { danger } from 'not-allowed';",
      "import { Stack } from '@1flowbase/block-renderer/antd-facade';",
      'export default defineBlock({ render() { return <Stack>{danger()}</Stack>; } });'
    ].join('\n');

    const evaluation = evaluateJsBlockSource({ source, modules });
    expect(evaluation.ok).toBe(false);
  });
});
