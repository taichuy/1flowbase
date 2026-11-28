import { act, screen, waitFor, within } from '@testing-library/react';
import { Modal } from 'antd';
import { expect, test, vi } from 'vitest';
import {
  click,
  extensionsApi,
  installedEntry,
  mcpManagementApi,
  renderSection,
  setupExtensionCenterFixture
} from './fixture';

setupExtensionCenterFixture();

test('confirming a version switch invalidates previously cached instance and tool pages', async () => {
  extensionsApi.switchSettingsPluginFamilyVersion.mockResolvedValue(
    installedEntry
  );
  const { queryClient } = await renderSection();
  const pages = [
    mcpManagementApi.settingsMcpInstancesQueryKey,
    mcpManagementApi.settingsMcpToolsQueryKey
  ].map((key) => [...key, 'page', 'cached']);
  for (const page of pages) queryClient.setQueryData(page, []);
  const row = await screen.findByRole('row', { name: /openai/ });
  await click(within(row).getByRole('button', { name: '查看' }));
  const drawer = await screen.findByRole('dialog');
  await click(
    within(drawer).getAllByRole('button', { name: '切换版本并重启插件' })[1]
  );
  for (const page of pages)
    expect(queryClient.getQueryState(page)?.isInvalidated).toBe(false);
  await act(async () => {
    await vi.mocked(Modal.confirm).mock.calls.at(-1)?.[0].onOk?.();
  });
  await waitFor(() => {
    for (const page of pages)
      expect(queryClient.getQueryState(page)?.isInvalidated).toBe(true);
  });
});
