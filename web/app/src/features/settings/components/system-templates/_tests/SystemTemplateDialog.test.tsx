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
      mcp_instances: []
    });
    api.previewSystemTemplate.mockResolvedValue({
      valid: true,
      counts: { pages: 1, applications: 1, data_models: 2, mcp_instances: 0 },
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
  test('server rejection prevents installation and replacing the file clears stale preview', async () => {
    api.previewSystemTemplate.mockResolvedValue({
      valid: false,
      counts: { pages: 0, applications: 0, data_models: 0, mcp_instances: 0 },
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
        mcp_instance_ids: ['agent-tools']
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
