import { readFileSync } from 'node:fs';
import { expect, test } from 'vitest';
import { publicSignInPlugin } from '../public-sign-in';

test('the public document never substitutes a speculative sign-in frame for session discovery', () => {
  const plugin = publicSignInPlugin(process.cwd());
  expect(plugin.transformIndexHtml).toBeUndefined();
  const html = readFileSync('index.html', 'utf8');
  expect(html).toContain('application-bootstrap-shell');
  expect(html).not.toContain('auth-sign-in-opening-title');
});
