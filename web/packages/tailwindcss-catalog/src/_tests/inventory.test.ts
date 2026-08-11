import { describe, expect, test } from 'vitest';

import {
  findUnsupportedTailwindUtilityClasses,
  TAILWIND_UTILITY_CLASS_NAMES
} from '../inventory';

describe('official Tailwind utility inventory', () => {
  test('AC-004 keeps the published inventory unique and includes the baseline layout utilities', () => {
    expect(new Set(TAILWIND_UTILITY_CLASS_NAMES).size).toBe(
      TAILWIND_UTILITY_CLASS_NAMES.length
    );
    expect(TAILWIND_UTILITY_CLASS_NAMES).toEqual(
      expect.arrayContaining(['grid', 'gap-4', 'p-4', 'md:grid-cols-2'])
    );
  });

  test('AC-004 reports unsupported static class names only when Tailwind is imported', () => {
    const source = [
      "import 'tailwindcss';",
      'export default function Block() {',
      '  return <div className="grid made-up-utility p-4" />;',
      '}'
    ].join('\n');

    expect(findUnsupportedTailwindUtilityClasses(source)).toEqual([
      {
        className: 'made-up-utility',
        sourceLocation: {
          line: 3,
          column: 31,
          endLine: 3,
          endColumn: 46
        }
      }
    ]);
    expect(
      findUnsupportedTailwindUtilityClasses(
        'export default () => <div className="made-up-utility" />;'
      )
    ).toEqual([]);
  });

  test('AC-004 rejects dynamic className expressions that the finite inventory cannot verify', () => {
    const source =
      "import 'tailwindcss';\nexport default ({ active }) => <div className={active ? 'grid' : 'unknown'} />;";

    expect(findUnsupportedTailwindUtilityClasses(source)).toEqual([
      expect.objectContaining({
        className: '<dynamic className expression>',
        sourceLocation: expect.objectContaining({ line: 2, column: 37 })
      })
    ]);
  });
});
