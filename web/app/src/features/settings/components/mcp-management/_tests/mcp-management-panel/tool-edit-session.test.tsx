import type { ConsoleMcpTool } from '@1flowbase/api-client';
import { QueryClient } from '@tanstack/react-query';
import { act, screen, waitFor, within } from '@testing-library/react';
import { expect, test } from 'vitest';
import { i18nText } from '../../../../../../shared/i18n/text';
import {
  clickSegmentedOption,
  domEvents,
  mcpManagementApi as api,
  renderPanelWithMountedTool
} from './client-and-tool-editor.fixtures';

const toolId = 'search_customer';
const key = api.settingsMcpToolDetailQueryKey(toolId);

async function edit() {
  const row = await screen.findByRole('row', { name: /Search customer/ });
  await domEvents.click(within(row).getByRole('button', { name: '编辑' }));
  return screen.findByRole('dialog');
}

function button(dialog: HTMLElement, action: 'cancel' | 'save') {
  return within(dialog).getByRole('button', {
    name: action === 'cancel' ? i18nText('settings', 'auto.cancel') : i18nText('settings', 'auto.save')
  });
}

async function primeAndClose() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  await renderPanelWithMountedTool({ queryClient: client, fullDescription: '' });
  await domEvents.click(screen.getByRole('tab', { name: 'Tool 配置' }));
  const dialog = await edit();
  await within(dialog).findByDisplayValue('des-1');
  const first = client.getQueryData<ConsoleMcpTool>(key)!;
  await domEvents.click(button(dialog, 'cancel'));
  await act(() => client.invalidateQueries({ queryKey: key }));
  return { client, first };
}

function delayNextRead() {
  let resolve!: (detail: ConsoleMcpTool) => void;
  const promise = new Promise<ConsoleMcpTool>((done) => { resolve = done; });
  api.fetchSettingsMcpTool.mockReturnValueOnce(promise);
  return resolve;
}

test('reopening hydrates only the current response; later cache changes do not overwrite the outgoing draft', async () => {
  const { client, first } = await primeAndClose();
  const finish = delayNextRead();
  const dialog = await edit();
  await waitFor(() => expect(api.fetchSettingsMcpTool).toHaveBeenCalledTimes(2));
  expect(within(dialog).queryByDisplayValue('des-1')).not.toBeInTheDocument();
  expect(button(dialog, 'save')).toBeDisabled();

  const refreshed = { ...first, des_id: 'des-after-refresh', revision: first.revision + 1 };
  await act(async () => finish(refreshed));
  await within(dialog).findByDisplayValue(refreshed.des_id);
  await domEvents.change(within(dialog).getByLabelText('short_description'), {
    target: { value: 'Edited locally' }
  });
  act(() => { client.setQueryData(key, { ...refreshed, des_id: 'newer-in-cache' }); });
  await clickSegmentedOption(dialog, 'input_mapping');
  await domEvents.click(button(dialog, 'save'));
  await waitFor(() => expect(api.updateSettingsMcpTool).toHaveBeenCalledWith(
    toolId,
    expect.objectContaining({ des_id: refreshed.des_id, short_description: 'Edited locally' }),
    expect.any(String)
  ));
});

test('a failed refresh cannot unlock a previously cached record', async () => {
  await primeAndClose();
  api.fetchSettingsMcpTool.mockRejectedValueOnce(new Error('detail unavailable'));
  const dialog = await edit();
  expect(await within(dialog).findByRole('alert')).toHaveTextContent(i18nText('settings', 'auto.mcp_load_failed'));
  expect(button(dialog, 'save')).toBeDisabled();
  expect(within(dialog).queryByDisplayValue('des-1')).not.toBeInTheDocument();
  expect(api.updateSettingsMcpTool).not.toHaveBeenCalled();
  expect(api.createSettingsMcpTool).not.toHaveBeenCalled();
});

test('closing a pending edit retires its read even when the same tool is reopened', async () => {
  const { first } = await primeAndClose();
  const finishClosedRead = delayNextRead();
  await domEvents.click(button(await edit(), 'cancel'));
  const finishCurrentRead = delayNextRead();
  const dialog = await edit();
  await act(async () => finishClosedRead({ ...first, des_id: 'retired' }));
  expect(button(dialog, 'save')).toBeDisabled();
  expect(within(dialog).queryByDisplayValue('retired')).not.toBeInTheDocument();
  await act(async () => finishCurrentRead({ ...first, des_id: 'current' }));
  await within(dialog).findByDisplayValue('current');
  expect(button(dialog, 'save')).toBeEnabled();
});
