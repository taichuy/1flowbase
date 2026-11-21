import { readFileSync } from 'node:fs';
import { expect, test } from 'vitest';
import { motionDocument } from '../sign-in-motion-document';

test('only the login document gets a self-starting animation before deferred runtime hints; no speculative form or visible tagline', async () => {
  const original = readFileSync('index.html', 'utf8').replace(
    '</head>',
    '<link rel="stylesheet" crossorigin href="/assets/route.css"></head>'
  );
  const page = await motionDocument(original, process.cwd(), [
    '<link rel="modulepreload" href="/deferred.js">'
  ]);
  expect(page.split('<body>')[0]).not.toContain('href="/assets/route.css"');
  expect(page).not.toContain('<!--#');
  const opening = page.slice(
    page.indexOf('<body>'),
    page.indexOf('<div id="root">')
  );
  expect(original).not.toContain('sign-in-document-scene');
  expect(opening).toContain('data-hero-artwork');
  expect(opening).toMatch(/data-opening-brand="" hidden/);
  expect(opening).toContain('<canvas');
  expect(opening).toContain('class="auth-sign-in-footer"');
  expect(opening).toContain('Powered by 1flowbase');
  expect(opening).toContain('loading-state--compact');
  expect(opening).toContain('role="status"');
  expect(opening).not.toMatch(/<(?:form|input|button)\b/i);
  const motion = opening.match(/<script>([\s\S]+?)<\/script>/)?.[1] ?? '';
  expect(motion).toContain('requestAnimationFrame');
  expect(motion).toContain('mousemove');
  expect(motion).toContain('/api/public/system-metadata');
  expect(motion).not.toMatch(/\bimport\s*\(/);
  expect(motion).toContain('/api/console/session');
  expect(motion).toContain('/api/public/auth/login-entries');
  expect(page).not.toContain('data-css-hash');
  expect(page).not.toContain('application-bootstrap-shell');
  expect(page).toContain('application-bootstrap-failure');
  expect(original).toContain('application-bootstrap-shell');
  expect(Buffer.byteLength(page)).toBeLessThanOrEqual(20000);
  expect(opening).toMatch(
    /<template id="sign-in-next-resources">[\s\S]*?href="\/deferred.js"[\s\S]*?<\/template>/
  );
});
