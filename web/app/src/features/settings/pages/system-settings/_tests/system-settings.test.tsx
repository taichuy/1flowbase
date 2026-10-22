import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, expect, test, vi } from 'vitest';
import { useQueryClient } from '@tanstack/react-query';
const { fetchSettings, saveSettings } = vi.hoisted(() => ({
  fetchSettings: vi.fn(),
  saveSettings: vi.fn()
}));
vi.mock('../../../api/system-branding', () => ({
  fetchConsoleSystemSettings: fetchSettings,
  saveConsoleSystemSettings: saveSettings,
  systemSettingsQueryKey: ['settings', 'system-branding']
}));
import { AppProviders } from '../../../../../app/AppProviders';
import { SystemSettingsPanel } from '../SystemSettingsPanel';
import { useAuthStore } from '../../../../../state/auth-store';
import { systemMetadataQueryKey } from '../../../../../shared/api/system-metadata';
const initial = {
  logo_url: '/icon.svg',
  system_name: '1flowbase',
  hide_login_tagline: false,
  system_description: '介绍',
  filing_html: ''
};
function PublicBrandObserver() {
  const queryClient = useQueryClient();
  return (
    <button
      onClick={() => {
        document.title =
          queryClient.getQueryData<typeof initial>(systemMetadataQueryKey)
            ?.system_name ?? '';
      }}
    >
      Read public cache
    </button>
  );
}
beforeEach(() => {
  fetchSettings.mockReset().mockResolvedValue(initial);
  saveSettings.mockReset().mockImplementation(async (value) => value);
  useAuthStore.setState({ csrfToken: 'csrf-test' });
});
test('saves the entire form, preserves HTML and publishes updated public metadata', async () => {
  render(
    <AppProviders>
      <SystemSettingsPanel />
      <PublicBrandObserver />
    </AppProviders>
  );
  await screen.findByDisplayValue('1flowbase');
  fireEvent.change(screen.getByLabelText('系统名称'), {
    target: { value: '新系统' }
  });
  expect(screen.queryByLabelText('登录页左侧标题')).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('switch', { name: '关闭登录左侧标语' }));
  const html =
    '<style>a{color:red}</style><a href="https://example.com" data-x="free">备案</a>';
  fireEvent.change(screen.getByLabelText('备案 HTML'), {
    target: { value: html }
  });
  fireEvent.click(screen.getByRole('button', { name: '保 存' }));
  await waitFor(() =>
    expect(saveSettings).toHaveBeenCalledWith(
      {
        ...initial,
        system_name: '新系统',
        hide_login_tagline: true,
        filing_html: html
      },
      'csrf-test'
    )
  );
  await screen.findByText('系统设置已保存');
  fireEvent.click(screen.getByText('Read public cache'));
  expect(document.title).toBe('新系统');
  fireEvent.change(screen.getByLabelText('系统名称'), {
    target: { value: '未保存' }
  });
  fireEvent.click(screen.getByRole('button', { name: '重 置' }));
  expect(screen.getByLabelText('系统名称')).toHaveValue('新系统');
  expect(
    screen.getByRole('switch', { name: '关闭登录左侧标语' })
  ).toBeChecked();
});
test('reset restores persisted values and a blank system name does not save', async () => {
  render(
    <AppProviders>
      <SystemSettingsPanel />
    </AppProviders>
  );
  await screen.findByDisplayValue('1flowbase');
  fireEvent.change(screen.getByLabelText('系统名称'), {
    target: { value: '草稿' }
  });
  fireEvent.click(screen.getByRole('button', { name: '重 置' }));
  expect(screen.getByLabelText('系统名称')).toHaveValue('1flowbase');
  fireEvent.change(screen.getByLabelText('系统名称'), {
    target: { value: '   ' }
  });
  fireEvent.click(screen.getByRole('button', { name: '保 存' }));
  await screen.findByText('请输入系统名称');
  expect(saveSettings).not.toHaveBeenCalled();
});
