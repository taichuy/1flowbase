import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { describe, expect, test } from 'vitest';

import {
  compileTailwindBlockPreset,
  sourceImportsTailwind,
  TAILWIND_BLOCK_PRESET_VARIANTS
} from '../compiler';
import {
  TAILWIND_PREFLIGHT_CSS,
  TAILWIND_STYLESHEET_SHA256,
  TAILWIND_THEME_CSS,
  TAILWIND_UTILITIES_CSS
} from '../stylesheet-contract';

describe('Native React Tailwind block preset contract', () => {
  const stylesheets = {
    themeCss: readFileSync(
      new URL('../../../../node_modules/tailwindcss/theme.css', import.meta.url),
      'utf8'
    ),
    preflightCss: readFileSync(
      new URL(
        '../../../../node_modules/tailwindcss/preflight.css',
        import.meta.url
      ),
      'utf8'
    ),
    utilitiesCss: readFileSync(
      new URL(
        '../../../../node_modules/tailwindcss/utilities.css',
        import.meta.url
      ),
      'utf8'
    )
  };

  test('AC-001 freezes Tailwind theme, preflight and utility inputs', () => {
    expect(TAILWIND_THEME_CSS).toBe(stylesheets.themeCss);
    expect(TAILWIND_PREFLIGHT_CSS).toBe(stylesheets.preflightCss);
    expect(TAILWIND_UTILITIES_CSS).toBe(stylesheets.utilitiesCss);
    expect(TAILWIND_STYLESHEET_SHA256).toBe(
      createHash('sha256')
        .update(
          `${stylesheets.themeCss}\n${stylesheets.preflightCss}\n${stylesheets.utilitiesCss}`
        )
        .digest('hex')
    );
  });

  test('AC-001 builds one source-independent default preset with standard variants', async () => {
    const preset = await compileTailwindBlockPreset();

    expect(preset.baseCandidates).toBeGreaterThan(20_000);
    expect(preset.candidates).toBe(
      preset.baseCandidates * (TAILWIND_BLOCK_PRESET_VARIANTS.length + 1)
    );
    expect(preset.css).toContain('.grid');
    expect(preset.css).toContain('.bg-red-500');
    expect(preset.css).toContain('.hover\\:bg-red-500');
    expect(preset.css).toContain('.focus-visible\\:ring-2');
    expect(preset.css).toContain('.disabled\\:opacity-50');
    expect(preset.css).toContain('.md\\:grid-cols-2');
    expect(preset.css).toContain('@media');
    expect(preset.css).toMatch(/(?:^|\})\s*\*,\s*::after,/u);
    expect(preset.css).not.toMatch(/\.ant-/u);
  });

  test('AC-001 keeps import recognition independent from authored candidates', () => {
    expect(sourceImportsTailwind("import 'tailwindcss';\nexport default 1;"))
      .toBe(true);
    expect(sourceImportsTailwind('export default 1;')).toBe(false);
  });
});
