import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

const compatibleTemplatesApi = vi.hoisted(() => ({
  fetch: vi.fn()
}));

vi.mock('../../api/data-models', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../api/data-models')>();
  return {
    ...actual,
    fetchSettingsCompatibleDataModelTemplates: compatibleTemplatesApi.fetch
  };
});

import type {
  SettingsCompatibleDataModelTemplate,
  SettingsDataSourceRemoteResource,
  SettingsRuntimeExtensionDataSource
} from '../../api/data-models';
import { settingsCompatibleDataModelTemplatesQueryKey } from '../../api/data-models';
import { DataSourceResourcesPanel } from '../../components/data-models/DataSourceResourcesPanel';

const dataSource: SettingsRuntimeExtensionDataSource = {
  id: 'source-1',
  display_name: 'HubSpot',
  status: 'ready',
  enabled: true,
  fixed: false,
  default_data_model_status: 'draft',
  capabilities: {
    can_update_defaults: true,
    can_create_data_model: false,
    can_validate: true,
    can_discover_resources: true,
    can_preview_resources: true,
    can_map_resources: true
  },
  backend: {
    kind: 'runtime_extension',
    installation_id: 'installation-1',
    source_code: 'hubspot',
    config_json: {},
    secret_ref: null,
    secret_version: null,
    catalog_refresh_status: 'ready',
    catalog_last_error_message: null,
    catalog_refreshed_at: '2026-04-30T08:00:00Z'
  }
};

const contacts: SettingsDataSourceRemoteResource = {
  resource_key: 'contacts',
  display_name: 'Contacts',
  resource_kind: 'object',
  capabilities: {},
  metadata: {}
};

const selectedTemplate: SettingsCompatibleDataModelTemplate = {
  template_provider: 'plugin.crm',
  template_code: 'contact_tree',
  template_version: 'v2',
  summary: '联系人树',
  description: '由 CRM 插件提供的联系人树。',
  system_fields: [
    {
      code: 'id',
      summary: 'Contact identifier',
      description: 'Stable external contact identifier.',
      field_kind: 'string',
      required: true
    }
  ]
};

beforeEach(() => {
  vi.clearAllMocks();
  compatibleTemplatesApi.fetch.mockResolvedValue([selectedTemplate]);
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('external Data Model template refetch', () => {
  test.each([
    ['a catalog without the selected identity', 'incompatible'],
    ['an empty catalog', 'empty'],
    ['a catalog error', 'error']
  ] as const)(
    'AC-013 clears selection while refetching %s and remains fail closed',
    async (_label, outcome) => {
      const queryClient = new QueryClient({
        defaultOptions: {
          queries: { retry: false },
          mutations: { retry: false }
        }
      });
      const onMap = vi.fn();
      const view = render(
        <QueryClientProvider client={queryClient}>
          <DataSourceResourcesPanel
            dataSource={dataSource}
            resources={[contacts]}
            loading={false}
            validating={false}
            discovering={false}
            previewingResourceKey={null}
            mappingResourceKey={null}
            canManage
            onValidate={vi.fn()}
            onDiscover={vi.fn()}
            onPreview={vi.fn()}
            onMap={onMap}
          />
        </QueryClientProvider>
      );

      try {
        fireEvent.click(
          await screen.findByRole('button', { name: '映射为 Data Model' })
        );
        const templateSelector = await screen.findByRole('combobox', {
          name: 'Data Model 模板'
        });
        await waitFor(() => expect(templateSelector).toBeEnabled());
        fireEvent.mouseDown(templateSelector);
        fireEvent.click(await screen.findByText('联系人树'));
        expect(
          screen.queryByText('plugin.crm/contact_tree/v2')
        ).not.toBeInTheDocument();
        expect(
          screen.queryByText('由 CRM 插件提供的联系人树。')
        ).not.toBeInTheDocument();
        expect(
          screen.getByText(selectedTemplate.description)
        ).toBeInTheDocument();
        const confirmButtons = screen.getAllByRole('button', {
          name: '映射为 Data Model'
        });
        const confirmButton = confirmButtons[confirmButtons.length - 1];
        expect(confirmButton).toBeEnabled();

        let resolveRefetch!: (
          templates: SettingsCompatibleDataModelTemplate[]
        ) => void;
        let rejectRefetch!: (error: Error) => void;
        const refetchResult = new Promise<
          SettingsCompatibleDataModelTemplate[]
        >((resolve, reject) => {
          resolveRefetch = resolve;
          rejectRefetch = reject;
        });
        compatibleTemplatesApi.fetch.mockImplementationOnce(
          () => refetchResult
        );

        act(() => {
          void queryClient.invalidateQueries({
            queryKey: settingsCompatibleDataModelTemplatesQueryKey(
              dataSource.id,
              contacts.resource_key
            )
          });
        });

        await waitFor(() =>
          expect(compatibleTemplatesApi.fetch).toHaveBeenCalledTimes(2)
        );
        await waitFor(() =>
          expect(templateSelector).toHaveAttribute('aria-busy', 'true')
        );
        expect(templateSelector).toBeDisabled();
        expect(
          screen.queryByText(selectedTemplate.description)
        ).not.toBeInTheDocument();
        expect(confirmButton).toBeDisabled();
        expect(onMap).not.toHaveBeenCalled();

        await act(async () => {
          if (outcome === 'error') {
            rejectRefetch(new Error('compatible catalog refetch unavailable'));
            await refetchResult.catch(() => undefined);
            return;
          }
          resolveRefetch(
            outcome === 'empty'
              ? []
              : [
                  {
                    template_provider: 'plugin.crm',
                    template_code: 'contact_list',
                    template_version: 'v1',
                    summary: '联系人列表',
                    description: '不再包含先前选择的模板身份。',
                    system_fields: []
                  }
                ]
          );
          await refetchResult;
        });

        if (outcome === 'error') {
          expect(
            await screen.findByText('compatible catalog refetch unavailable')
          ).toBeInTheDocument();
          expect(templateSelector).toBeDisabled();
        } else if (outcome === 'empty') {
          expect(
            await screen.findByText('当前数据源没有可用的 Data Model 模板。')
          ).toBeInTheDocument();
          expect(templateSelector).toBeDisabled();
        } else {
          await waitFor(() => expect(templateSelector).toBeEnabled());
          expect(screen.queryByText('联系人树')).not.toBeInTheDocument();
        }
        expect(confirmButton).toBeDisabled();
        expect(onMap).not.toHaveBeenCalled();
      } finally {
        view.unmount();
        queryClient.clear();
      }
    }
  );
});
