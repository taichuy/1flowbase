import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { ApplicationTemplateCatalogList } from '../ApplicationTemplateCatalogList';

describe('template catalog loading announcements', () => {
  it('announces loading while preserving the HTML content model of native outputs', () => {
    render(
      <ApplicationTemplateCatalogList
        items={[]}
        total={0}
        loading
        failed={false}
        hasMore={false}
        canInstall={false}
        onLoadMore={async () => undefined}
        onInstall={vi.fn()}
      />
    );
    const status = screen.getByRole('status');
    expect(status).toBeVisible();
    expect(screen.getByRole('table')).toHaveAttribute('aria-busy', 'true');
    expect(status.querySelector('.ant-spin')).not.toBeNull();
    // HTML output permits phrasing content. AntD's real loading host is a div,
    // so a live region that owns it must preserve a flow-content container.
    const outputs = [
      ...(status.tagName === 'OUTPUT' ? [status] : []),
      ...status.querySelectorAll('output')
    ];
    for (const output of outputs) {
      expect(
        output.querySelector('div, p, section, article, table, ul, ol')
      ).toBeNull();
    }
  });
});
