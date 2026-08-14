import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { describe, expect, test } from 'vitest';

import {
  compileTailwindUtilities,
  extractStaticTailwindCandidates,
  findUnboundedTailwindClassExpressions,
  sourceImportsTailwind
} from '../compiler';
import {
  TAILWIND_PREFLIGHT_CSS,
  TAILWIND_STYLESHEET_SHA256,
  TAILWIND_THEME_CSS,
  TAILWIND_UTILITIES_CSS
} from '../stylesheet-contract';

describe('Native React Tailwind candidate compiler', () => {
  const stylesheets = {
    themeCss: readFileSync(
      new URL(
        '../../../../node_modules/tailwindcss/theme.css',
        import.meta.url
      ),
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

  test('compiles only finite class candidates found in class expressions', async () => {
    const candidates = extractStaticTailwindCandidates(
      `const color = active ? 'red' : 'blue'; export default () => <div className={\`p-4 bg-\${color}-500\`} />;`
    );
    expect(candidates).toEqual(['bg-blue-500', 'bg-red-500', 'p-4']);
    const result = await compileTailwindUtilities(candidates);
    expect(result.acceptedCandidates).toEqual(candidates);
    expect(result.css).toContain('.bg-red-500');
    expect(result.css.length).toBeLessThan(20_000);
  });

  test('AC-001 keeps import recognition independent from authored candidates', () => {
    expect(
      sourceImportsTailwind("import 'tailwindcss';\nexport default 1;")
    ).toBe(true);
    expect(sourceImportsTailwind('export default 1;')).toBe(false);
  });

  test('diagnoses only class expressions whose values are not finite', () => {
    const source = `
      import 'tailwindcss';
      const color = active ? 'red' : 'blue';
      const remoteClass = readRemoteClass();
      export default () => <>
        <div className="p-4" />
        <div className={active ? 'grid' : 'flex'} />
        <div className={\`bg-\${color}-500\`} />
        <div className={remoteClass} />
      </>;
    `;
    expect(findUnboundedTailwindClassExpressions(source)).toEqual([
      expect.objectContaining({ expression: 'remoteClass' })
    ]);
  });
});
