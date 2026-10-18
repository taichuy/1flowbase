import '../../../../../test/fixtures/rc-util-unique-ids';
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { App } from 'antd';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import copy from 'copy-to-clipboard';
import * as client from '@1flowbase/api-client';
import { FileTableView } from '../FileTableView';
import { resetAuthStore, useAuthStore } from '../../../../../state/auth-store';

vi.mock('copy-to-clipboard', () => ({ default: vi.fn(() => true) }));

vi.mock('@1flowbase/api-client', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@1flowbase/api-client')>()),
  fetchConsoleFileTables: vi.fn(),
  fetchConsoleFileRecords: vi.fn(),
  uploadConsoleFile: vi.fn(),
  updateConsoleRuntimeModelRecord: vi.fn(),
  deleteConsoleRuntimeModelRecord: vi.fn(),
  createConsoleFileShareLink: vi.fn()
}));
const table = {
  id: 'table-1',
  code: 'file_demo',
  title: '文件表 Demo',
  status: 'active',
  scope_kind: 'workspace',
  scope_id: 'workspace-1',
  model_definition_id: 'model-1',
  bound_storage_id: null,
  bound_storage_title: null,
  is_builtin: false,
  is_default: false
} as const;
const record = {
  id: 'record-1',
  url: '/api/console/files/table-1/records/record-1/content',
  preview_url: null,
  filename: 'hello.txt',
  title: 'hello.txt',
  mimetype: 'text/plain',
  size: 5,
  storage_id: 'storage-1',
  created_at: '2026-10-12T12:00:00Z'
};

function mount(
  queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } }
  })
) {
  return render(
    <QueryClientProvider client={queryClient}>
      <App>
        <FileTableView fileTableCode="file_demo" />
      </App>
    </QueryClientProvider>
  );
}
beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  useAuthStore.setState({
    csrfToken: 'csrf-123',
    actor: {
      id: 'user-1',
      account: 'root',
      effective_display_role: 'root',
      current_workspace_id: 'workspace-1'
    }
  });
  vi.mocked(client.fetchConsoleFileTables).mockResolvedValue([table]);
  vi.mocked(client.fetchConsoleFileRecords).mockResolvedValue({
    items: [],
    total: 0
  });
});
afterEach(() => {
  resetAuthStore();
});

