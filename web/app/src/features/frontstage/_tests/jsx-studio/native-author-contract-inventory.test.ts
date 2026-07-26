import { readFileSync } from 'node:fs';
import { describe, expect, test } from 'vitest';

import { FRONTSTAGE_NATIVE_REACT_MONACO_EXTRA_LIBS } from '../../lib/jsx-studio/native-react-editor-contract';
import { createBlankJsBlockTemplateCode } from '../../lib/block-templates';

const legacyAuthorFragments = [
  'function main',
  'BlockModule',
  'BlockResult',
  'actionId',
  'formValues',
  'antd-facade'
];

describe('D4 Native React author contract inventory', () => {
  test('D4-AC-005 teaches only standard React default-export source', () => {
    const manifest = readFileSync(
      new URL(
        '../../../../../../../api/plugins/capability-plugins/1flowbase/manifest.yaml',
        import.meta.url
      ),
      'utf8'
    );
    const builtInTemplate = createBlankJsBlockTemplateCode({
      blockId: 'inventory-block',
      codeRef: 'inventory-code',
      contributionCode: 'frontstage.js-ui-block'
    });
    const monacoDeclarations = FRONTSTAGE_NATIVE_REACT_MONACO_EXTRA_LIBS.map(
      ({ content }) => content
    ).join('\n');

    for (const authorSurface of [
      manifest,
      builtInTemplate,
      monacoDeclarations
    ]) {
      expect(authorSurface).toContain('React');
      for (const legacyFragment of legacyAuthorFragments) {
        expect(authorSurface).not.toContain(legacyFragment);
      }
    }
    expect(manifest).toContain('export default function ExampleBlock');
    expect(builtInTemplate).toContain('export default function Block');
    expect(monacoDeclarations).toContain('interface NativeReactBlockProps');
  });
});
