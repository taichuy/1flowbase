import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { App } from 'antd';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

const api = vi.hoisted(() => ({
  getSystemTemplateCatalog: vi.fn(),
  exportSystemTemplateArchive: vi.fn(),
  previewSystemTemplate: vi.fn(),
  installSystemTemplate: vi.fn()
}));
vi.mock('@1flowbase/api-client', () => api);
import { appI18n } from '../../../../../shared/i18n/app-i18n';
import { useAuthStore } from '../../../../../state/auth-store';
import { useState } from 'react';
import { SystemTemplateDialog } from '../SystemTemplateDialog';

const body = {
  schema_version: '1flowbase.portable-template/v1',
  pages: [],
  applications: [],
  data_models: [],
  plugins: []
};
function Harness() {
  const [dialog, setDialog] = useState<{
    mode: 'import' | 'export';
    file?: File;
    key: number;
  }>();
  return (
    <>
      <button onClick={() => setDialog({ mode: 'export', key: Date.now() })}>
        Export template
      </button>
      <input
        type="file"
        onChange={(event) =>
          setDialog({
            mode: 'import',
            file: event.target.files?.[0],
            key: Date.now()
          })
        }
      />
      {dialog && (
        <SystemTemplateDialog
          key={dialog.key}
          mode={dialog.mode}
          file={dialog.file}
          onClose={() => setDialog(undefined)}
        />
      )}
    </>
  );
}
function setup() {
  return render(
    <App>
      <QueryClientProvider
        client={
          new QueryClient({
            defaultOptions: {
              queries: { retry: false },
              mutations: { retry: false }
            }
          })
        }
      >
        <Harness />
      </QueryClientProvider>
    </App>
  );
}
function upload(container: HTMLElement, contents = JSON.stringify(body)) {
  const file = new File([contents], 'template.json', {
    type: 'application/json'
  });
  Object.defineProperty(file, 'text', {
    value: () => Promise.resolve(contents)
  });
  fireEvent.change(container.querySelector('input[type="file"]')!, {
    target: { files: [file] }
  });
}
describe('portable template flow', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });
  beforeEach(async () => {
    Object.values(api).forEach((mock) => mock.mockReset());
    await appI18n.changeLanguage('en_US');
    useAuthStore.setState({ csrfToken: 'csrf-token' });
    api.getSystemTemplateCatalog.mockResolvedValue({
      pages: [],
      applications: [],
      data_models: [],
      i18n_entries: [],
      mcp_instances: []
    });
    api.previewSystemTemplate.mockResolvedValue({
      valid: true,
      counts: {
        pages: 1,
        applications: 1,
        data_models: 2,
        mcp_instances: 0,
        i18n_entries: 0
      },
      failures: [],
      warnings: [],
      dependencies: [],
      effects: [],
      mcp_shared_tool_impacts: []
    });
  });
  test('export starts with no selected objects', async () => {
    setup();
    fireEvent.click(screen.getByRole('button', { name: 'Export template' }));
    expect(
      await screen.findByRole('button', { name: 'Download ZIP' })
    ).toBeDisabled();
    expect(api.exportSystemTemplateArchive).not.toHaveBeenCalled();
  });
  test('MCP instance alone can be selected for export', async () => {
    api.getSystemTemplateCatalog.mockResolvedValue({
      pages: [],
      applications: [],
      data_models: [],
      i18n_entries: [],
      mcp_instances: [{ id: 'agent-tools', name: 'Agent tools' }]
    });
    setup();
    fireEvent.click(screen.getByRole('button', { name: 'Export template' }));
    const selector = await screen.findByRole('combobox', {
      name: 'MCP instances'
    });
    fireEvent.mouseDown(selector);
    fireEvent.click(await screen.findByText('Agent tools'));
    expect(screen.getByRole('button', { name: 'Download ZIP' })).toBeEnabled();
  });
  test('exports explicitly selected translation keys without selecting other objects', async () => {
    api.getSystemTemplateCatalog.mockResolvedValue({
      pages: [],
      applications: [],
      data_models: [],
      mcp_instances: [],
      i18n_entries: [{ key: 'gateway.title' }, { key: 'gateway.description' }]
    });
    // A rejected download keeps this test focused on the outgoing selection.
    api.exportSystemTemplateArchive.mockRejectedValue(new Error('offline'));
    setup();
    fireEvent.click(screen.getByRole('button', { name: 'Export template' }));
    fireEvent.mouseDown(
      await screen.findByRole('combobox', { name: 'Translations' })
    );
    fireEvent.click(await screen.findByText('gateway.title'));
    fireEvent.click(screen.getByRole('button', { name: 'Download ZIP' }));
    await waitFor(() =>
      expect(api.exportSystemTemplateArchive).toHaveBeenCalledWith(
        {
          page_ids: [],
          application_ids: [],
          data_model_ids: [],
          mcp_instance_ids: [],
          i18n_keys: ['gateway.title']
        },
        'csrf-token'
      )
    );
  });
  test('preview explains preserved resources and install reports skipped resources without blocking eligible changes', async () => {
    const skipped = [
      {
        kind: 'page',
        source_id: 'customized-page',
        target_id: 'customized-page',
        reason: 'user_modified'
      },
      {
        kind: 'application',
        source_id: 'legacy-app',
        target_id: 'legacy-app',
        reason: 'unknown_baseline'
      },
      {
        kind: 'data_model',
        source_id: 'deleted-model',
        target_id: null,
        reason: 'user_deleted'
      },
      {
        kind: 'page',
        source_id: 'pending-page',
        target_id: 'pending-page',
        reason: 'pending_write'
      }
    ];
    api.previewSystemTemplate.mockResolvedValue({
      valid: true,
      counts: {
        pages: 2,
        applications: 1,
        data_models: 1,
        mcp_instances: 0,
        i18n_entries: 2
      },
      failures: [],
      warnings: [],
      dependencies: [],
      mcp_shared_tool_impacts: [],
      effects: [
        ...skipped.map((item) => ({ ...item, action: 'skip' })),
        {
          kind: 'page',
          source_id: 'new-page',
          target_id: null,
          action: 'create'
        }
      ]
    });
    api.installSystemTemplate.mockResolvedValue({
      complete: true,
      created: [{ kind: 'page', source_id: 'new-page', target_id: 'new-page' }],
      updated: [],
      skipped,
      id_map: {},
      failures: []
    });
    const { container } = setup();
    upload(container);
    expect(
      await screen.findByText('Local changes are preserved.')
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        'No template baseline is available; existing content is preserved.'
      )
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        'This resource was deleted locally and will not be recreated.'
      )
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        'A previous write has an unconfirmed outcome. This resource was skipped; check its state before retrying.'
      )
    ).toBeInTheDocument();
    expect(screen.queryByText('pending_write')).not.toBeInTheDocument();
    expect(screen.getAllByText('Skip and preserve')).toHaveLength(4);
    expect(screen.queryByText('user_modified')).not.toBeInTheDocument();
    const install = screen.getByRole('button', { name: 'Install template' });
    expect(install).toBeEnabled();
    fireEvent.click(install);
    expect(await screen.findByText('Template installed')).toBeInTheDocument();
    expect(
      screen.getByText(
        'Resources preserved and skipped: 4. Other eligible resources were processed.'
      )
    ).toBeInTheDocument();
    expect(screen.getAllByText('Local changes are preserved.')).toHaveLength(2);
    expect(screen.getAllByText('new-page').length).toBeGreaterThan(1);
    expect(api.installSystemTemplate).toHaveBeenCalledWith(body, 'csrf-token');
  });
  test('imports a v2 JSON package without dropping translations or changing the version', async () => {
    const translatedBody = {
      ...body,
      schema_version: '1flowbase.portable-template/v2',
      i18n_entries: [
        { key: 'gateway.title', locale: 'en_US', translation: 'Gateway' },
        { key: 'gateway.title', locale: 'zh_Hans', translation: '网关' }
      ]
    };
    api.installSystemTemplate.mockResolvedValue({
      complete: true,
      created: [],
      updated: [],
      skipped: [],
      id_map: {},
      failures: []
    });
    const { container } = setup();
    upload(container, JSON.stringify(translatedBody));
    await waitFor(() =>
      expect(api.previewSystemTemplate).toHaveBeenCalledWith(
        translatedBody,
        'csrf-token'
      )
    );
    const install = await screen.findByRole('button', {
      name: 'Install template'
    });
    await waitFor(() => expect(install).toBeEnabled());
    fireEvent.click(install);
    await waitFor(() =>
      expect(api.installSystemTemplate).toHaveBeenCalledWith(
        translatedBody,
        'csrf-token'
      )
    );
  });
  test('server rejection prevents installation and replacing the file clears stale preview', async () => {
    api.previewSystemTemplate.mockResolvedValue({
      valid: false,
      counts: {
        pages: 0,
        applications: 0,
        data_models: 0,
        mcp_instances: 0,
        i18n_entries: 0
      },
      failures: ['Route already exists'],
      warnings: [],
      dependencies: [],
      effects: [],
      mcp_shared_tool_impacts: []
    });
    const { container } = setup();
    upload(container);
    expect(await screen.findByText('Route already exists')).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'Install template' })
    ).toBeDisabled();
    upload(container, 'not json');
    expect(
      await screen.findByText(
        'Could not read this template file. Choose a ZIP archive or a legacy JSON file.'
      )
    ).toBeInTheDocument();
    expect(screen.queryByText('Route already exists')).not.toBeInTheDocument();
    expect(api.installSystemTemplate).not.toHaveBeenCalled();
  });
  test('installs the complete previewed package and preserves honest partial outcome', async () => {
    api.installSystemTemplate.mockResolvedValue({
      complete: false,
      updated: [],
      created: [
        {
          kind: 'data_model',
          source_id: 'source-model',
          target_id: 'target-model'
        }
      ],
      skipped: [],
      id_map: { 'source-model': 'target-model' },
      failures: ['Application publish failed']
    });
    const { container } = setup();
    upload(container);
    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: 'Install template' })
      ).toBeEnabled()
    );
    expect(api.previewSystemTemplate).toHaveBeenCalledWith(body, 'csrf-token');
    fireEvent.click(screen.getByRole('button', { name: 'Install template' }));
    expect(
      await screen.findByText('Template installation incomplete')
    ).toBeInTheDocument();
    expect(screen.getAllByText('target-model').length).toBeGreaterThan(0);
    expect(screen.getByText('Application publish failed')).toBeInTheDocument();
    expect(api.installSystemTemplate).toHaveBeenCalledWith(body, 'csrf-token');
    expect(
      screen.getByRole('button', { name: 'Install template' })
    ).toBeDisabled();
    fireEvent.click(screen.getAllByRole('button', { name: 'Close' }).at(-1)!);
    expect(screen.queryByText('target-model')).not.toBeInTheDocument();
  });
  test('downloads the ZIP bytes and filename returned by archive export', async () => {
    api.getSystemTemplateCatalog.mockResolvedValue({
      pages: [],
      applications: [],
      data_models: [],
      i18n_entries: [],
      mcp_instances: [{ id: 'agent-tools', name: 'Agent tools' }]
    });
    api.exportSystemTemplateArchive.mockResolvedValue({
      archive_base64: 'UEsDBAD/',
      file_name: 'selected-template.zip'
    });
    const createUrl = vi
      .fn<(blob: Blob) => string>()
      .mockReturnValue('blob:template-archive');
    Object.defineProperty(URL, 'createObjectURL', {
      configurable: true,
      writable: true,
      value: createUrl
    });
    Object.defineProperty(URL, 'revokeObjectURL', {
      configurable: true,
      writable: true,
      value: vi.fn()
    });
    const click = vi
      .spyOn(HTMLAnchorElement.prototype, 'click')
      .mockImplementation(() => {});
    setup();
    fireEvent.click(screen.getByRole('button', { name: 'Export template' }));
    fireEvent.mouseDown(
      await screen.findByRole('combobox', { name: 'MCP instances' })
    );
    fireEvent.click(await screen.findByText('Agent tools'));
    fireEvent.click(screen.getByRole('button', { name: 'Download ZIP' }));
    await waitFor(() => expect(click).toHaveBeenCalledOnce());
    expect(api.exportSystemTemplateArchive).toHaveBeenCalledWith(
      {
        page_ids: [],
        application_ids: [],
        data_model_ids: [],
        mcp_instance_ids: ['agent-tools'],
        i18n_keys: []
      },
      'csrf-token'
    );
    const blob = createUrl.mock.calls[0][0] as Blob;
    expect(blob.type).toBe('application/zip');
    const bytes = await new Promise<ArrayBuffer>((resolve) => {
      const reader = new FileReader();
      reader.onload = () => resolve(reader.result as ArrayBuffer);
      reader.readAsArrayBuffer(blob);
    });
    expect(Array.from(new Uint8Array(bytes))).toEqual([80, 75, 3, 4, 0, 255]);
    const anchor = click.mock.contexts[0] as HTMLAnchorElement;
    expect(anchor.download).toBe('selected-template.zip');
    expect(anchor.href).toBe('blob:template-archive');
  });
  test('previews and installs a ZIP using the same opaque archive bytes', async () => {
    api.installSystemTemplate.mockResolvedValue({
      complete: true,
      created: [],
      updated: [],
      skipped: [],
      id_map: {},
      failures: []
    });
    const { container } = setup();
    const file = new File(
      [new Uint8Array([80, 75, 3, 4, 0, 255])],
      'template.ZIP',
      { type: 'application/zip' }
    );
    fireEvent.change(container.querySelector('input[type="file"]')!, {
      target: { files: [file] }
    });
    await waitFor(() =>
      expect(api.previewSystemTemplate).toHaveBeenCalledWith(
        { archive_base64: 'UEsDBAD/' },
        'csrf-token'
      )
    );
    const install = await screen.findByRole('button', {
      name: 'Install template'
    });
    await waitFor(() => expect(install).toBeEnabled());
    fireEvent.click(install);
    expect(await screen.findByText('Template installed')).toBeInTheDocument();
    expect(api.installSystemTemplate).toHaveBeenCalledWith(
      { archive_base64: 'UEsDBAD/' },
      'csrf-token'
    );
  });
  test('archive validation failure never enables installation', async () => {
    api.previewSystemTemplate.mockRejectedValue(
      new Error('archive digest mismatch')
    );
    const { container } = setup();
    const file = new File(['tampered archive'], 'template.zip', {
      type: 'application/zip'
    });
    fireEvent.change(container.querySelector('input[type="file"]')!, {
      target: { files: [file] }
    });
    expect(
      await screen.findByText(
        'Template preview failed. Clear this file and import it again.'
      )
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'Install template' })
    ).toBeDisabled();
    expect(api.installSystemTemplate).not.toHaveBeenCalled();
  });
});
