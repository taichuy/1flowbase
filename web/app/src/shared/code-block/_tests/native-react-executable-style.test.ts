import { describe, expect, test } from 'vitest';

import { compileNativeReactExecutableStyle } from '../native-react-executable-style';

describe('Native React local Tailwind style compilation', () => {
  test('generates only the finite candidates used by one block', async () => {
    const result = await compileNativeReactExecutableStyle(
      `import 'tailwindcss'; export default () => <div className="p-4 bg-red-500 md:grid-cols-2" />;`
    );
    expect(result.candidates).toEqual(
      expect.arrayContaining(['p-4', 'bg-red-500', 'md:grid-cols-2'])
    );
    expect(result.utility_css).toContain('.p-4');
    expect(result.utility_css.length).toBeLessThan(20_000);
    expect(result.assets).toHaveLength(2);
  });

  test('uses a CSS identity independent from ordinary JavaScript edits', async () => {
    const first = await compileNativeReactExecutableStyle(
      `import 'tailwindcss'; const count = 1; export default () => <div className="p-4" />;`
    );
    const second = await compileNativeReactExecutableStyle(
      `import 'tailwindcss'; const count = 2; export default () => <div className="p-4" />;`
    );
    expect(second.candidate_identity).toBe(first.candidate_identity);
    expect(second.utility_css_sha256).toBe(first.utility_css_sha256);
  });

  test('does not create styles without the compile-time capability import', async () => {
    const result = await compileNativeReactExecutableStyle(
      `export default () => <div className="p-4" />;`
    );
    expect(result.assets).toEqual([]);
  });

  test('rejects an unbounded dynamic class before emitting incomplete CSS', async () => {
    await expect(
      compileNativeReactExecutableStyle(
        `import 'tailwindcss'; export default () => <div className={remoteClass} />;`
      )
    ).rejects.toThrow('finite set of local literals');
  });
});
