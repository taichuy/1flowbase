import { readFile } from 'node:fs/promises';
import path from 'node:path';

import { describe, expect, test } from 'vitest';

describe('BGP public auth runtime boundary', () => {
  test('uses the shared lightweight theme contract', async () => {
    const source = await readFile(
      path.resolve(
        process.cwd(),
        'src/features/auth/components/PublicAuthProviders.tsx'
      ),
      'utf8'
    );
    expect(source).toContain('@1flowbase/ui/app-theme-provider');
    expect(source).toContain('<AppThemeProvider>');
    expect(source).not.toContain('AppProviders');
  });

  test('exposes compile, mount, ready and typed failure phases', async () => {
    const source = await readFile(
      path.resolve(
        process.cwd(),
        'src/features/auth/components/PublicAuthBlock.tsx'
      ),
      'utf8'
    );
    for (const phase of [
      'discovering',
      'compiling',
      'mounting',
      'ready',
      'failed'
    ]) {
      expect(source).toContain(`'${phase}'`);
    }
    expect(source).toContain('data-public-auth-phase');
    expect(source).toContain('data-public-auth-diagnostic');
    expect(source).toContain('<BuiltinPasswordSignIn');
  });
});
