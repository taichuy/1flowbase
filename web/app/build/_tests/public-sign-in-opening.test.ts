import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { expect, test } from 'vitest';
import { publicSignInPlugin } from '../public-sign-in';

test('default password virtual module hashes canonical LF template bytes across OS checkouts', async () => {
  const plugin = publicSignInPlugin(process.cwd());
  const load =
    typeof plugin.load === 'function' ? plugin.load : plugin.load!.handler;
  const templatePath = path.resolve(
    process.cwd(),
    '../../api/crates/control-plane/src/auth/public_ui_blocks/password_local.tsx'
  );
  const expectedLfSha = createHash('sha256')
    .update(readFileSync(templatePath, 'utf8').replace(/\r\n/gu, '\n'))
    .digest('hex');
  const loaded = (await load.call(
    { addWatchFile: () => undefined, warn: () => undefined } as never,
    '\0virtual:1flowbase-default-password'
  )) as { code: string };
  expect(loaded.code).toContain(
    `export const sourceSha256 = ${JSON.stringify(expectedLfSha)};`
  );
});

test('the public document never substitutes a speculative sign-in frame for session discovery', () => {
  const plugin = publicSignInPlugin(process.cwd());
  expect(plugin.transformIndexHtml).toBeUndefined();
  const html = readFileSync('index.html', 'utf8');
  expect(html).toContain('application-bootstrap-shell');
  expect(html).not.toContain('auth-sign-in-opening-title');
});

test('anonymous translations are an exact projection, not a divergent catalog or MCP dependency', async () => {
  const plugin = publicSignInPlugin(process.cwd());
  const load =
    typeof plugin.load === 'function' ? plugin.load : plugin.load!.handler;
  const watched: string[] = [];
  const code = (await load.call(
    { addWatchFile: (file: string) => watched.push(file) } as never,
    '\0virtual:1flowbase-auth-translations'
  )) as string;
  expect(watched).toHaveLength(2);
  for (const locale of ['en_US', 'zh_Hans']) {
    const { sign_in, external_auth, auto } = JSON.parse(
      readFileSync(`src/features/auth/i18n/${locale}.json`, 'utf8')
    );
    expect(code).toContain(
      `export const ${locale} = ${JSON.stringify({ sign_in, external_auth, auto })};`
    );
  }
  expect(code).not.toContain('mcp_oauth');
});
