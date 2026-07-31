import { DeleteOutlined, PlusOutlined } from '@ant-design/icons';
import { Button, Empty } from 'antd';
import { useRef, useState } from 'react';

import type {
  FlowStartModelCapabilities,
  FlowStartModelDescriptor,
  FlowStartModelReasoning
} from '@1flowbase/flow-schema';
import { formatLlmTokenCount } from '../../../lib/model-options';
import { i18nText } from '../../../../../shared/i18n/text';
import { StartModelSettingsPanel } from './StartModelSettingsPanel';

const DEFAULT_REASONING_EFFORT = 'medium';
const DEFAULT_REASONING_EFFORTS = ['minimal', 'low', 'medium', 'high', 'xhigh'];
const DEFAULT_START_MODEL_ID = 'flowbase';
const DEFAULT_START_MODEL_CONTEXT_WINDOW = 257_000;
const DEFAULT_START_MODEL_MAX_CONTEXT_WINDOW = 128_000;
const DEFAULT_START_MODEL_MAX_OUTPUT_TOKENS = 32_000;
const DEFAULT_START_MODEL_AUTO_COMPACT_PERCENT = 85;

type EditingModel = {
  index: number | null;
  model: FlowStartModelDescriptor;
};

function normalizeModelList(value: unknown): FlowStartModelDescriptor[] {
  if (!Array.isArray(value)) {
    return [];
  }

  return value
    .map((item) => {
      if (typeof item === 'string') {
        return { id: item };
      }
      if (typeof item !== 'object' || item === null) {
        return null;
      }
      const source = item as Record<string, unknown>;
      const id = typeof source.id === 'string' ? source.id : '';
      const name = typeof source.name === 'string' ? source.name : undefined;
      const contextWindow = normalizeTokenNumber(source.context_window);
      const maxContextWindow = normalizeTokenNumber(source.max_context_window);
      const maxOutputTokens = normalizeTokenNumber(source.max_output_tokens);
      const autoCompactTokenLimit = normalizeTokenNumber(
        source.auto_compact_token_limit
      );
      const capabilities = normalizeCapabilities(source.capabilities);
      const reasoning = normalizeReasoning(source.reasoning);

      return {
        id,
        ...(name ? { name } : {}),
        ...(contextWindow ? { context_window: contextWindow } : {}),
        ...(maxContextWindow ? { max_context_window: maxContextWindow } : {}),
        ...(maxOutputTokens ? { max_output_tokens: maxOutputTokens } : {}),
        ...(autoCompactTokenLimit
          ? { auto_compact_token_limit: autoCompactTokenLimit }
          : {}),
        ...(hasCapabilities(capabilities) ? { capabilities } : {}),
        ...(hasReasoning(reasoning) ? { reasoning } : {})
      };
    })
    .filter((item): item is FlowStartModelDescriptor => item !== null);
}

function normalizeTokenNumber(value: unknown) {
  return typeof value === 'number' && Number.isInteger(value) && value > 0
    ? value
    : undefined;
}

function normalizeCapabilities(value: unknown): FlowStartModelCapabilities {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return {};
  }

  const source = value as Record<string, unknown>;
  return {
    ...(source.reasoning === true ? { reasoning: true } : {}),
    ...(source.tool_call === true ? { tool_call: true } : {}),
    ...(source.multimodal === true ? { multimodal: true } : {}),
    ...(source.structured_output === true ? { structured_output: true } : {})
  };
}

function withDefaultCapabilities(
  capabilities: FlowStartModelCapabilities
): FlowStartModelCapabilities {
  return {
    ...capabilities,
    reasoning: true,
    tool_call: true,
    multimodal: true,
    structured_output: true
  };
}

function normalizeReasoning(value: unknown): FlowStartModelReasoning {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return {};
  }

  const source = value as Record<string, unknown>;
  const defaultEffort =
    typeof source.default_effort === 'string' &&
    source.default_effort.trim().length > 0
      ? source.default_effort.trim()
      : undefined;
  const supportedEfforts = parseEffortList(source.supported_efforts);

  return {
    ...(defaultEffort ? { default_effort: defaultEffort } : {}),
    ...(supportedEfforts.length ? { supported_efforts: supportedEfforts } : {})
  };
}

function parseEffortList(value: unknown) {
  if (typeof value === 'string') {
    return value
      .split(',')
      .map((item) => item.trim())
      .filter(Boolean);
  }
  if (!Array.isArray(value)) {
    return [];
  }
  return value
    .filter((item): item is string => typeof item === 'string')
    .map((item) => item.trim())
    .filter(Boolean);
}

function hasCapabilities(capabilities: FlowStartModelCapabilities) {
  return Boolean(
    capabilities.reasoning ||
    capabilities.tool_call ||
    capabilities.multimodal ||
    capabilities.structured_output
  );
}

function hasReasoning(reasoning: FlowStartModelReasoning) {
  return Boolean(
    reasoning.default_effort || reasoning.supported_efforts?.length
  );
}

function withDefaultReasoning(
  reasoning: FlowStartModelReasoning
): FlowStartModelReasoning {
  const supportedEfforts = reasoning.supported_efforts?.length
    ? reasoning.supported_efforts
    : DEFAULT_REASONING_EFFORTS;

  return {
    default_effort: reasoning.default_effort ?? DEFAULT_REASONING_EFFORT,
    supported_efforts: supportedEfforts
  };
}

