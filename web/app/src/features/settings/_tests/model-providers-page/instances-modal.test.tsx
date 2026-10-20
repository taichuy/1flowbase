import { describe, test, expect, vi } from 'vitest';
import {
  authenticateAsModelProviderManager,
  modelProvidersApi,
  openProviderInstancesModal,
  openSourceManagementTab
} from './instances-modal.fixtures';
import { AppProviders } from '../../../../app/AppProviders';
import { ModelProviderInstancesModal } from '../../components/model-providers/ModelProviderInstancesModal';
import { SettingsModelProvidersSection } from '../../pages/settings-page/SettingsModelProvidersSection';
import {
  buildMainInstanceSettings,
  buildSettingsModelProviderInstances,
  buildSettingsModelProviderOptions
} from '../model-provider-test-fixtures';
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import {
  modelProviderCatalogEntries,
  primaryContractProviderModels
} from '../../../../test/model-provider-contract-fixtures';

describe('ModelProvidersPage - instances modal', () => {
  test(
    'wires preview and create submission from the model provider drawer into the settings api',
    { timeout: 20000 },
    async () => {
      authenticateAsModelProviderManager();
      modelProvidersApi.createSettingsModelProviderInstance.mockResolvedValue({
        id: 'provider-3',
        installation_id: modelProviderCatalogEntries[0].installation_id,
        provider_code: modelProviderCatalogEntries[0].provider_code,
        protocol: modelProviderCatalogEntries[0].protocol,
        display_name: 'OpenAI Draft',
        status: 'ready',
        included_in_main: true,
        config_json: {
          base_url: 'https://api.openai.com/v1',
          api_key: 'supe****cret'
        },
        configured_models: [
          {
            model_id: 'gpt-4o-mini',
            enabled: true,
            context_window_override_tokens: null
          }
        ],
        enabled_model_ids: ['gpt-4o-mini'],
        catalog_refresh_status: 'ready',
        catalog_last_error_message: null,
        catalog_refreshed_at: '2026-04-18T10:05:00Z',
        model_count: 1
      });

      render(
        <AppProviders>
          <SettingsModelProvidersSection canManage />
        </AppProviders>
      );

      fireEvent.click(await screen.findByRole('button', { name: '新增' }));

      expect(await screen.findByText('API 密钥授权配置')).toBeInTheDocument();
      expect(screen.getByRole('switch', { name: '注入主实例' })).toBeChecked();

      fireEvent.change(screen.getByLabelText('API Endpoint'), {
        target: { value: 'https://api.openai.com/v1' }
      });
      fireEvent.change(screen.getByLabelText('API Key'), {
        target: { value: 'super-secret' }
      });
      fireEvent.change(screen.getByLabelText('名称'), {
        target: { value: 'OpenAI Draft' }
      });

      fireEvent.click(screen.getByRole('button', { name: /检\s*测/ }));

      await waitFor(() => {
        expect(
          modelProvidersApi.previewSettingsModelProviderModels
        ).toHaveBeenCalledWith(
          {
            installation_id: modelProviderCatalogEntries[0].installation_id,
            config: {
              base_url: 'https://api.openai.com/v1',
              api_key: 'super-secret'
            }
          },
          'csrf-123'
        );
      });

      const cachedModelSelect = screen.getByRole('combobox', {
        name: '缓存模型'
      });
      fireEvent.mouseDown(cachedModelSelect);
      fireEvent.click(await screen.findByText('gpt-4o-mini'));
      expect(screen.queryByLabelText('模型 ID 1')).not.toBeInTheDocument();

      fireEvent.click(
        within(screen.getByRole('dialog')).getByRole('button', { name: '新增' })
      );
      expect(await screen.findByText('新增 模型配置')).toBeInTheDocument();
      expect(screen.getByLabelText('模型 ID')).toHaveValue('gpt-4o-mini');
      fireEvent.click(screen.getByRole('button', { name: /确\s*认/ }));
      await waitFor(() => {
        expect(screen.queryByText('新增 模型配置')).not.toBeInTheDocument();
      });
      expect(screen.getByRole('switch', { name: '启用模型 1' })).toBeChecked();

      fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));

      await waitFor(() => {
        expect(
          modelProvidersApi.createSettingsModelProviderInstance
        ).toHaveBeenCalledWith(
          {
            installation_id: modelProviderCatalogEntries[0].installation_id,
            display_name: 'OpenAI Draft',
            config: {
              base_url: 'https://api.openai.com/v1',
              api_key: 'super-secret'
            },
            configured_models: [
              {
                model_id: 'gpt-4o-mini',
                enabled: true,
                context_window_override_tokens: null,
                supports_multimodal: false,
                pricing_provider_code: 'zero',
                pricing_model_id: 'any'
              }
            ],
            included_in_main: true,
            preview_token: 'preview-1'
          },
          'csrf-123'
        );
      });
    }
  );

  test(
    'opens provider instances modal from installed provider row as a management list',
    { timeout: 30000 },
    async () => {
      authenticateAsModelProviderManager();
      const options = buildSettingsModelProviderOptions();
      options.providers[0].model_groups[0] = {
        model_id: primaryContractProviderModels[0].model_id,
        pricing_configured: false,
        distribution_rule: 'none',
        model: primaryContractProviderModels[0],
        targets: [
          {
            source_instance_id: 'provider-1',
            source_instance_display_name: 'OpenAI Production',
            routing_enabled: true,
            model: primaryContractProviderModels[0]
          },
          {
            source_instance_id: 'provider-2',
            source_instance_display_name: 'OpenAI Backup',
            routing_enabled: true,
            model: primaryContractProviderModels[0]
          }
        ]
      };
      modelProvidersApi.fetchSettingsModelProviderOptions.mockResolvedValueOnce(
        options
      );

      render(
        <AppProviders>
          <SettingsModelProvidersSection canManage />
        </AppProviders>
      );

      const modal = await openProviderInstancesModal();
      expect(screen.getByTestId('fixed-height-modal-scroll-body')).toHaveClass(
        'model-provider-panel__instances-modal'
      );
      expect(
        within(modal).getByRole('tab', { name: '模型管理' })
      ).toHaveAttribute('aria-selected', 'true');
      expect(
        within(modal).getByRole('tab', { name: '来源管理' })
      ).toBeInTheDocument();
      expect(within(modal).getByText('聚合视图')).toBeInTheDocument();
      expect(
        within(modal).getByRole('switch', { name: '新实例自动注入主实例' })
      ).toBeInTheDocument();
      const mainInstanceTable = within(modal).getByRole('table', {
        name: '主实例'
      });
      expect(
        within(mainInstanceTable).getByRole('columnheader', {
          name: '模型 ID'
        })
      ).toBeInTheDocument();
      expect(
        within(mainInstanceTable).getByRole('columnheader', { name: '分组' })
      ).toBeInTheDocument();
      expect(
        within(mainInstanceTable).queryByRole('columnheader', {
          name: '计费状态'
        })
      ).not.toBeInTheDocument();
      expect(
        within(mainInstanceTable).queryByRole('link', { name: '模型计费规则' })
      ).not.toBeInTheDocument();
      expect(
        within(mainInstanceTable).getByRole('row', {
          name: /gpt-4o-mini.*OpenAI Production.*OpenAI Backup/
        })
      ).toBeInTheDocument();
      const aggregatedRow = within(mainInstanceTable).getByRole('row', {
        name: /gpt-4o-mini.*OpenAI Production.*OpenAI Backup/
      });
      expect(within(aggregatedRow).getByText('gpt-4o-mini')).not.toHaveClass(
        'model-provider-panel__mono'
      );
      const productionTag =
        within(aggregatedRow).getByText('OpenAI Production');
      const backupTag = within(aggregatedRow).getByText('OpenAI Backup');
      expect(productionTag).toHaveClass('ant-tag-geekblue');
      expect(backupTag).toHaveClass('ant-tag-green');
      expect(
        within(modal).queryByRole('combobox', { name: '主实例' })
      ).not.toBeInTheDocument();
      expect(
        within(modal).queryByRole('switch', {
          name: '注入主实例 OpenAI Production'
        })
      ).not.toBeInTheDocument();
      await openSourceManagementTab(modal);
      expect(
        within(modal).getByRole('switch', {
          name: '注入主实例 OpenAI Production'
        })
      ).toBeInTheDocument();
      expect(
        within(modal).getByRole('switch', { name: '注入主实例 OpenAI Backup' })
      ).toBeInTheDocument();
      expect(
        within(modal).getAllByText(/gpt-4o-mini/i).length
      ).toBeGreaterThanOrEqual(1);
      expect(
        within(modal).getAllByText('OpenAI Backup').length
      ).toBeGreaterThanOrEqual(1);
    }
  );

  test(
    'AC-001 renders distribution_rule in the main-instance aggregation table and emits the DTO field value',
    { timeout: 15000 },
    async () => {
      authenticateAsModelProviderManager();
      let mainInstanceState = buildMainInstanceSettings(true, 'none');
      const distributionRuleChangeSpy = vi.fn(
        (modelId: string, distributionRule: string) => {
          mainInstanceState = {
            ...mainInstanceState,
            model_routing_policies:
              mainInstanceState.model_routing_policies.map((policy) =>
                policy.model_id === modelId
                  ? { ...policy, distribution_rule: distributionRule }
                  : policy
              )
          };
        }
      );

      modelProvidersApi.fetchSettingsModelProviderMainInstance.mockImplementation(
        async () => mainInstanceState
      );
      render(
        <AppProviders>
          <ModelProviderInstancesModal
            open
            catalogEntry={modelProviderCatalogEntries[0]}
            providerDisplayName="OpenAI Compatible"
            mainInstance={mainInstanceState}
            modelGroups={
              buildSettingsModelProviderOptions().providers[0].model_groups
            }
            instances={buildSettingsModelProviderInstances()}
            updatingMainInstance={false}
            updatingInstanceId={null}
            refreshingCandidates={false}
            refreshing={false}
            deleting={false}
            canManage={true}
            versionSwitchNotice={null}
            onClose={vi.fn()}
            onEdit={vi.fn()}
            onRefreshCandidates={vi.fn()}
            onRefreshModels={vi.fn()}
            onDelete={vi.fn()}
            onToggleAutoIncludeNewInstances={vi.fn()}
            onChangeDistributionRule={distributionRuleChangeSpy}
            onSaveRoutingPolicy={vi.fn()}
            onToggleIncludedInMain={vi.fn()}
          />
        </AppProviders>
      );

      const modal = await screen.findByRole('dialog', {
        name: /OpenAI Compatible 实例/
      });
      const mainInstanceTable = within(modal).getByRole('table', {
        name: '主实例'
      });
      expect(
        within(mainInstanceTable).getByRole('columnheader', {
          name: '分发规则'
        })
      ).toBeInTheDocument();

      const distributionRuleSelect = mainInstanceTable.querySelector(
        'select.model-provider-panel__distribution-rule-select'
      );
      if (!(distributionRuleSelect instanceof HTMLSelectElement)) {
        throw new Error('expected distribution rule select');
      }
      expect(distributionRuleSelect).toBeEnabled();
      expect((await screen.findAllByText('无')).length).toBeGreaterThan(0);
      expect((await screen.findAllByText('轮询')).length).toBeGreaterThan(0);
      expect((await screen.findAllByText('重试轮询')).length).toBeGreaterThan(
        0
      );
      fireEvent.change(distributionRuleSelect, {
        target: { value: 'retry_round_robin' }
      });

      expect(distributionRuleChangeSpy).toHaveBeenCalledWith(
        primaryContractProviderModels[0].model_id,
        'retry_round_robin'
      );
    }
  );

  test('AC-001 edits the routing rule and ordered instance group atomically', async () => {
    authenticateAsModelProviderManager();
    const mainInstance = buildMainInstanceSettings(true, 'round_robin');
    mainInstance.model_routing_policies[0] = {
      ...mainInstance.model_routing_policies[0],
      provider_instance_ids: ['provider-2', 'provider-1']
    };
    const options = buildSettingsModelProviderOptions();
    const firstGroup = options.providers[0].model_groups[0];
    firstGroup.targets.push({
      source_instance_id: 'provider-2',
      source_instance_display_name: 'OpenAI Backup',
      routing_enabled: true,
      model: firstGroup.model
    });
    const saveSpy = vi.fn(
      (
        _modelId: string,
        _distributionRule: string,
        _distributionRuleContractVersion: string | undefined,
        _distributionRuleConfig: Record<string, unknown> | undefined,
        _providerInstanceIds: string[],
        _excludedProviderInstanceIds: string[],
        onSuccess: () => void
      ) => onSuccess()
    );

    render(
      <AppProviders>
        <ModelProviderInstancesModal
          open
          catalogEntry={modelProviderCatalogEntries[0]}
          providerDisplayName="OpenAI Compatible"
          mainInstance={mainInstance}
          modelGroups={options.providers[0].model_groups}
          instances={buildSettingsModelProviderInstances()}
          updatingMainInstance={false}
          updatingInstanceId={null}
          refreshingCandidates={false}
          refreshing={false}
          deleting={false}
          canManage={true}
          versionSwitchNotice={null}
          onClose={vi.fn()}
          onEdit={vi.fn()}
          onRefreshCandidates={vi.fn()}
          onRefreshModels={vi.fn()}
          onDelete={vi.fn()}
          onToggleAutoIncludeNewInstances={vi.fn()}
          onChangeDistributionRule={vi.fn()}
          onSaveRoutingPolicy={saveSpy}
          onToggleIncludedInMain={vi.fn()}
        />
      </AppProviders>
    );

    const table = screen.getByRole('table', { name: '主实例' });
    fireEvent.click(within(table).getAllByRole('button', { name: '编辑' })[0]);
    await waitFor(() => {
      expect(
        document.querySelector('.model-provider-panel__routing-policy-modal')
      ).toBeInTheDocument();
    });
    const policyDialog = document.querySelector(
      '.model-provider-panel__routing-policy-modal'
    ) as HTMLElement;
    expect(
      within(policyDialog).getByText('编辑轮询规则与分组顺序')
    ).toBeInTheDocument();
    const routingTable = within(policyDialog).getByRole('table');
    const rowNames = () =>
      Array.from(routingTable.querySelectorAll('tbody tr')).map(
        (row) => row.textContent
      );
    expect(rowNames()).toEqual([
      expect.stringContaining('OpenAI Backup'),
      expect.stringContaining('OpenAI Production')
    ]);
    expect(
      within(routingTable).getByRole('columnheader', { name: '参与分组' })
    ).toBeInTheDocument();
    fireEvent.click(
      within(routingTable).getByRole('switch', {
        name: '参与分组 OpenAI Backup'
      })
    );

    Array.from(routingTable.querySelectorAll('tbody tr')).forEach(
      (row, index) => {
        vi.spyOn(row, 'getBoundingClientRect').mockReturnValue({
          x: 0,
          y: index * 48,
          top: index * 48,
          right: 640,
          bottom: (index + 1) * 48,
          left: 0,
          width: 640,
          height: 48,
          toJSON: () => ({})
        });
      }
    );
    const firstDragHandle = within(routingTable).getAllByRole('button', {
      name: '拖拽排序'
    })[0];
    firstDragHandle.focus();
    fireEvent.keyDown(firstDragHandle, { key: ' ', code: 'Space' });
    await waitFor(() => {
      expect(firstDragHandle).toHaveAttribute('aria-pressed', 'true');
    });
    fireEvent.keyDown(firstDragHandle, { key: 'ArrowDown', code: 'ArrowDown' });
    fireEvent.keyDown(firstDragHandle, { key: ' ', code: 'Space' });
    await waitFor(() => {
      expect(rowNames()).toEqual([
        expect.stringContaining('OpenAI Production'),
        expect.stringContaining('OpenAI Backup')
      ]);
    });
    fireEvent.change(
      within(policyDialog).getByRole('combobox', { name: '分发规则' }),
      { target: { value: 'retry_round_robin' } }
    );
    fireEvent.click(
      within(policyDialog).getByRole('button', { name: /保\s*存/ })
    );

    expect(saveSpy).toHaveBeenCalledWith(
      primaryContractProviderModels[0].model_id,
      'retry_round_robin',
      '1flowbase.provider-distribution-rule/v1',
      {},
      ['provider-1', 'provider-2'],
      ['provider-2'],
      expect.any(Function)
    );
  });
});
