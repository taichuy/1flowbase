import { describe, expect, test } from 'vitest';
import { listLlmProviderOptions } from '../../../lib/model-options';
import { validateDocument } from '../../../lib/validate-document';
import {
  createDefaultAgentFlowDocument,
  modelProviderOptionsContract,
  primaryGroup,
  primaryModel,
  primaryProvider
} from '../support';

describe('validateDocument model providers', () => {
  test('keeps all backend-provided models selectable, including manual entries', () => {
    const options = {
      ...modelProviderOptionsContract,
      providers: [
        {
          ...primaryProvider,
          model_groups: [
            {
              ...primaryGroup,
              model_id: 'gpt-4o-mini',
              model: {
                ...primaryModel,
                model_id: 'gpt-4o-mini',
                display_name: 'GPT-4o Mini'
              }
            },
            {
              ...primaryGroup,
              model_id: 'gpt-4o',
              model: {
                ...primaryModel,
                model_id: 'gpt-4o',
                display_name: 'GPT-4o'
              }
            },
            {
              ...primaryGroup,
              model_id: 'manual-enabled-model',
              model: {
                ...primaryModel,
                model_id: 'manual-enabled-model',
                display_name: '手动启用模型',
                source: 'manual'
              }
            }
          ]
        }
      ]
    };

    expect(
      listLlmProviderOptions(
        options as typeof modelProviderOptionsContract
      )[0]?.models.map((model) => model.value)
    ).toEqual(['gpt-4o-mini', 'gpt-4o', 'manual-enabled-model']);
  });

  test('flags a missing llm model provider selection on the unified field', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    const issues = validateDocument(document);

    expect(issues).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          nodeId: 'node-llm',
          fieldKey: 'config.model_provider',
          title: 'LLM 缺少模型供应商'
        })
      ])
    );
  });

  test('flags unavailable provider code and missing model in provider catalog', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const llmNode = document.graph.nodes.find((node) => node.id === 'node-llm');

    if (!llmNode) {
      throw new Error('expected default LLM node');
    }

    llmNode.config.model_provider = {
      provider_code: 'provider-stale',
      model_id: 'gpt-4.1'
    };

    const issues = validateDocument(document, modelProviderOptionsContract);

    expect(issues).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          nodeId: 'node-llm',
          fieldKey: 'config.model_provider',
          title: 'LLM 模型供应商不可用'
        })
      ])
    );
  });

  test('flags a model that is not in the backend-provided model list', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const llmNode = document.graph.nodes.find((node) => node.id === 'node-llm');

    if (!llmNode) {
      throw new Error('expected default LLM node');
    }

    llmNode.config.model_provider = {
      provider_code: 'openai_compatible',
      model_id: 'missing-model'
    };

    const issues = validateDocument(document, modelProviderOptionsContract);

    expect(issues).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          nodeId: 'node-llm',
          fieldKey: 'config.model_provider',
          title: 'LLM 模型不可用'
        })
      ])
    );
  });

  test('accepts stable llm provider and model selection without source instance', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const llmNode = document.graph.nodes.find((node) => node.id === 'node-llm');

    if (!llmNode) {
      throw new Error('expected default LLM node');
    }

    llmNode.config.model_provider = {
      provider_code: primaryProvider.provider_code,
      model_id: primaryModel.model_id
    };

    const issues = validateDocument(document, modelProviderOptionsContract);

    expect(issues).not.toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          nodeId: 'node-llm',
          fieldKey: 'config.model_provider'
        })
      ])
    );
  });

  test('accepts a stable model exposed by multiple included instances as an ordered group', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const duplicatedContract = JSON.parse(
      JSON.stringify(modelProviderOptionsContract)
    ) as typeof modelProviderOptionsContract;
    const duplicatedProvider = duplicatedContract.providers[0];
    const llmNode = document.graph.nodes.find((node) => node.id === 'node-llm');

    if (!llmNode) {
      throw new Error('expected default LLM node');
    }

    duplicatedProvider.model_groups = [
      {
        model_id: primaryModel.model_id,
        distribution_rule: 'none',
        model: { ...primaryModel },
        targets: [
          {
            source_instance_id: 'provider-openai-prod',
            source_instance_display_name: 'OpenAI Production',
            routing_enabled: true,
            model: { ...primaryModel }
          },
          {
            source_instance_id: 'provider-openai-backup',
            source_instance_display_name: 'OpenAI Backup',
            routing_enabled: true,
            model: { ...primaryModel }
          }
        ]
      }
    ];
    llmNode.config.model_provider = {
      provider_code: primaryProvider.provider_code,
      model_id: primaryModel.model_id
    };

    const issues = validateDocument(document, duplicatedContract);

    expect(issues).not.toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          nodeId: 'node-llm',
          fieldKey: 'config.model_provider'
        })
      ])
    );
  });

  test('keeps the node populated but flags a model that does not exist under the selected provider', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const llmNode = document.graph.nodes.find((node) => node.id === 'node-llm');

    if (!llmNode) {
      throw new Error('expected default LLM node');
    }

    llmNode.config.model_provider = {
      provider_code: primaryProvider.provider_code,
      model_id: 'missing-model'
    };

    const issues = validateDocument(document, modelProviderOptionsContract);

    expect(llmNode.config.model_provider).toEqual(
      expect.objectContaining({
        provider_code: primaryProvider.provider_code,
        model_id: 'missing-model'
      })
    );
    expect(issues).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          nodeId: 'node-llm',
          fieldKey: 'config.model_provider',
          title: 'LLM 模型不可用'
        })
      ])
    );
  });
});
