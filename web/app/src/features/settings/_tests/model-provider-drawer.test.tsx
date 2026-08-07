import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';
import { modelProviderCatalogEntries } from '../../../test/model-provider-contract-fixtures';
import { ModelProviderInstanceDrawer } from '../components/model-providers/ModelProviderInstanceDrawer';
import {
  MODEL_CONTEXT_WINDOW_PRESET_OPTIONS,
  MODEL_CONTEXT_WINDOW_VALIDATION_MESSAGE,
  formatModelContextWindowValue,
  parseModelContextWindowInput
} from '../components/model-providers/model-context-window';
import { buildSettingsModelProviderInstances } from './model-provider-test-fixtures';

describe('model-context-window helpers', () => {
  test.each([
    ['200000', 200000],
    ['200K', 200000],
    ['1M', 1000000]
  ])('parses %s into numeric tokens', (input, expectedValue) => {
    expect(parseModelContextWindowInput(input)).toEqual({
      value: expectedValue,
      error: null
    });
  });

  test('exposes the supported preset choices', () => {
    expect(
      MODEL_CONTEXT_WINDOW_PRESET_OPTIONS.map((option) => option.value)
    ).toEqual(['16K', '32K', '64K', '128K', '256K', '1M']);
  });

  test.each([
    [16000, '16K'],
    [32000, '32K'],
    [64000, '64K'],
    [128000, '128K'],
    [256000, '256K'],
    [1000000, '1M']
  ])(
    'formats %s into preferred uppercase display %s',
    (input, expectedValue) => {
      expect(formatModelContextWindowValue(input)).toBe(expectedValue);
    }
  );

  test.each(['abc', '1g', '10kk', '   '])(
    'rejects invalid context window input %s',
    (input) => {
      expect(parseModelContextWindowInput(input)).toEqual({
        value: null,
        error: '请输入有效的上下文大小，支持纯数字、K 或 M 后缀。'
      });
    }
  );
});

