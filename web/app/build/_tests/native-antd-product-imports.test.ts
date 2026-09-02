import { describe, expect, test } from 'vitest';

import { rewriteProductAntDesignImports } from '../native-antd-es-modules';

describe('product Ant Design leaf imports', () => {
  test('rewrites values while preserving erased root types', () => {
    expect(
      rewriteProductAntDesignImports(
        "import { Button, Modal as Dialog, type MenuProps } from 'antd';"
      )
    ).toBe(
      [
        "import type { MenuProps } from 'antd';",
        'import Button from "antd/es/button";',
        'import Dialog from "antd/es/modal";'
      ].join('\n')
    );
  });

  test('does not rewrite unrelated modules', () => {
    const source = "import { Button } from '@other/ui';";
    expect(rewriteProductAntDesignImports(source)).toBe(source);
  });
});
