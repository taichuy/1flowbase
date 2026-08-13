import { describe, expect, test } from 'vitest';

import {
  LEGACY_TAILWIND_UTILITY_CLASS_NAMES,
  recognizeLegacyUnsupportedTailwindUtilityClasses
} from '../inventory';

describe('legacy Tailwind utility inventory recognition', () => {
  test('AC-004 keeps the #1671 inventory recognizable but explicitly legacy', () => {
    expect(new Set(LEGACY_TAILWIND_UTILITY_CLASS_NAMES).size).toBe(
      LEGACY_TAILWIND_UTILITY_CLASS_NAMES.length
    );
    expect(LEGACY_TAILWIND_UTILITY_CLASS_NAMES).toEqual(
      expect.arrayContaining(['grid', 'gap-4', 'p-4', 'md:grid-cols-2'])
    );
  });

  test('AC-004 recognizes classes outside the legacy snapshot only for Tailwind sources', () => {
    const source = [
      "import 'tailwindcss';",
      'export default function Block() {',
      '  return <div className="grid made-up-utility p-4" />;',
      '}'
    ].join('\n');

    expect(recognizeLegacyUnsupportedTailwindUtilityClasses(source)).toEqual([
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
      recognizeLegacyUnsupportedTailwindUtilityClasses(
        'export default () => <div className="made-up-utility" />;'
      )
    ).toEqual([]);
  });

  test('AC-004 records dynamic expressions for legacy migration preview', () => {
    const source =
      "import 'tailwindcss';\nexport default ({ active }) => <div className={active ? 'grid' : 'unknown'} />;";

    expect(recognizeLegacyUnsupportedTailwindUtilityClasses(source)).toEqual([
      expect.objectContaining({
        className: '<dynamic className expression>',
        sourceLocation: expect.objectContaining({ line: 2, column: 37 })
      })
    ]);
  });
});
