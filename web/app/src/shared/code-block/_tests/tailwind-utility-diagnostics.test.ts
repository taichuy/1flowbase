import { describe, expect, test } from 'vitest';

import { diagnoseUnsupportedTailwindUtilities } from '../tailwind-utility-diagnostics';

describe('Tailwind utility diagnostics', () => {
  test('requires an explicit finite class set for dynamic expressions', () => {
    const diagnostics = diagnoseUnsupportedTailwindUtilities(
      "import 'tailwindcss'; export default () => <div className={remoteClass} />;"
    );

    expect(diagnostics).toEqual([
      expect.objectContaining({
        code: 'transform_failed',
        path: 'tailwind.className'
      })
    ]);
  });

  test('accepts finite conditional and template candidates', () => {
    expect(
      diagnoseUnsupportedTailwindUtilities(`
        import 'tailwindcss';
        const color = active ? 'red' : 'blue';
        export default () => <div className={\`bg-\${color}-500\`} />;
      `)
    ).toEqual([]);
  });
});
