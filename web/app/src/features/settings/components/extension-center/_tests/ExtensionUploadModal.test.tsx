import {
  act,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import { beforeEach, expect, test, vi } from 'vitest';
import { App } from 'antd';
import { ApiClientError } from '@1flowbase/api-client';
import { AppI18nProvider } from '../../../../../app/AppI18nProvider';
import { ExtensionUploadModal } from '../ExtensionUploadModal';

const api = vi.hoisted(() => ({
  uploadSettingsExtension: vi.fn(),
  getSettingsExtensionRiskChallenge: vi.fn()
}));
vi.mock('../../../api/extensions/extensions', () => api);
beforeEach(() => {
  vi.clearAllMocks();
  api.getSettingsExtensionRiskChallenge.mockReturnValue(null);
});

async function mount() {
  const onClose = vi.fn(),
    onInstalled = vi.fn().mockResolvedValue(undefined);
  render(
    <AppI18nProvider>
      <App>
        <ExtensionUploadModal
          csrfToken="csrf"
          onClose={onClose}
          onInstalled={onInstalled}
        />
      </App>
    </AppI18nProvider>
  );
  const input = document.querySelector('input[type=file]') as HTMLInputElement;
  const file = new File(['data'], 'ssh.1flowbasepkg');
  fireEvent.change(input, { target: { files: [file] } });
  await screen.findByText(file.name);
  return { onClose, onInstalled, file };
}

test('shows bytes/percentage, waits for install response and only then refreshes', async () => {
  let resolve!: (value: unknown) => void;
  api.uploadSettingsExtension.mockImplementation(
    () =>
      new Promise((done) => {
        resolve = done;
      })
  );
  const { file, onInstalled } = await mount();
  fireEvent.click(screen.getByRole('button', { name: '开始上传' }));
  await waitFor(() => expect(api.uploadSettingsExtension).toHaveBeenCalled());
  const args = api.uploadSettingsExtension.mock.calls[0];
  expect(args.slice(0, 4)).toEqual([file, {}, 'csrf', {}]);
  act(() => args[4]({ loaded: 50, total: 100 }));
  expect(screen.getByText('50%')).toBeInTheDocument();
  expect(screen.getByText('正在上传文件')).toBeInTheDocument();
  act(() => args[4]({ loaded: 100, total: 100, complete: true }));
  expect(screen.getByText('文件已上传，正在校验和安装')).toBeInTheDocument();
  expect(onInstalled).not.toHaveBeenCalled();
  expect(screen.getByRole('button', { name: /取\s*消/ })).toBeDisabled();
  await act(async () => resolve({ installation: { id: 'installed' } }));
  await waitFor(() => expect(onInstalled).toHaveBeenCalledOnce());
  expect(screen.getAllByText('扩展包已安装').length).toBeGreaterThan(0);
});

test('backend risk requires a second explicit action and transmits exact overrides', async () => {
  const challenge = {
    warnings: [
      {
        code: 'signature_missing',
        message: 'Missing signature',
        overridable: true
      }
    ],
    compatibility: null
  };
  api.uploadSettingsExtension
    .mockRejectedValueOnce(new Error('risk'))
    .mockResolvedValueOnce({});
  api.getSettingsExtensionRiskChallenge.mockReturnValue(challenge);
  const { onInstalled } = await mount();
  fireEvent.click(screen.getByRole('button', { name: '开始上传' }));
  await screen.findByText('该扩展包没有可验证的发布者签名。');
  expect(api.uploadSettingsExtension).toHaveBeenCalledOnce();
  expect(onInstalled).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: '确认风险并安装' }));
  await waitFor(() => expect(onInstalled).toHaveBeenCalledOnce());
  expect(api.uploadSettingsExtension.mock.calls[1][3]).toEqual({
    risk_override: {
      reason: 'user_confirmed',
      acknowledged_warnings: ['signature_missing']
    }
  });
});

