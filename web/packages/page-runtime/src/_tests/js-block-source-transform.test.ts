import { describe, expect, test } from 'vitest';

import { transformJsBlockSource } from '../index';

describe('JS block source transform', () => {
  test('rewrites imports and a direct BlockModule default export', () => {
    const result = transformJsBlockSource(`
import { Text as BlockText } from '@1flowbase/block-renderer/antd-facade';

async function main() {
  return { view: BlockText({ children: 'Ready' }), outputs: {} };
}

export default { main };
`);
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.importBindings).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          source: '@1flowbase/block-renderer/antd-facade',
          imported: 'Text',
          local: 'BlockText'
        })
      ])
    );
    expect(result.executableBody).toContain(
      'const __flowbaseJsBlockDefaultExport = { main };'
    );
    expect(result.executableBody).not.toContain('export default');
  });

  test('supports namespace and default imports from explicitly allowed modules', () => {
    const result = transformJsBlockSource(`
import * as Facade from '@1flowbase/block-renderer/antd-facade';
import BlockSdk from '@1flowbase/block-sdk';
async function main() {
  return { view: Facade.Text({ children: BlockSdk }), outputs: {} };
}
export default { main };
`);
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.importBindings.map((binding) => binding.kind)).toEqual([
        'namespace',
        'default'
      ]);
    }
  });

  test('preserves comments and strings containing import/export text', () => {
    const result = transformJsBlockSource(`
const text = "export default and import are data";
// export default fake
async function main() {
  return { view: { primitive: 'Text', props: { children: text } }, outputs: {} };
}
export default { main };
`);
    expect(result.ok).toBe(true);
  });

  test.each([
    ['missing default export', 'const value = 1;', 'source.defaultExport'],
    [
      'multiple default exports',
      'export default { main(){} }; export default { main(){} };',
      'source.defaultExport'
    ],
    [
      'reserved transform identifier',
      'const __flowbaseJsBlockModules = {}; export default { main(){} };',
      'source.identifiers'
    ]
  ])('rejects %s', (_label, source, path) => {
    const result = transformJsBlockSource(source);
    expect(result).toMatchObject({ ok: false, errors: [{ path }] });
  });
});
