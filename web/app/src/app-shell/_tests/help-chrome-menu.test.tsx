import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

const { fetchConsoleReleaseStatus } = vi.hoisted(() => ({
  fetchConsoleReleaseStatus: vi.fn()
}));

vi.mock('@1flowbase/api-client', () => ({
  fetchConsoleReleaseStatus
}));

import { AppProviders } from '../../app/AppProviders';
import { appI18n } from '../../shared/i18n/app-i18n';
import { HelpChromeMenu } from '../HelpChromeMenu';

describe('HelpChromeMenu', () => {
  test('opens the project, documentation, and license links from the help popup', async () => {
    await appI18n.changeLanguage('zh_Hans');
    fetchConsoleReleaseStatus.mockResolvedValue({
      current_version: '0.2.6',
      latest_version: null,
      has_update: false,
      release_info: null
    });

    render(
      <AppProviders>
        <HelpChromeMenu />
      </AppProviders>
    );

    fireEvent.mouseEnter(screen.getByLabelText('帮助'));

    const githubLink = (await screen.findByText('github')).closest('a');
    expect(githubLink).toHaveAttribute(
      'href',
      'https://github.com/taichuy/1flowbase'
    );
    expect(screen.getByRole('link', { name: /文档/u })).toHaveAttribute(
      'href',
      'https://1flowbase.taichuy.com/docs/'
    );
    expect(screen.getByRole('link', { name: /许可协议/u })).toHaveAttribute(
      'href',
      'https://1flowbase.taichuy.com/license/'
    );
    expect(screen.getByText('v0.2.6')).toBeInTheDocument();
  });

  test('shows compact API version status in the help popup', async () => {
    await appI18n.changeLanguage('zh_Hans');
    fetchConsoleReleaseStatus.mockResolvedValue({
      current_version: '0.1.6',
      latest_version: '0.1.7',
      has_update: true,
      release_info: {
        name: 'v0.1.7',
        body: 'Release notes',
        published_at: '2026-06-05T00:00:00Z',
        html_url: 'https://github.com/taichuy/1flowbase/releases/tag/v0.1.7'
      },
      contributors_url:
        'https://github.com/taichuy/1flowbase/graphs/contributors',
      upgrade_commands: {
        shell:
          'curl -fsSL https://raw.githubusercontent.com/taichuy/1flowbase/main/scripts/shell/docker-deploy.sh | sh',
        powershell:
          'irm https://raw.githubusercontent.com/taichuy/1flowbase/main/scripts/powershell/docker-deploy.ps1 | iex'
      },
      cached: false,
      warning: null
    });

    render(
      <AppProviders>
        <HelpChromeMenu />
      </AppProviders>
    );

    fireEvent.mouseEnter(screen.getByLabelText('帮助'));

    expect(await screen.findByText('v0.1.6')).toBeInTheDocument();
    const latestReleaseLink = await screen.findByRole('link', {
      name: /v0\.1\.7/u
    });
    expect(latestReleaseLink).toHaveAttribute(
      'href',
      'https://github.com/taichuy/1flowbase/releases/tag/v0.1.7'
    );
    expect(screen.queryByText('v0.1.7最新')).not.toBeInTheDocument();
    expect(screen.queryByText('版本')).not.toBeInTheDocument();
    expect(screen.queryByText('当前版本')).not.toBeInTheDocument();
    expect(screen.queryByText('最新版本')).not.toBeInTheDocument();
    expect(screen.queryByText('贡献者')).not.toBeInTheDocument();
    expect(screen.queryByText('Docker 一键升级')).not.toBeInTheDocument();
  });
});
