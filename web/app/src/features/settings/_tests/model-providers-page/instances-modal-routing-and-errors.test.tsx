import { describe, test, expect } from 'vitest';
import {
  authenticateAsModelProviderManager,
  modelProvidersApi,
  openProviderInstancesModal,
  openSourceManagementTab,
  renderApp
} from './instances-modal.fixtures';
import {
  buildSettingsModelProviderInstances,
  buildSettingsModelProviderOptions
} from '../model-provider-test-fixtures';
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import {
  modelProviderCatalogEntries,
  primaryContractProviderModels
} from '../../../../test/model-provider-contract-fixtures';

describe('ModelProvidersPage - instances modal', () => {
  test(
    'runs candidate refresh and delete from the provider instances modal',
    { timeout: 15000 },
    async () => {
      authenticateAsModelProviderManager();
      modelProvidersApi.refreshSettingsModelProviderModels.mockResolvedValue({
        provider_instance_id: 'provider-1',
        refresh_status: 'ready',
        source: 'remote',
        last_error_message: null,
        refreshed_at: '2026-04-18T10:03:00Z',
        models: []
      });
      modelProvidersApi.deleteSettingsModelProviderInstance.mockResolvedValue({
        deleted: true
      });

      renderApp('/settings/model-providers');

      const modal = await openProviderInstancesModal();
      await openSourceManagementTab(modal);

      fireEvent.click(
        await screen.findByRole('button', {
          name: '刷新候选模型 OpenAI Production'
        })
      );
      await waitFor(() => {
        expect(
          modelProvidersApi.validateSettingsModelProviderInstance
        ).toHaveBeenCalledWith('provider-1', 'csrf-123');
      });

      fireEvent.click(
        screen.getByRole('button', { name: '刷新模型 OpenAI Production' })
      );
      await waitFor(() => {
        expect(
          modelProvidersApi.refreshSettingsModelProviderModels
        ).toHaveBeenCalledWith('provider-1', 'csrf-123');
      });

      fireEvent.click(
        screen.getByRole('button', { name: '删除实例 OpenAI Production' })
      );
      await waitFor(() => {
        expect(
          modelProvidersApi.deleteSettingsModelProviderInstance
        ).toHaveBeenCalledWith('provider-1', 'csrf-123');
      });
    }
  );

  test(
    'explains source instance action buttons on hover',
    { timeout: 30_000 },
    async () => {
      authenticateAsModelProviderManager();

      renderApp('/settings/model-providers');

      const modal = await openProviderInstancesModal();
      await openSourceManagementTab(modal);

      const actions = [
        {
          buttonName: '编辑 API Key OpenAI Production',
          tooltip: '编辑来源实例的 API Key 和连接配置'
        },
        {
          buttonName: '刷新候选模型 OpenAI Production',
          tooltip: '验证当前配置，并预览可用候选模型'
        },
        {
          buttonName: '刷新模型 OpenAI Production',
          tooltip: '同步该来源实例的模型缓存'
        },
        {
          buttonName: '删除实例 OpenAI Production',
          tooltip: '删除该来源实例'
        }
      ];

      for (const action of actions) {
        fireEvent.mouseEnter(
          within(modal).getByRole('button', { name: action.buttonName })
        );
        expect(
          await screen.findByText(action.tooltip, undefined, { timeout: 10000 })
        ).toBeInTheDocument();
      }
    }
  );

  test(
    'opens the matching source instance drawer from a model group tag',
    { timeout: 15000 },
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

      renderApp('/settings/model-providers');

      const modal = await openProviderInstancesModal();
      const mainInstanceTable = within(modal).getByRole('table', {
        name: '主实例'
      });
      const aggregatedRow = within(mainInstanceTable).getByRole('row', {
        name: /gpt-4o-mini.*OpenAI Production.*OpenAI Backup/
      });

      fireEvent.click(
        within(aggregatedRow).getByRole('button', {
          name: 'OpenAI Production'
        })
      );

      expect(await screen.findByText('编辑 API 密钥配置')).toBeInTheDocument();
      expect(screen.getByText('OpenAI Compatible 实例')).toBeInTheDocument();
    }
  );

  test(
    'keeps the provider instances modal open while opening the edit drawer',
    { timeout: 15000 },
    async () => {
      authenticateAsModelProviderManager();

      renderApp('/settings/model-providers');

      const modal = await openProviderInstancesModal();
      await openSourceManagementTab(modal);
      fireEvent.click(
        within(modal).getByRole('button', {
          name: '编辑 API Key OpenAI Production'
        })
      );

      expect(await screen.findByText('编辑 API 密钥配置')).toBeInTheDocument();
      expect(screen.getByText('OpenAI Compatible 实例')).toBeInTheDocument();
      expect(
        within(modal).getByRole('tab', { name: '来源管理' })
      ).toHaveAttribute('aria-selected', 'true');
      expect(
        within(modal).getByRole('switch', {
          name: '注入主实例 OpenAI Production'
        })
      ).toBeInTheDocument();
    }
  );

  test(
    'AC-001 runs generic device and callback authorization actions from the instance drawer',
    { timeout: 15000 },
    async () => {
      authenticateAsModelProviderManager();
      const instance = buildSettingsModelProviderInstances()[0];
      modelProvidersApi.fetchSettingsModelProviderCatalog.mockResolvedValue([
        {
          ...modelProviderCatalogEntries[0],
          auth: {
            actions: [
              {
                code: 'device_login',
                label: 'Generic device sign-in',
                user_action_kinds: ['device_code']
              },
              {
                code: 'callback_login',
                label: 'Generic callback sign-in',
                user_action_kinds: ['paste_callback_url']
              }
            ]
          }
        }
      ]);
      modelProvidersApi.authenticateSettingsModelProviderInstance
        .mockResolvedValueOnce({
          instance,
          status: 'pending',
          message: null,
          user_action: {
            kind: 'device_code',
            open_url: 'https://provider.example/device',
            user_code: 'ABCD-EFGH',
            expires_at: '2026-08-19T12:00:00Z',
            poll_interval_seconds: 600,
            prompt: 'Use the code to continue.'
          }
        })
        .mockResolvedValueOnce({
          instance,
          status: 'cancelled',
          message: null,
          user_action: null
        })
        .mockResolvedValueOnce({
          instance,
          status: 'pending',
          message: null,
          user_action: {
            kind: 'paste_callback_url',
            open_url: 'https://provider.example/callback',
            user_code: null,
            expires_at: null,
            poll_interval_seconds: 600,
            prompt: 'Paste the callback URL after signing in.'
          }
        })
        .mockResolvedValueOnce({
          instance,
          status: 'authorized',
          message: null,
          user_action: null
        });

      renderApp('/settings/model-providers');

      const modal = await openProviderInstancesModal();
      await openSourceManagementTab(modal);
      fireEvent.click(
        within(modal).getByRole('button', {
          name: '编辑 API Key OpenAI Production'
        })
      );

      fireEvent.click(
        await screen.findByRole('button', { name: 'Generic device sign-in' })
      );
      await waitFor(() => {
        expect(
          modelProvidersApi.authenticateSettingsModelProviderInstance
        ).toHaveBeenCalledWith(
          'provider-1',
          { type: 'begin', action: 'device_login' },
          'csrf-123'
        );
      });
      expect(screen.getByText('ABCD-EFGH')).toBeInTheDocument();
      expect(
        screen.getByRole('link', { name: '打开授权页面' })
      ).toHaveAttribute('href', 'https://provider.example/device');

      fireEvent.click(screen.getByRole('button', { name: '取消授权' }));
      await waitFor(() => {
        expect(
          modelProvidersApi.authenticateSettingsModelProviderInstance
        ).toHaveBeenLastCalledWith(
          'provider-1',
          { type: 'cancel' },
          'csrf-123'
        );
      });

      fireEvent.click(
        screen.getByRole('button', { name: 'Generic callback sign-in' })
      );
      expect(
        await screen.findByLabelText('回调 URL 或代码')
      ).toBeInTheDocument();
      fireEvent.change(screen.getByLabelText('回调 URL 或代码'), {
        target: { value: 'http://127.0.0.1/callback?code=example' }
      });
      fireEvent.click(screen.getByRole('button', { name: '提交授权' }));
      await waitFor(() => {
        expect(
          modelProvidersApi.authenticateSettingsModelProviderInstance
        ).toHaveBeenLastCalledWith(
          'provider-1',
          {
            type: 'submit',
            value: 'http://127.0.0.1/callback?code=example'
          },
          'csrf-123'
        );
      });
      expect(await screen.findByText('已授权')).toBeInTheDocument();
    }
  );

  test(
    'folds advanced provider fields into a collapsed section in the edit drawer',
    { timeout: 15000 },
    async () => {
      authenticateAsModelProviderManager();

      renderApp('/settings/model-providers');

      const modal = await openProviderInstancesModal();
      await openSourceManagementTab(modal);
      fireEvent.click(
        within(modal).getByRole('button', {
          name: '编辑 API Key OpenAI Production'
        })
      );

      expect(await screen.findByText('编辑 API 密钥配置')).toBeInTheDocument();
      expect(screen.getByLabelText('API Endpoint')).toBeInTheDocument();
      expect(screen.queryByText('organization')).not.toBeInTheDocument();

      fireEvent.click(screen.getByText('高级配置（可选）'));

      expect(await screen.findByLabelText('organization')).toBeInTheDocument();
      expect(screen.getByLabelText('default_headers')).toBeInTheDocument();
    }
  );

  test(
    'hydrates cached model select from existing model catalog in edit mode',
    { timeout: 15000 },
    async () => {
      authenticateAsModelProviderManager();

      renderApp('/settings/model-providers');

      const modal = await openProviderInstancesModal();
      await openSourceManagementTab(modal);

      fireEvent.click(
        within(modal).getByRole('button', {
          name: '编辑 API Key OpenAI Production'
        })
      );

      expect(await screen.findByText('编辑 API 密钥配置')).toBeInTheDocument();
      await waitFor(() => {
        expect(
          modelProvidersApi.fetchSettingsModelProviderModels
        ).toHaveBeenCalledWith('provider-1');
      });

      const cachedModelSelect = await screen.findByRole('combobox', {
        name: '缓存模型'
      });
      fireEvent.mouseDown(cachedModelSelect);
      expect(
        (await screen.findAllByText(primaryContractProviderModels[0].model_id))
          .length
      ).toBeGreaterThanOrEqual(1);
      fireEvent.click(screen.getByRole('button', { name: '编辑模型 1' }));
      expect(await screen.findByText('编辑 模型配置')).toBeInTheDocument();
      expect(screen.getByLabelText('模型 ID')).toHaveValue('gpt-4o-mini');
    }
  );

  test(
    'masks api key by default and reveals it only after explicit action',
    { timeout: 15000 },
    async () => {
      authenticateAsModelProviderManager();

      renderApp('/settings/model-providers');

      const modal = await openProviderInstancesModal();
      await openSourceManagementTab(modal);
      fireEvent.click(
        within(modal).getByRole('button', {
          name: '编辑 API Key OpenAI Production'
        })
      );

      expect(await screen.findByText('编辑 API 密钥配置')).toBeInTheDocument();
      expect(screen.getByDisplayValue('supe****cret')).toBeInTheDocument();
      expect(
        screen.queryByDisplayValue('super-secret')
      ).not.toBeInTheDocument();

      fireEvent.click(screen.getByRole('button', { name: '显示 API Key' }));

      await waitFor(() => {
        expect(
          modelProvidersApi.revealSettingsModelProviderSecret
        ).toHaveBeenCalledWith('provider-1', 'api_key', 'csrf-123');
      });
      await waitFor(() => {
        expect(screen.getByLabelText('API Key')).toHaveValue('super-secret');
      });
    }
  );

  test(
    'AC-001 allows an unconfigured optional secret to be entered and submitted in edit mode',
    { timeout: 30000 },
    async () => {
      authenticateAsModelProviderManager();
      modelProvidersApi.fetchSettingsModelProviderCatalog.mockResolvedValue([
        {
          ...modelProviderCatalogEntries[0],
          form_schema: [
            ...modelProviderCatalogEntries[0].form_schema,
            {
              key: 'proxy_url',
              field_type: 'secret',
              label: '代理地址',
              required: false,
              advanced: true
            }
          ]
        }
      ]);

      renderApp('/settings/model-providers');

      const modal = await openProviderInstancesModal();
      await openSourceManagementTab(modal);
      fireEvent.click(
        within(modal).getByRole('button', {
          name: '编辑 API Key OpenAI Production'
        })
      );

      expect(await screen.findByText('编辑 API 密钥配置')).toBeInTheDocument();
      fireEvent.click(screen.getByText('高级配置（可选）'));

      const proxyUrlInput = await screen.findByLabelText('代理地址');
      expect(proxyUrlInput).not.toHaveAttribute('readonly');
      expect(
        screen.queryByRole('button', { name: '显示 代理地址' })
      ).not.toBeInTheDocument();

      fireEvent.change(proxyUrlInput, {
        target: { value: 'http://127.0.0.1:7897' }
      });
      fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));

      await waitFor(() => {
        expect(
          modelProvidersApi.updateSettingsModelProviderInstance
        ).toHaveBeenCalledWith(
          'provider-1',
          expect.objectContaining({
            config: expect.objectContaining({
              proxy_url: 'http://127.0.0.1:7897'
            })
          }),
          'csrf-123'
        );
      });
      expect(
        modelProvidersApi.revealSettingsModelProviderSecret
      ).not.toHaveBeenCalledWith('provider-1', 'proxy_url', 'csrf-123');
    }
  );
});
