import { AppProviders, ApplicationManagementPanel, applicationArchiveDownload, applicationManagementApi, applicationsApi, setupApplicationManagementFixture } from './fixture';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

describe('ApplicationManagementPanel', () => {
  setupApplicationManagementFixture();

  test('AC-001 AC-002 exports only selected applications as a ZIP archive', async () => {
    render(
      <AppProviders>
        <ApplicationManagementPanel />
      </AppProviders>
    );

    await screen.findByText('Daily Report');
    const exportButton = screen.getByRole('button', { name: /导出应用（0）/ });
    expect(exportButton).toBeDisabled();
    expect(screen.queryByText('导出筛选结果')).not.toBeInTheDocument();
    expect(screen.queryByText(/CSV/)).not.toBeInTheDocument();

    const rowCheckboxes = screen.getAllByRole('checkbox');
    fireEvent.click(rowCheckboxes[1]);
    const selectedExportButton = screen.getByRole('button', {
      name: /导出应用（1）/
    });
    fireEvent.click(selectedExportButton);

    await waitFor(() =>
      expect(applicationsApi.exportApplicationArchive).toHaveBeenCalledWith(
        ['app-workflow'],
        expect.any(Object)
      )
    );
    await waitFor(() =>
      expect(
        applicationArchiveDownload.downloadApplicationArchive
      ).toHaveBeenCalledWith(
        expect.objectContaining({ contentType: 'application/zip' })
      )
    );

    fireEvent.click(screen.getByRole('button', { name: /筛\s*选/ }));
    expect(
      screen.getByRole('button', { name: /导出应用（0）/ })
    ).toBeDisabled();

    await waitFor(() =>
      expect(screen.getAllByRole('checkbox')).toHaveLength(2)
    );
    fireEvent.click(screen.getAllByRole('checkbox')[1]);
    fireEvent.click(screen.getByTitle('2'));
    await waitFor(() => expect(window.location.search).toContain('page=2'));
    expect(
      screen.getByRole('button', { name: /导出应用（0）/ })
    ).toBeDisabled();

    await waitFor(() =>
      expect(screen.getAllByRole('checkbox')).toHaveLength(2)
    );
    fireEvent.click(screen.getAllByRole('checkbox')[1]);
    fireEvent.click(screen.getByRole('button', { name: /重\s*置/ }));
    expect(
      screen.getByRole('button', { name: /导出应用（0）/ })
    ).toBeDisabled();
  });

  test('batch import AC-001 AC-003 previews all four applications and reports per-entry results', async () => {
    applicationsApi.previewApplicationArchive.mockResolvedValue({
      applications: Array.from({ length: 4 }, (_, entry_index) => ({
        entry_index,
        preview: {
          application: {
            name: `Batch ${entry_index}`,
            application_type: entry_index === 1 ? 'agent_flow' : 'workflow',
            description: `Description ${entry_index}`
          },
          dependencies: [],
          unresolved_nodes: []
        }
      }))
    });
    applicationsApi.importApplicationArchive.mockResolvedValue({
      succeeded_count: 2,
      failed_count: 1,
      partial_count: 1,
      results: [
        {
          entry_index: 0,
          status: 'succeeded',
          result: {
            application: { id: 'imported-0', name: 'Renamed workflow' }
          }
        },
        { entry_index: 1, status: 'failed', code: 'extension_slug' },
        {
          entry_index: 2,
          status: 'succeeded',
          result: { application: { id: 'imported-2', name: 'Batch 2' } }
        },
        {
          entry_index: 3,
          status: 'partial',
          application_id: 'partial-3',
          code: 'application_archive_import_incomplete'
        }
      ]
    });
    render(
      <AppProviders>
        <ApplicationManagementPanel />
      </AppProviders>
    );
    await screen.findByText('Daily Report');
    const file = new File(['zip'], 'applications-4-items.zip', {
      type: 'application/zip'
    });
    fireEvent.change(screen.getByLabelText('导入'), {
      target: { files: [file] }
    });
    const dialog = await screen.findByRole('dialog');
    const names = within(dialog).getAllByRole('textbox');
    expect(names).toHaveLength(4);
    fireEvent.change(names[0], { target: { value: 'Renamed workflow' } });
    fireEvent.click(within(dialog).getByRole('button', { name: '导入应用' }));
    await waitFor(() =>
      expect(
        within(dialog).getByText('导入成功：2，失败：1，未完成：1')
      ).toBeInTheDocument()
    );
    expect(
      within(dialog).getByText('应用路径已被占用，无法导入。')
    ).toBeInTheDocument();
    expect(applicationsApi.importApplicationArchive).toHaveBeenCalledWith(
      file,
      {
        applications: [
          { entry_index: 0, name: 'Renamed workflow' },
          { entry_index: 1, name: 'Batch 1' },
          { entry_index: 2, name: 'Batch 2' },
          { entry_index: 3, name: 'Batch 3' }
        ]
      },
      'csrf-123'
    );
    expect(
      within(dialog).queryByRole('button', { name: '导入应用' })
    ).not.toBeInTheDocument();
    expect(window.location.pathname).toBe('/settings/applications');
  });

  test('import AC-001 AC-002 previews, renames and refreshes applications in place', async () => {
    render(
      <AppProviders>
        <ApplicationManagementPanel />
      </AppProviders>
    );
    await screen.findByText('Daily Report');
    const route = window.location.href;
    const importButton = screen.getByRole('button', { name: '导入应用' });
    const toolbar = importButton.parentElement!;
    expect(
      within(toolbar)
        .getAllByRole('button')
        .slice(0, 4)
        .map((button) => button.textContent?.replace(/\s/g, ''))
    ).toEqual(['新增', '删除（0）', '导入应用', '导出应用（0）']);
    const input = screen.getByLabelText('导入');
    const chooseFile = vi.spyOn(input, 'click');
    fireEvent.click(importButton);
    expect(chooseFile).toHaveBeenCalledOnce();
    expect(input).toHaveAttribute(
      'accept',
      'application/zip,.zip,application/json,.json'
    );
    const file = new File(['{}'], 'application.json', {
      type: 'application/json'
    });
    fireEvent.change(input, { target: { files: [file] } });
    const dialog = await screen.findByRole('dialog');
    expect(applicationsApi.previewApplicationArchive).toHaveBeenCalledWith(
      file,
      expect.any(Object)
    );
    expect(within(dialog).getByLabelText('应用名称')).toHaveValue(
      'Imported Report'
    );
    expect(
      within(dialog).getByText('Imported description')
    ).toBeInTheDocument();
    fireEvent.change(within(dialog).getByLabelText('应用名称'), {
      target: { value: '  Daily Imported Report  ' }
    });
    const current =
      await applicationManagementApi.fetchSettingsApplicationManagement.mock
        .results[0].value;
    applicationManagementApi.fetchSettingsApplicationManagement.mockResolvedValue(
      {
        ...current,
        items: [
          ...current.items,
          {
            ...current.items[0],
            id: 'app-imported',
            name: 'Daily Imported Report'
          }
        ],
        total: current.total + 1
      }
    );
    fireEvent.click(within(dialog).getByRole('button', { name: '导入应用' }));
    expect(
      await screen.findByText('Daily Imported Report')
    ).toBeInTheDocument();
    expect(applicationsApi.importApplicationArchive).toHaveBeenCalledWith(
      file,
      {
        applications: [{ entry_index: 0, name: 'Daily Imported Report' }]
      },
      'csrf-123'
    );
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    );
    expect(window.location.href).toBe(route);
  });

  test('import AC-003 reports preview failure and lets the same ZIP file be selected again', async () => {
    applicationsApi.previewApplicationArchive.mockRejectedValueOnce(
      new Error('invalid archive')
    );
    render(
      <AppProviders>
        <ApplicationManagementPanel />
      </AppProviders>
    );
    await screen.findByText('Daily Report');
    const file = new File(['zip'], 'application.zip', {
      type: 'application/zip'
    });
    const input = screen.getByLabelText('导入');
    fireEvent.change(input, { target: { files: [file] } });
    expect(await screen.findByText('应用压缩包预览失败')).toBeInTheDocument();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(applicationsApi.importApplicationArchive).not.toHaveBeenCalled();
    fireEvent.change(input, { target: { files: [file] } });
    const dialog = await screen.findByRole('dialog');
    fireEvent.click(within(dialog).getByRole('button', { name: /取\s*消/ }));
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    );
    expect(applicationsApi.importApplicationArchive).not.toHaveBeenCalled();
  });

  test('import AC-003 retains the preview on import failure and prevents duplicate submissions', async () => {
    let rejectImport!: (error: Error) => void;
    applicationsApi.importApplicationArchive.mockImplementationOnce(
      () =>
        new Promise((_, reject) => {
          rejectImport = reject;
        })
    );
    render(
      <AppProviders>
        <ApplicationManagementPanel />
      </AppProviders>
    );
    await screen.findByText('Daily Report');
    const file = new File(['{}'], 'application.json', {
      type: 'application/json'
    });
    fireEvent.change(screen.getByLabelText('导入'), {
      target: { files: [file] }
    });
    const dialog = await screen.findByRole('dialog');
    const name = within(dialog).getByLabelText('应用名称');
    fireEvent.change(name, { target: { value: '   ' } });
    expect(
      within(dialog).getByRole('button', { name: '导入应用' })
    ).toBeDisabled();
    fireEvent.change(name, { target: { value: 'Retry Report' } });
    const submit = within(dialog).getByRole('button', { name: '导入应用' });
    fireEvent.click(submit);
    await waitFor(() => expect(submit).toHaveClass('ant-btn-loading'));
    fireEvent.click(submit);
    expect(applicationsApi.importApplicationArchive).toHaveBeenCalledOnce();
    rejectImport(new Error('import failed'));
    expect(await screen.findByText('应用压缩包导入失败')).toBeInTheDocument();
    expect(name).toHaveValue('Retry Report');
    fireEvent.click(submit);
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    );
    expect(await screen.findByText('应用已导入')).toBeInTheDocument();
  });

});