function cleanModel(row: FlowStartModelDescriptor): FlowStartModelDescriptor {
  const contextWindow = normalizeTokenNumber(row.context_window);
  const maxContextWindow = normalizeTokenNumber(row.max_context_window);
  const capabilities = normalizeCapabilities(row.capabilities);
  const reasoning = normalizeReasoning(row.reasoning);

  return {
    id: row.id.trim(),
    ...(row.name?.trim() ? { name: row.name.trim() } : {}),
    ...(contextWindow ? { context_window: contextWindow } : {}),
    ...(maxContextWindow ? { max_context_window: maxContextWindow } : {}),
    ...(normalizeTokenNumber(row.max_output_tokens)
      ? { max_output_tokens: row.max_output_tokens }
      : {}),
    ...(normalizeTokenNumber(row.auto_compact_token_limit)
      ? { auto_compact_token_limit: row.auto_compact_token_limit }
      : {}),
    ...(hasCapabilities(capabilities) ? { capabilities } : {}),
    ...(hasReasoning(reasoning) ? { reasoning } : {})
  };
}

function cleanRows(rows: FlowStartModelDescriptor[]) {
  return rows.map(cleanModel).filter((row) => row.id.length > 0);
}

function createNextModel(): FlowStartModelDescriptor {
  return {
    id: DEFAULT_START_MODEL_ID,
    name: DEFAULT_START_MODEL_ID,
    context_window: DEFAULT_START_MODEL_CONTEXT_WINDOW,
    max_context_window: DEFAULT_START_MODEL_MAX_CONTEXT_WINDOW,
    max_output_tokens: DEFAULT_START_MODEL_MAX_OUTPUT_TOKENS,
    auto_compact_token_limit: Math.round(
      (DEFAULT_START_MODEL_CONTEXT_WINDOW *
        DEFAULT_START_MODEL_AUTO_COMPACT_PERCENT) /
        100
    ),
    capabilities: withDefaultCapabilities({}),
    reasoning: withDefaultReasoning({})
  };
}

function modelContext(row: FlowStartModelDescriptor) {
  return row.context_window ?? row.max_context_window ?? null;
}

function formatModelContext(row: FlowStartModelDescriptor) {
  return (
    formatLlmTokenCount(modelContext(row)) ??
    i18nText('agentFlow', 'auto.not_set')
  );
}

function replaceAt(
  rows: FlowStartModelDescriptor[],
  index: number,
  model: FlowStartModelDescriptor
) {
  return rows.map((row, rowIndex) => (rowIndex === index ? model : row));
}

export function StartModelListField({
  value,
  onChange
}: {
  value: unknown;
  onChange: (value: FlowStartModelDescriptor[]) => void;
}) {
  const rows = normalizeModelList(value);
  const [editing, setEditing] = useState<EditingModel | null>(null);
  const triggerRef = useRef<HTMLButtonElement | null>(null);

  function openAddPanel() {
    setEditing({
      index: null,
      model: createNextModel()
    });
  }

  function openEditPanel(model: FlowStartModelDescriptor, index: number) {
    setEditing({
      index,
      model: cleanModel(model)
    });
  }

  function closePanel() {
    setEditing(null);
  }

  function updateDraft(patch: Partial<FlowStartModelDescriptor>) {
    setEditing((current) =>
      current
        ? {
            ...current,
            model: {
              ...current.model,
              ...patch
            }
          }
        : current
    );
  }

  function saveDraft() {
    if (!editing) {
      return;
    }

    const nextModel = cleanModel(editing.model);

    if (!nextModel.id) {
      return;
    }

    if (editing.index === null) {
      onChange([...cleanRows(rows), nextModel]);
    } else {
      onChange(cleanRows(replaceAt(rows, editing.index, nextModel)));
    }

    closePanel();
  }

  const floatingPanel = editing ? (
    <StartModelSettingsPanel
      mode={editing.index === null ? 'create' : 'edit'}
      model={editing.model}
      triggerRef={triggerRef}
      onChange={updateDraft}
      onClose={closePanel}
      onSave={saveDraft}
    />
  ) : null;

  return (
    <div className="agent-flow-start-model-list">
      <div className="agent-flow-start-input-fields__header">
        <Button
          aria-label={i18nText('agentFlow', 'auto.add_new_model')}
          icon={<PlusOutlined />}
          size="small"
          type="text"
          onClick={openAddPanel}
          ref={triggerRef}
        />
      </div>
      {rows.length > 0 ? (
        <div className="agent-flow-start-input-fields__list">
          {rows.map((row, index) => (
            <div
              className="agent-flow-start-model-list__item agent-flow-node-detail__list-item"
              data-testid={`start-model-row-${index + 1}`}
              key={`${row.id}-${index}`}
            >
              <button
                aria-label={i18nText('agentFlow', 'auto.edit_model')}
                className="agent-flow-start-model-list__main"
                type="button"
                onClick={() => openEditPanel(row, index)}
              >
                <span className="agent-flow-node-detail__list-item-left">
                  <span className="agent-flow-node-detail__list-item-name">
                    {row.id.trim() || 'model-id'}
                  </span>
                </span>
                <span className="agent-flow-start-input-fields__item-meta">
                  <span className="agent-flow-node-detail__list-item-type">
                    {formatModelContext(row)}
                  </span>
                </span>
              </button>
              <Button
                aria-label={i18nText('agentFlow', 'auto.delete_model', {
                  value1: index + 1
                })}
                className="agent-flow-start-model-list__delete"
                danger
                icon={<DeleteOutlined />}
                size="small"
                type="text"
                onClick={() =>
                  onChange(rows.filter((_, rowIndex) => rowIndex !== index))
                }
              />
            </div>
          ))}
        </div>
      ) : (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={i18nText('agentFlow', 'auto.no_model_yet')}
        />
      )}
      {floatingPanel}
    </div>
  );
}