test('non-overridable warning blocks retries and cancelling does not upload again', async () => {
  api.uploadSettingsExtension.mockRejectedValue(new Error('risk'));
  api.getSettingsExtensionRiskChallenge.mockReturnValue({
    warnings: [
      {
        code: 'signature_invalid',
        message: 'Invalid signature',
        overridable: false
      }
    ],
    compatibility: null
  });
  const { onClose } = await mount();
  fireEvent.click(screen.getByRole('button', { name: '开始上传' }));
  await screen.findByText('Invalid signature');
  expect(screen.getByRole('button', { name: '确认风险并安装' })).toBeDisabled();
  fireEvent.click(screen.getByRole('button', { name: /取\s*消/ }));
  expect(onClose).toHaveBeenCalledOnce();
  expect(api.uploadSettingsExtension).toHaveBeenCalledOnce();
});

test('failure permits explicit retry without reporting success', async () => {
  api.uploadSettingsExtension
    .mockRejectedValueOnce(new Error('Disconnected'))
    .mockResolvedValueOnce({});
  const { onInstalled } = await mount();
  fireEvent.click(screen.getByRole('button', { name: '开始上传' }));
  await screen.findByText('上传或安装失败');
  expect(onInstalled).not.toHaveBeenCalled();
  // Ant Design's loading indicator exits after the failure status is rendered.
  const retry = await screen.findByRole('button', { name: '开始上传' });
  await waitFor(() => expect(retry).toBeEnabled());
  fireEvent.click(retry);
  await waitFor(() => expect(onInstalled).toHaveBeenCalledOnce());
});

test('uploads the file list sequentially and never reuploads successful rows', async () => {
  let firstDone!: (value: unknown) => void;
  api.uploadSettingsExtension
    .mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          firstDone = resolve;
        })
    )
    .mockResolvedValue({});
  const { onInstalled } = await mount();
  const second = new File(['second'], 'second.1flowbasepkg');
  fireEvent.change(document.querySelector('input[type=file]')!, {
    target: { files: [second] }
  });
  await screen.findByText(second.name);
  fireEvent.click(screen.getByRole('button', { name: '开始上传' }));
  await waitFor(() =>
    expect(api.uploadSettingsExtension).toHaveBeenCalledOnce()
  );
  expect(screen.getByRole('button', { name: '清空列表' })).toBeDisabled();
  expect(
    screen
      .getAllByRole('button', { name: /移\s*除/ })
      .every((button) => (button as HTMLButtonElement).disabled)
  ).toBe(true);
  await act(async () => firstDone({}));
  await waitFor(() => expect(onInstalled).toHaveBeenCalledOnce());
  expect(
    api.uploadSettingsExtension.mock.calls.map((args) => args[0].name)
  ).toEqual(['ssh.1flowbasepkg', 'second.1flowbasepkg']);
  const third = new File(['third'], 'third.1flowbasepkg');
  fireEvent.change(document.querySelector('input[type=file]')!, {
    target: { files: [third] }
  });
  await screen.findByText(third.name);
  fireEvent.click(screen.getByRole('button', { name: '开始上传' }));
  await waitFor(() => expect(onInstalled).toHaveBeenCalledTimes(2));
  expect(
    api.uploadSettingsExtension.mock.calls.map((args) => args[0].name)
  ).toEqual(['ssh.1flowbasepkg', 'second.1flowbasepkg', 'third.1flowbasepkg']);
});

test('keeps the backend installation failure reason visible', async () => {
  api.uploadSettingsExtension.mockRejectedValue(
    new ApiClientError({
      status: 400,
      code: 'plugin_runtime_target_mismatch',
      message:
        'plugin runtime target linux-arm64 is incompatible with host linux-amd64'
    })
  );
  const { onInstalled } = await mount();
  fireEvent.click(screen.getByRole('button', { name: '开始上传' }));
  expect(
    await screen.findByText(
      'plugin runtime target linux-arm64 is incompatible with host linux-amd64'
    )
  ).toBeInTheDocument();
  expect(onInstalled).not.toHaveBeenCalled();
});
