import { describe, expect, test } from 'vitest';

import { diagnoseUnsupportedTailwindUtilities } from '../tailwind-utility-diagnostics';

describe('Native React Tailwind diagnostics', () => {
  test('AC-004 exposes unsupported static utilities as source diagnostics', () => {
    const source = [
      "import 'tailwindcss';",
      'export default function Block() {',
      '  return <div className="grid unknown-layout p-4" />;',
      '}'
    ].join('\n');

    expect(diagnoseUnsupportedTailwindUtilities(source)).toEqual([
      {
        code: 'transform_failed',
        path: 'source.classNames[0]',
        message:
          "Tailwind utility 'unknown-layout' is not available in the official low-code inventory.",
        sourceLocation: {
          line: 3,
          column: 31,
          endLine: 3,
          endColumn: 45
        }
      }
    ]);
  });
});
