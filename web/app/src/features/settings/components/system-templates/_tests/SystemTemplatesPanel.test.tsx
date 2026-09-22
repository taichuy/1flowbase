import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { App } from 'antd';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const api = vi.hoisted(() => ({
  getSystemTemplateCatalog: vi.fn(),
  exportSystemTemplate: vi.fn(),
  previewSystemTemplate: vi.fn(),
  installSystemTemplate: vi.fn()
}));
vi.mock('@1flowbase/api-client', () => api);
import { appI18n } from '../../../../../shared/i18n/app-i18n';
import { useAuthStore } from '../../../../../state/auth-store';
import { SystemTemplatesPanel } from '../SystemTemplatesPanel';

const body = {
  schema_version: '1flowbase.portable-template/v1',
  pages: [],
  applications: [],
  data_models: [],
  plugins: []
};
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
        <SystemTemplatesPanel />
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
  beforeEach(async () => {
    Object.values(api).forEach((mock) => mock.mockReset());
    await appI18n.changeLanguage('en_US');
    useAuthStore.setState({ csrfToken: 'csrf-token' });
    api.getSystemTemplateCatalog.mockResolvedValue({
      pages: [],
      applications: [],
      data_models: []
    });
    api.previewSystemTemplate.mockResolvedValue({
      valid: true,
      counts: { pages: 1, applications: 1, data_models: 2 },
      failures: [],
      warnings: [],
      dependencies: []
    });
  });
  test('export starts with no selected objects', async () => {
    setup();
    fireEvent.click(screen.getByRole('button', { name: 'Export template' }));
    expect(
      await screen.findByRole('button', { name: 'Download JSON' })
    ).toBeDisabled();
    expect(api.exportSystemTemplate).not.toHaveBeenCalled();
  });
  test('server rejection prevents installation and replacing the file clears stale preview', async () => {
    api.previewSystemTemplate.mockResolvedValue({
      valid: false,
      counts: { pages: 0, applications: 0, data_models: 0 },
      failures: ['Route already exists'],
      warnings: [],
      dependencies: []
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
        'The file is not valid JSON. Choose another file.'
      )
    ).toBeInTheDocument();
    expect(screen.queryByText('Route already exists')).not.toBeInTheDocument();
    expect(api.installSystemTemplate).not.toHaveBeenCalled();
  });
  test('installs the complete previewed package and preserves honest partial outcome', async () => {
    api.installSystemTemplate.mockResolvedValue({
      complete: false,
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
    fireEvent.click(
      screen.getByRole('button', { name: 'Clear import report' })
    );
    expect(screen.queryByText('target-model')).not.toBeInTheDocument();
  });
});
