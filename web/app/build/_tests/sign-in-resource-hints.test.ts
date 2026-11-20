import { expect, test } from 'vitest';
import { signInPreloads } from '../sign-in-resource-hints';
test('prepares shared files once, including cycles, without traversing unrelated routes', () => {
  const chunks = new Map([
    ['page', { imports: ['shared', 'hero'] }],
    ['main', { imports: ['shared'] }],
    ['shared', { imports: ['page'] }],
    ['hero', { imports: [] }],
    ['form', { imports: [] }],
    ['mcp', { imports: [] }]
  ]);
  expect(signInPreloads(chunks, ['page', 'main'])).toEqual([
    'page',
    'shared',
    'hero',
    'main'
  ]);
});

test('discovers original visual CSS once without activating styles from lazy routes', async () => {
  const { signInStylePreloads } = await import('../sign-in-resource-hints');
  const chunks = new Map([
    [
      'page',
      {
        imports: ['theme'],
        viteMetadata: { importedCss: new Set(['page.css']) }
      }
    ],
    [
      'theme',
      {
        imports: ['page'],
        viteMetadata: { importedCss: new Set(['page.css', 'theme.css']) }
      }
    ],
    [
      'mcp',
      { imports: [], viteMetadata: { importedCss: new Set(['mcp.css']) } }
    ]
  ]);
  expect(signInStylePreloads(chunks, signInPreloads(chunks, ['page']))).toEqual(
    ['page.css', 'theme.css']
  );
});
