import { describe, expect, test } from 'vitest';

import {
  LEGACY_BLOCK_MODULE_SOURCE_DIAGNOSTIC,
  compileNativeReactComponent,
  diagnoseLegacyBlockModuleSource
} from '../../index';

describe('Native React source contract', () => {
  test.each([
    "import { Text } from '@1flowbase/block-renderer/antd-facade'; export default function Block() { return <Text />; }",
    'async function main(ctx) { return { view: null, outputs: {} }; } export default { main };',
    'const run = async (ctx) => ({ view: null, outputs: {} }); export default { main: run } satisfies BlockModule;'
  ])('D4-AC-006 returns one stable diagnostic for legacy source', (source) => {
    expect(diagnoseLegacyBlockModuleSource(source)).toEqual(
      LEGACY_BLOCK_MODULE_SOURCE_DIAGNOSTIC
    );
    expect(compileNativeReactComponent(source)).toEqual({
      ok: false,
      diagnostics: [LEGACY_BLOCK_MODULE_SOURCE_DIAGNOSTIC]
    });
  });

  test('D4-AC-005 accepts a standard React default export without rewriting it', () => {
    const source = `import { useState } from 'react';
export default function Block({ ctx }) {
  const [count, setCount] = useState(0);
  return <button onClick={() => setCount(count + 1)}>{String(ctx.props.label)}: {count}</button>;
}`;
    expect(diagnoseLegacyBlockModuleSource(source)).toBeNull();
    expect(source).toContain('export default function Block');
  });

  test('does not classify documentation text as executable legacy syntax', () => {
    const source = `// Do not use: export default { main } satisfies BlockModule.
export default function Block() {
  return <pre>{'@1flowbase/block-renderer/antd-facade'}</pre>;
}`;
    expect(diagnoseLegacyBlockModuleSource(source)).toBeNull();
  });
});
