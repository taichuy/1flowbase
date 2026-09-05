import { readFile } from 'node:fs/promises';
import path from 'node:path';

import { describe, expect, test } from 'vitest';

describe('application bootstrap loading state', () => {
  test('AC-003 keeps a native loading indicator before React and Ant Design mount', async () => {
    const html = await readFile(
      path.resolve(process.cwd(), 'index.html'),
      'utf8'
    );
    const bootstrap = new DOMParser().parseFromString(html, 'text/html');
    const status = bootstrap.querySelector<HTMLElement>(
      '[data-testid="application-bootstrap-shell"]'
    );

    expect(status?.getAttribute('role')).toBe('status');
    expect(status?.getAttribute('aria-label')).toBe('thinking');
    expect(status?.textContent).toContain('thinking');
    expect(
      status?.querySelector('.application-bootstrap-shell__spinner')
    ).not.toBeNull();
    expect(html).toContain('height: 48px');
    expect(html).toContain('width: 48px');
    expect(html).toContain(
      'application-bootstrap-shell-spin 1s linear infinite'
    );
  });
});