describe('ModelProviderInstanceDrawer', () => {
  test('uses the shared resizable drawer with the provider width range', async () => {
    render(
      <ModelProviderInstanceDrawer
        open
        mode="create"
        catalogEntry={modelProviderCatalogEntries[0]}
        instance={null}
        cachedModelCatalog={null}
        defaultIncludedInMain={true}
        submitting={false}
        onClose={() => undefined}
        onSubmit={async () => undefined}
        onPreviewModels={async () => ({
          models: [],
          preview_token: 'preview-1',
          expires_at: '2026-04-22T12:00:00Z'
        })}
        onRevealSecret={async () => 'super-secret'}
      />
    );

    expect(
      await screen.findByRole('separator', { name: '调整供应商抽屉宽度' })
    ).toHaveAttribute('aria-valuenow', '560');
    expect(screen.getByRole('separator', { name: '调整供应商抽屉宽度' })).toHaveAttribute(
      'aria-valuemin',
      '480'
    );
    expect(screen.getByRole('separator', { name: '调整供应商抽屉宽度' })).toHaveAttribute(
      'aria-valuemax',
      '1200'
    );
  });

  test(
    'renders enum config fields as selects and submits the schema default',
    { timeout: 15000 },
    async () => {
      const submit = vi.fn().mockResolvedValue(undefined);
      const catalogEntry = {
        ...modelProviderCatalogEntries[0],
        form_schema: [
          ...modelProviderCatalogEntries[0].form_schema.slice(0, 2),
          {
            key: 'api_protocol',
            field_type: 'enum',
            required: false,
            advanced: false,
            control: 'select',
            description: '选择模型供应商 API 协议。',
            default_value: 'openai_chat',
            options: [
              {
                label: 'OpenAI Chat Completions',
                value: 'openai_chat'
              },
              {
                label: 'OpenAI Responses',
                value: 'openai_responses'
              }
            ]
          },
          ...modelProviderCatalogEntries[0].form_schema.slice(2)
        ]
      };

      render(
        <ModelProviderInstanceDrawer
          open
          mode="create"
          catalogEntry={catalogEntry}
          instance={null}
          cachedModelCatalog={null}
          defaultIncludedInMain={true}
          submitting={false}
          onClose={() => undefined}
          onSubmit={submit}
          onPreviewModels={async () => ({
            models: [],
            preview_token: 'preview-1',
            expires_at: '2026-04-22T12:00:00Z'
          })}
          onRevealSecret={async () => 'super-secret'}
        />
      );

      expect(
        await screen.findByRole('combobox', { name: 'API 协议' })
      ).toBeInTheDocument();
      expect(screen.getByText('选择模型供应商 API 协议。')).toBeInTheDocument();
      expect(screen.getByText('OpenAI Chat Completions')).toBeInTheDocument();

      fireEvent.change(screen.getByLabelText('API Endpoint'), {
        target: { value: 'https://dashscope.aliyuncs.com/compatible-mode/v1' }
      });
      fireEvent.change(screen.getByLabelText('API Key'), {
        target: { value: 'super-secret' }
      });
      fireEvent.change(screen.getByLabelText('名称'), {
        target: { value: 'Alibaba Bailian' }
      });
      fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));

      await waitFor(() => {
        expect(submit).toHaveBeenCalledWith(
          expect.objectContaining({
            display_name: 'Alibaba Bailian',
            config: {
              base_url: 'https://dashscope.aliyuncs.com/compatible-mode/v1',
              api_key: 'super-secret',
              api_protocol: 'openai_chat'
            }
          })
        );
      });
    }
  );

  test(
    'loads candidate models from the draft drawer and submits grouped configured model rows',
    { timeout: 30000 },
    async () => {
      const previewModels = vi.fn().mockResolvedValue({
        models: [
          {
            model_id: 'gpt-4o-mini',
            display_name: 'gpt-4o-mini',
            source: 'dynamic',
            supports_streaming: true,
            supports_tool_call: true,
            supports_multimodal: true,
            context_window: null,
            max_output_tokens: null,
            parameter_form: null,
            provider_metadata: {}
          }
        ],
        preview_token: 'preview-1',
        expires_at: '2026-04-22T12:00:00Z'
      });
      const submit = vi.fn().mockResolvedValue(undefined);

      render(
        <ModelProviderInstanceDrawer
          open
          mode="create"
          catalogEntry={modelProviderCatalogEntries[0]}
          instance={null}
          cachedModelCatalog={null}
          defaultIncludedInMain={true}
          submitting={false}
          onClose={() => undefined}
          onSubmit={submit}
          onPreviewModels={previewModels}
          onRevealSecret={async () => 'super-secret'}
        />
      );

      await screen.findByRole('dialog');
      expect(screen.getByText('API 密钥授权配置')).toBeInTheDocument();
      expect(
        screen.getByRole('button', { name: '新增' })
      ).toBeInTheDocument();
      expect(screen.queryByText('校验模型')).not.toBeInTheDocument();
      expect(screen.queryByText('validate_model')).not.toBeInTheDocument();
      expect(screen.queryByLabelText('organization')).not.toBeInTheDocument();
      expect(screen.getByText('高级配置（可选）')).toBeInTheDocument();
      expect(
        screen.getByRole('combobox', { name: '缓存模型' })
      ).not.toHaveAttribute('aria-disabled', 'true');
      expect(
        screen.getByRole('button', { name: /检\s*测/ })
      ).toBeInTheDocument();
      expect(
        screen.getByRole('button', { name: /保\s*存/ })
      ).toBeInTheDocument();
      expect(
        screen.getByRole('button', { name: /取\s*消/ })
      ).toBeInTheDocument();

      fireEvent.change(screen.getByLabelText('API Endpoint'), {
        target: { value: 'https://api.openai.com/v1' }
      });
      fireEvent.change(screen.getByLabelText('API Key'), {
        target: { value: 'super-secret' }
      });
      fireEvent.change(screen.getByLabelText('名称'), {
        target: { value: 'OpenAI Production' }
      });

      const expectedConfig = {
        base_url: 'https://api.openai.com/v1',
        api_key: 'super-secret'
      };

      fireEvent.click(screen.getByRole('button', { name: /检\s*测/ }));

      await waitFor(() => {
        expect(previewModels).toHaveBeenCalledWith(expectedConfig);
      });

      const cachedModelSelect = screen.getByRole('combobox', {
        name: '缓存模型'
      });
      fireEvent.mouseDown(cachedModelSelect);
      fireEvent.click(await screen.findByText('gpt-4o-mini'));
      expect(screen.queryByLabelText('模型 ID 1')).not.toBeInTheDocument();

      fireEvent.click(screen.getByRole('button', { name: '新增' }));
      fireEvent.change(screen.getByLabelText('模型 ID 1'), {
        target: { value: 'gpt-4o-mini' }
      });

      fireEvent.click(screen.getByRole('button', { name: '新增' }));

      fireEvent.change(screen.getByLabelText('模型 ID 2'), {
        target: { value: 'manual-model-id' }
      });
      fireEvent.click(screen.getByRole('switch', { name: '启用模型 2' }));

      previewModels.mockResolvedValueOnce({
        models: [
          {
            model_id: 'gpt-4.1-mini',
            display_name: 'gpt-4.1-mini',
            source: 'dynamic',
            supports_streaming: true,
            supports_tool_call: true,
            supports_multimodal: false,
            context_window: null,
            max_output_tokens: null,
            parameter_form: null,
            provider_metadata: {}
          }
        ],
        preview_token: 'preview-2',
        expires_at: '2026-04-22T13:00:00Z'
      });

      fireEvent.click(screen.getByRole('button', { name: /检\s*测/ }));

      await waitFor(() => {
        expect(previewModels).toHaveBeenCalledTimes(2);
      });
      expect(screen.getByLabelText('模型 ID 1')).toHaveValue('gpt-4o-mini');
      expect(screen.getByLabelText('模型 ID 2')).toHaveValue('manual-model-id');

      fireEvent.mouseDown(screen.getByRole('combobox', { name: '缓存模型' }));
      expect(await screen.findByText('gpt-4.1-mini')).toBeInTheDocument();

      fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));

      await waitFor(() => {
        expect(submit).toHaveBeenCalledWith({
          display_name: 'OpenAI Production',
          config: expectedConfig,
          configured_models: [
            {
              model_id: 'gpt-4o-mini',
              enabled: true,
              context_window_override_tokens: null,
              supports_multimodal: false
            },
            {
              model_id: 'manual-model-id',
              enabled: false,
              context_window_override_tokens: null,
              supports_multimodal: false
            }
          ],
          included_in_main: true,
          preview_token: 'preview-2'
        });
      });
      expect(previewModels).toHaveBeenNthCalledWith(2, expectedConfig);
    }
  );

  test(
    'hydrates included_in_main from the instance in edit mode and submits it back unchanged',
    { timeout: 15000 },
    async () => {
      const submit = vi.fn().mockResolvedValue(undefined);
      const instance = {
        ...buildSettingsModelProviderInstances()[1],
        included_in_main: false
      };

      render(
        <ModelProviderInstanceDrawer
          open
          mode="edit"
          catalogEntry={modelProviderCatalogEntries[0]}
          instance={instance}
          cachedModelCatalog={null}
          defaultIncludedInMain={true}
          submitting={false}
          onClose={() => undefined}
          onSubmit={submit}
          onPreviewModels={async () => ({
            models: [],
            preview_token: 'preview-1',
            expires_at: '2026-04-22T12:00:00Z'
          })}
          onRevealSecret={async () => 'backup-secret'}
        />
      );

      expect(await screen.findByText('编辑 API 密钥配置')).toBeInTheDocument();
      expect(
        screen.getByRole('switch', { name: '注入主实例' })
      ).not.toBeChecked();

      fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));

      await waitFor(() => {
        expect(submit).toHaveBeenCalledWith(
          expect.objectContaining({
            display_name: 'OpenAI Backup',
            included_in_main: false
          })
        );
      });
    }
  );

  test(
    'parses create-mode context overrides into numeric payloads and blocks invalid values',
    { timeout: 15000 },
    async () => {
      const submit = vi.fn().mockResolvedValue(undefined);

      render(
        <ModelProviderInstanceDrawer
          open
          mode="create"
          catalogEntry={modelProviderCatalogEntries[0]}
          instance={null}
          cachedModelCatalog={null}
          defaultIncludedInMain={true}
          submitting={false}
          onClose={() => undefined}
          onSubmit={submit}
          onPreviewModels={async () => ({
            models: [],
            preview_token: 'preview-1',
            expires_at: '2026-04-22T12:00:00Z'
          })}
          onRevealSecret={async () => 'super-secret'}
        />
      );

      fireEvent.change(await screen.findByLabelText('API Endpoint'), {
        target: { value: 'https://api.openai.com/v1' }
      });
      fireEvent.change(screen.getByLabelText('API Key'), {
        target: { value: 'super-secret' }
      });
      fireEvent.change(screen.getByLabelText('名称'), {
        target: { value: 'OpenAI Draft' }
      });
      fireEvent.click(screen.getByRole('button', { name: '新增' }));
      fireEvent.change(screen.getByLabelText('模型 ID 1'), {
        target: { value: 'gpt-4o-mini' }
      });
      fireEvent.click(screen.getByRole('switch', { name: '启用多模态模型 1' }));
      fireEvent.change(screen.getByLabelText('上下文 1'), {
        target: { value: 'abc' }
      });

      fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));

      await waitFor(() => {
        expect(submit).not.toHaveBeenCalled();
        expect(
          screen.getByText(MODEL_CONTEXT_WINDOW_VALIDATION_MESSAGE)
        ).toBeInTheDocument();
      });

      fireEvent.change(screen.getByLabelText('上下文 1'), {
        target: { value: '200K' }
      });
      fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));

      await waitFor(() => {
        expect(submit).toHaveBeenCalledWith({
          display_name: 'OpenAI Draft',
          config: {
            base_url: 'https://api.openai.com/v1',
            api_key: 'super-secret'
          },
          configured_models: [
            {
              model_id: 'gpt-4o-mini',
              enabled: true,
              context_window_override_tokens: 200000,
              supports_multimodal: true
            }
          ],
          included_in_main: true,
          preview_token: undefined
        });
      });
    }
  );

  test(
    'rehydrates formatted edit-mode context overrides and submits null after clearing',
    { timeout: 15000 },
    async () => {
      const submit = vi.fn().mockResolvedValue(undefined);
      const instance = {
        ...buildSettingsModelProviderInstances()[0],
        configured_models: [
          {
            model_id: 'gpt-4o-mini',
            enabled: true,
            context_window_override_tokens: 16000,
            supports_multimodal: true
          }
        ]
      };

      render(
        <ModelProviderInstanceDrawer
          open
          mode="edit"
          catalogEntry={modelProviderCatalogEntries[0]}
          instance={instance}
          cachedModelCatalog={null}
          defaultIncludedInMain={true}
          submitting={false}
          onClose={() => undefined}
          onSubmit={submit}
          onPreviewModels={async () => ({
            models: [],
            preview_token: 'preview-1',
            expires_at: '2026-04-22T12:00:00Z'
          })}
          onRevealSecret={async () => 'super-secret'}
        />
      );

      expect(await screen.findByLabelText('上下文 1')).toHaveValue('16K');

      fireEvent.change(screen.getByLabelText('上下文 1'), {
        target: { value: '' }
      });
      fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));

      await waitFor(() => {
        expect(submit).toHaveBeenCalledWith(
          expect.objectContaining({
            configured_models: [
              {
                model_id: 'gpt-4o-mini',
                enabled: true,
                context_window_override_tokens: null,
                supports_multimodal: true
              }
            ]
          })
        );
      });
    }
  );
});