describe('FileTableView', () => {
  test('searches and sorts on the server instead of filtering the current page', async () => {
    mount();
    fireEvent.change(screen.getByRole('textbox', { name: '搜索文件名…' }), {
      target: { value: 'report' }
    });
    await waitFor(() =>
      expect(client.fetchConsoleFileRecords).toHaveBeenLastCalledWith(
        'file_demo',
        1,
        5,
        expect.any(AbortSignal),
        { filename: 'report', sort: 'created_at:desc' }
      )
    );
    fireEvent.click(screen.getByRole('columnheader', { name: /大小/ }));
    await waitFor(() =>
      expect(client.fetchConsoleFileRecords).toHaveBeenLastCalledWith(
        'file_demo',
        1,
        5,
        expect.any(AbortSignal),
        { filename: 'report', sort: 'size:asc' }
      )
    );
  });
  test('edits the filename through the selected model and confirms deletion before sending it', async () => {
    vi.mocked(client.fetchConsoleFileRecords).mockResolvedValue({
      items: [record],
      total: 1
    });
    vi.mocked(client.updateConsoleRuntimeModelRecord).mockResolvedValue({
      ...record,
      filename: 'renamed.txt'
    });
    vi.mocked(client.deleteConsoleRuntimeModelRecord).mockResolvedValue({
      deleted: true
    });
    mount();
    fireEvent.click(await screen.findByRole('button', { name: /编辑/ }));
    const drawer = await screen.findByRole('dialog', { name: '编辑文件' });
    fireEvent.change(within(drawer).getByRole('textbox'), {
      target: { value: 'renamed.txt' }
    });
    fireEvent.click(within(drawer).getByRole('button', { name: '保存' }));
    await waitFor(() =>
      expect(client.updateConsoleRuntimeModelRecord).toHaveBeenCalledWith(
        'file_demo',
        record.id,
        { filename: 'renamed.txt' },
        'csrf-123'
      )
    );
    await waitFor(() =>
      expect(
        screen.queryByRole('dialog', { name: '编辑文件' })
      ).not.toBeInTheDocument()
    );
    fireEvent.click(screen.getByRole('button', { name: /删除/ }));
    const confirm = await screen.findByRole('dialog');
    expect(client.deleteConsoleRuntimeModelRecord).not.toHaveBeenCalled();
    fireEvent.click(within(confirm).getByRole('button', { name: /删除/ }));
    await waitFor(() =>
      expect(client.deleteConsoleRuntimeModelRecord).toHaveBeenCalledWith(
        'file_demo',
        record.id,
        'csrf-123'
      )
    );
  });

  test('reports failed URL copying instead of showing a success message', async () => {
    vi.mocked(client.fetchConsoleFileRecords).mockResolvedValue({
      items: [record],
      total: 1
    });
    vi.mocked(copy).mockResolvedValueOnce(false);
    mount();
    fireEvent.click(await screen.findByRole('button', { name: '复制链接' }));
    expect(await screen.findByText('复制失败，请手动复制链接')).toBeVisible();
    expect(screen.queryByText('链接已复制')).not.toBeInTheDocument();
  });

  test('keeps the editor and existing record when the server rejects a rename', async () => {
    vi.mocked(client.fetchConsoleFileRecords).mockResolvedValue({
      items: [record],
      total: 1
    });
    vi.mocked(client.updateConsoleRuntimeModelRecord).mockRejectedValue(
      new Error('No write access')
    );
    mount();
    fireEvent.click(await screen.findByRole('button', { name: /编辑/ }));
    const drawer = await screen.findByRole('dialog', { name: '编辑文件' });
    fireEvent.change(within(drawer).getByRole('textbox'), {
      target: { value: 'denied.txt' }
    });
    fireEvent.click(within(drawer).getByRole('button', { name: '保存' }));
    expect(await screen.findByText('No write access')).toBeVisible();
    expect(drawer).toBeVisible();
    expect(
      screen.getByRole('link', { name: record.filename })
    ).toBeInTheDocument();
    expect(client.fetchConsoleFileRecords).toHaveBeenCalledTimes(1);
  });

  test.each(['text/plain', 'application/pdf', 'application/zip'])(
    'shows the access URL in the preview cell when %s has no backend preview',
    async (mimetype) => {
      vi.mocked(client.fetchConsoleFileRecords).mockResolvedValue({
        items: [{ ...record, mimetype }],
        total: 1
      });
      mount();
      const filename = await screen.findByRole('link', {
        name: record.filename
      });
      const previewCell = within(filename.closest('tr')!).getAllByRole(
        'cell'
      )[0];
      expect(
        within(previewCell).getByRole('link', { name: record.url })
      ).toHaveAttribute('href', record.url);
      expect(within(previewCell).queryByRole('img')).not.toBeInTheDocument();
    }
  );

  test('falls back to the access URL when the preview image cannot load', async () => {
    vi.mocked(client.fetchConsoleFileRecords).mockResolvedValue({
      items: [{ ...record, preview_url: '/unavailable-image.png' }],
      total: 1
    });
    mount();
    const thumbnail = await screen.findByRole('img', { name: record.filename });
    const cell = thumbnail.closest('td')!;
    fireEvent.error(thumbnail);
    expect(
      await within(cell).findByRole('link', { name: record.url })
    ).toHaveAttribute('href', record.url);
    expect(within(cell).queryByRole('img')).not.toBeInTheDocument();
  });

  test('shows two-decimal binary size and exact bytes on hover', async () => {
    vi.mocked(client.fetchConsoleFileRecords).mockResolvedValue({
      items: [{ ...record, size: 1_667_307 }],
      total: 1
    });
    mount();
    const size = await screen.findByText('1.59 MiB');
    fireEvent.mouseEnter(size);
    expect(await screen.findByRole('tooltip')).toHaveTextContent('1,667,307 B');
  });

  test('renders backend preview URL lazily and opens image preview on click', async () => {
    const imageRecord = {
      ...record,
      id: 'image-record-2',
      url: '/api/console/files/table-1/records/image-record-2/content',
      filename: 'picture.png',
      mimetype: 'image/png',
      preview_url: '/different-preview-resource/image.png'
    };
    vi.mocked(client.fetchConsoleFileRecords).mockResolvedValue({
      items: [imageRecord, record],
      total: 2
    });
    mount();
    const thumbnail = await screen.findByRole('img', { name: 'picture.png' });
    expect(thumbnail).toHaveAttribute('src', imageRecord.preview_url);
    expect(thumbnail).toHaveAttribute('loading', 'lazy');
    expect(
      screen.queryByRole('img', { name: 'hello.txt' })
    ).not.toBeInTheDocument();
    fireEvent.click(thumbnail);
    await waitFor(() =>
      expect(document.querySelector('[role="dialog"]')).toBeInTheDocument()
    );
    expect(
      await screen.findByRole('link', { name: 'picture.png' })
    ).toHaveAttribute('href', imageRecord.url);
    expect(screen.getByRole('link', { name: 'hello.txt' })).toHaveAttribute('href', record.url);
  });
  test('selects the requested file table, uploads multipart through the host and refreshes records', async () => {
    vi.mocked(client.uploadConsoleFile).mockImplementation(async () => {
      vi.mocked(client.fetchConsoleFileRecords).mockResolvedValue({
        items: [record],
        total: 1
      });
      return { file_table_id: table.id, storage_id: record.storage_id, record };
    });
    mount();
    const button = await screen.findByRole('button', { name: /上传文件/ });
    await waitFor(() => expect(button).toBeEnabled());
    fireEvent.click(button);
    const dialog = await screen.findByRole('dialog', { name: '上传文件' });
    const input = dialog.querySelector('input[type="file"]')!;
    const file = new File(['hello'], 'hello.txt', { type: 'text/plain' });
    fireEvent.change(input, { target: { files: [file] } });
    await waitFor(() =>
      expect(client.uploadConsoleFile).toHaveBeenCalledWith(
        expect.any(File),
        table.id,
        'csrf-123',
        expect.any(AbortSignal)
      )
    );
    expect(
      await screen.findByRole('link', { name: 'hello.txt' })
    ).toHaveAttribute(
      'href',
      '/api/console/files/table-1/records/record-1/content'
    );
    expect(client.fetchConsoleFileRecords).toHaveBeenCalledWith(
      'file_demo',
      1,
      5,
      expect.any(AbortSignal),
      { filename: '', sort: 'created_at:desc' }
    );
  });
  test('does not enable upload or query records for a missing target table', async () => {
    vi.mocked(client.fetchConsoleFileTables).mockResolvedValue([]);
    mount();
    expect(
      await screen.findByText('文件表不可用，请检查文件表配置和访问权限。')
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /上传文件/ })).toBeDisabled();
    expect(client.fetchConsoleFileRecords).not.toHaveBeenCalled();
  });
  test('a rejected upload does not add a row or refetch records as a success', async () => {
    vi.mocked(client.uploadConsoleFile).mockRejectedValue(
      new Error('存储不可用')
    );
    mount();
    const button = await screen.findByRole('button', { name: /上传文件/ });
    await waitFor(() => expect(button).toBeEnabled());
    fireEvent.click(button);
    const dialog = await screen.findByRole('dialog', { name: '上传文件' });
    fireEvent.change(dialog.querySelector('input[type="file"]')!, {
      target: { files: [new File(['hello'], 'hello.txt')] }
    });
    expect(await screen.findByText('存储不可用')).toBeInTheDocument();
    expect(client.fetchConsoleFileRecords).toHaveBeenCalledTimes(1);
    expect(
      screen.queryByRole('link', { name: 'hello.txt' })
    ).not.toBeInTheDocument();
  });

  test('uploading from another page refreshes a still-fresh cached first page', async () => {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false, staleTime: Infinity } }
    });
    queryClient.setQueryData(['files', 'tables', 'workspace-1'], [table]);
    queryClient.setQueryData(
      [
        'files',
        'records',
        'workspace-1',
        table.id,
        1,
        5,
        '',
        'created_at:desc'
      ],
      {
        items: [record],
        total: 21
      }
    );
    const second = { ...record, id: 'record-2', filename: 'second.txt' };
    const newest = { ...record, id: 'record-3', filename: 'newest.txt' };
    let uploaded = false;
    vi.mocked(client.fetchConsoleFileRecords).mockImplementation(
      async (_, page) => ({
        items: page === 1 ? [uploaded ? newest : record] : [second],
        total: 21
      })
    );
    vi.mocked(client.uploadConsoleFile).mockImplementation(async () => {
      uploaded = true;
      return {
        file_table_id: table.id,
        storage_id: record.storage_id,
        record: newest
      };
    });
    mount(queryClient);
    fireEvent.click(await screen.findByTitle('2'));
    await screen.findByRole('link', { name: 'second.txt' });
    fireEvent.click(screen.getByRole('button', { name: /上传文件/ }));
    const dialog = await screen.findByRole('dialog', { name: '上传文件' });
    fireEvent.change(dialog.querySelector('input[type="file"]')!, {
      target: { files: [new File(['newest'], 'newest.txt')] }
    });
    expect(
      await screen.findByRole('link', { name: 'newest.txt' })
    ).toBeInTheDocument();
  });
  test.each([null, '/api/public/files/content?token=temporary&preview=true'])(
    'creates a clickable share link and copies the same URL (preview: %s)',
    async (previewUrl) => {
      const stored = { ...record, url: '/files/persisted/hello.txt' };
      vi.mocked(client.fetchConsoleFileRecords).mockResolvedValue({
        items: [stored],
        total: 1
      });
      vi.mocked(client.createConsoleFileShareLink).mockResolvedValue({
        url: '/api/public/files/content?token=temporary',
        preview_url: previewUrl,
        expires_at: '2026-10-13T12:00:00Z'
      });
      mount();
      expect(
        await screen.findByRole('link', { name: 'hello.txt' })
      ).toHaveAttribute('href', stored.url);
      fireEvent.click(await screen.findByRole('button', { name: '分享' }));
      await waitFor(() =>
        expect(
          screen.getByText('有效期内，任何持有链接的人均可访问此文件。')
        ).toBeVisible()
      );
      fireEvent.change(screen.getByRole('spinbutton'), {
        target: { value: '120' }
      });
      fireEvent.click(screen.getByRole('button', { name: '生成链接' }));
      const field = await screen.findByRole('link', { name: '分享链接' });
      const url = new URL(
        previewUrl ?? '/api/public/files/content?token=temporary',
        window.location.origin
      ).href;
      expect(field).toHaveAttribute('href', url);
      expect(field).toHaveAttribute('target', '_blank');
      expect(field).toHaveTextContent(url);
      expect(screen.queryByText('打开分享图片预览')).not.toBeInTheDocument();
      expect(client.createConsoleFileShareLink).toHaveBeenCalledWith(
        table.id,
        record.id,
        { expires_in_seconds: 120 },
        'csrf-123',
        expect.any(AbortSignal)
      );
      fireEvent.click(
        within(screen.getByRole('dialog', { name: '分享' })).getByRole(
          'button',
          { name: '复制链接' }
        )
      );
      expect(copy).toHaveBeenCalledWith(url);
      expect(screen.getByText(/到期时间：/)).toBeVisible();
      fireEvent.click(
        within(screen.getByRole('dialog', { name: '分享' })).getByRole(
          'button',
          { name: /取\s*消/ }
        )
      );
      await waitFor(() =>
        expect(
          screen.queryByRole('dialog', { name: '分享' })
        ).not.toBeInTheDocument()
      );
    }
  );

  test('closes a pending link request on workspace change and ignores its late result', async () => {
    vi.mocked(client.fetchConsoleFileRecords).mockResolvedValue({
      items: [record],
      total: 1
    });
    let resolve!: (value: client.FileShareLink) => void;
    vi.mocked(client.createConsoleFileShareLink).mockImplementation(
      () =>
        new Promise((done) => {
          resolve = done;
        })
    );
    mount();
    fireEvent.click(await screen.findByRole('button', { name: '分享' }));
    fireEvent.click(screen.getByRole('button', { name: '生成链接' }));
    await waitFor(() =>
      expect(client.createConsoleFileShareLink).toHaveBeenCalled()
    );
    const signal = vi.mocked(client.createConsoleFileShareLink).mock
      .calls[0][4]!;
    useAuthStore.setState({
      actor: {
        id: 'user-1',
        account: 'root',
        effective_display_role: 'root',
        current_workspace_id: 'workspace-2'
      }
    });
    await waitFor(() => expect(signal.aborted).toBe(true));
    resolve({
      url: '/api/public/files/content?token=stale',
      preview_url: null,
      expires_at: '2026-10-13T12:00:00Z'
    });
    await waitFor(() =>
      expect(
        screen.queryByRole('link', { name: '分享链接' })
      ).not.toBeInTheDocument()
    );
  });

  test('reports a failed link request without displaying a stale link', async () => {
    vi.mocked(client.fetchConsoleFileRecords).mockResolvedValue({
      items: [record],
      total: 1
    });
    vi.mocked(client.createConsoleFileShareLink).mockRejectedValue(
      new Error('file access denied')
    );
    mount();
    fireEvent.click(await screen.findByRole('button', { name: '分享' }));
    fireEvent.click(screen.getByRole('button', { name: '生成链接' }));
    await waitFor(() =>
      expect(screen.getByText('file access denied')).toBeVisible()
    );
    expect(
      screen.queryByRole('link', { name: '分享链接' })
    ).not.toBeInTheDocument();
  });
});
