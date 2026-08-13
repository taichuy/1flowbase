import { describe, expect, test } from 'vitest';
import { readFileSync } from 'node:fs';

import {
  compileTailwindUtilities,
  extractStaticTailwindCandidates
} from '../compiler';

describe('Native React Tailwind compiler contract', () => {
  const stylesheets = {
    themeCss: readFileSync(
      new URL(
        '../../../../node_modules/tailwindcss/theme.css',
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

  test('AC-001 compiles standard static variants and arbitrary values without a private inventory', async () => {
    const source = [
      "import 'tailwindcss';",
      'export default function Block() {',
      '  return (',
      '    <div className="grid grid-cols-[200px_1fr] bg-[#00ab73] md:grid-cols-2 hover:[&>span]:opacity-80">',
      '      <span>content</span>',
      '    </div>',
      '  );',
      '}'
    ].join('\n');

    const candidates = extractStaticTailwindCandidates(source);
    const result = await compileTailwindUtilities(candidates, stylesheets);

    expect(candidates).toEqual(
      expect.arrayContaining([
        'grid',
        'grid-cols-[200px_1fr]',
        'bg-[#00ab73]',
        'md:grid-cols-2',
        'hover:[&>span]:opacity-80'
      ])
    );
    expect(result.css).toContain('200px 1fr');
    expect(result.css).toContain('#00ab73');
    expect(result.css).toContain('@media');
    expect(result.css).toContain('span');
    expect(result.css).not.toContain('button,input');
    expect(result.css).not.toContain('@layer base');
    expect(result.css).not.toMatch(/\.ant-/u);
  });

  test('AC-002 ignores authored CSS class names while compiling valid Tailwind candidates', async () => {
    const source = [
      "import 'tailwindcss';",
      "const CSS = '.hero { color: red; }';",
      'export default () => <section className="hero mt-3" />;'
    ].join('\n');

    const result = await compileTailwindUtilities(
      extractStaticTailwindCandidates(source),
      stylesheets
    );

    expect(result.acceptedCandidates).toContain('mt-3');
    expect(result.acceptedCandidates).not.toContain('hero');
    expect(result.css).toContain('margin-top');
  });

  test('AC-003 is deterministic and excludes computed dynamic classes from frozen-source output', async () => {
    const source = [
      "import 'tailwindcss';",
      'export default ({ ctx }) => (',
      '  <div className={`bg-${ctx.input.color}`} data-mixed="custom-class" />',
      ');'
    ].join('\n');
    const candidates = extractStaticTailwindCandidates(source);
    const first = await compileTailwindUtilities(candidates, stylesheets);
    const second = await compileTailwindUtilities(
      [...candidates].reverse(),
      stylesheets
    );

    expect(first).toEqual(second);
    expect(first.css).not.toContain('.bg-red-500');
    expect(first.css).not.toContain('.custom-class');
    expect(first.css).not.toMatch(
      /(?:^|})\s*(?:\*|button|input|h[1-6])(?:,|{)/u
    );
  });

  test('AC-005 compiles the 68-class false-positive boundary from source without an inventory', async () => {
    const boundaryClasses = Array.from(
      { length: 68 },
      (_, index) => `mt-[${index + 1}px]`
    );
    const source = `import 'tailwindcss';\nexport default () => <div className="${boundaryClasses.join(' ')}" />;`;
    const result = await compileTailwindUtilities(
      extractStaticTailwindCandidates(source),
      stylesheets
    );

    expect(result.acceptedCandidates).toHaveLength(68);
    expect(result.css).toMatch(/margin-top:\s*1px/u);
    expect(result.css).toMatch(/margin-top:\s*68px/u);
  });
});
