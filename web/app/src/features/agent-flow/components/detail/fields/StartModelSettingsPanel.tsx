import { DeleteOutlined, PlusOutlined } from '@ant-design/icons';
import { Button, Input, InputNumber, Select, Switch, Typography } from 'antd';
import { useEffect, useState, type RefObject } from 'react';

import type {
  FlowStartModelCapabilities,
  FlowStartModelDescriptor,
  FlowStartModelReasoning
} from '@1flowbase/flow-schema';

import { FloatingSettingsPanel } from '../FloatingSettingsPanel';
import { i18nText } from '../../../../../shared/i18n/text';
import { defaultLlmMaxOutputTokens } from '../../../lib/model-options';

type StartModelSettingsPanelProps = {
  mode: 'create' | 'edit';
  model: FlowStartModelDescriptor;
  triggerRef: RefObject<HTMLElement | null>;
  onChange: (patch: Partial<FlowStartModelDescriptor>) => void;
  onClose: () => void;
  onSave: () => void;
};

type TokenUnit = 'k' | 'm' | 'b';

const DEFAULT_AUTO_COMPACT_PERCENT = 85;

const TOKEN_UNITS: Array<{ value: TokenUnit; label: string; factor: number }> =
  [
    { value: 'k', label: 'k', factor: 1_000 },
    { value: 'm', label: 'm', factor: 1_000_000 },
    { value: 'b', label: 'b', factor: 1_000_000_000 }
  ];

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

function draftEffortList(value: unknown) {
  if (!Array.isArray(value)) {
    return parseEffortList(value);
  }

  return value
    .filter((item): item is string => typeof item === 'string')
    .map((item) => item.trim());
}

function uniqueDraftEffortList(value: unknown) {
  const seen = new Set<string>();

  return draftEffortList(value).filter((item) => {
    if (!item) {
      return true;
    }
    if (seen.has(item)) {
      return false;
    }
    seen.add(item);
    return true;
  });
}

function effortOptions(efforts: string[]) {
  return efforts.map((value) => ({ value, label: value }));
}

function tokenUnitFor(value: number | undefined): TokenUnit {
  if (!value || value <= 0) {
    return 'k';
  }
  if (value % 1_000_000_000 === 0) {
    return 'b';
  }
  if (value % 1_000_000 === 0) {
    return 'm';
  }
  return 'k';
}

function tokenFactor(unit: TokenUnit) {
  return TOKEN_UNITS.find((item) => item.value === unit)?.factor ?? 1_000;
}

function tokenAmount(value: number | undefined, unit: TokenUnit) {
  return value ? value / tokenFactor(unit) : null;
}

function tokenValue(amount: number | null, unit: TokenUnit) {
  return typeof amount === 'number' && Number.isFinite(amount) && amount > 0
    ? Math.round(amount * tokenFactor(unit))
    : undefined;
}

function autoCompactPercent(model: FlowStartModelDescriptor) {
  const base = model.context_window ?? model.max_context_window;
  const limit = model.auto_compact_token_limit;

  if (!base || !limit || base <= 0 || limit <= 0) {
    return null;
  }

  return Math.round((limit / base) * 10_000) / 100;
}

function autoCompactLimitFromPercent(
  percent: number | null,
  model: FlowStartModelDescriptor
) {
  const base = model.context_window ?? model.max_context_window;

  if (
    typeof percent !== 'number' ||
    !Number.isFinite(percent) ||
    percent <= 0 ||
    !base ||
    base <= 0
  ) {
    return undefined;
  }

  return Math.round((base * percent) / 100);
}

function defaultAutoCompactTokenLimit(contextWindow: number) {
  return Math.round((contextWindow * DEFAULT_AUTO_COMPACT_PERCENT) / 100);
}

function TokenAmountInput({
  label,
  unitLabel,
  placeholder,
  value,
  onChange
}: {
  label: string;
  unitLabel: string;
  placeholder: string;
  value: number | undefined;
  onChange: (value: number | undefined) => void;
}) {
  const [unit, setUnit] = useState<TokenUnit>(() => tokenUnitFor(value));

  useEffect(() => {
    if (value) {
      setUnit(tokenUnitFor(value));
    }
  }, [value]);

  const amount = tokenAmount(value, unit);

  return (
    <div className="agent-flow-start-model-settings__token-row">
      <InputNumber
        aria-label={label}
        min={0.001}
        placeholder={placeholder}
        value={amount}
        onChange={(nextAmount) =>
          onChange(
            tokenValue(typeof nextAmount === 'number' ? nextAmount : null, unit)
          )
        }
      />
      <Select
        aria-label={unitLabel}
        options={TOKEN_UNITS}
        value={unit}
        virtual={false}
        onChange={(nextUnit: TokenUnit) => {
          setUnit(nextUnit);
          onChange(tokenValue(amount, nextUnit));
        }}
      />
    </div>
  );
}

export function StartModelSettingsPanel({
  mode,
  model,
  triggerRef,
  onChange,
  onClose,
  onSave
}: StartModelSettingsPanelProps) {
  const title =
    mode === 'create'
      ? i18nText('agentFlow', 'auto.add_new_model')
      : i18nText('agentFlow', 'auto.edit_model');
  const contextWindowExceeded = Boolean(
    model.context_window &&
    model.max_context_window &&
    model.context_window > model.max_context_window
  );
  const canSave = model.id.trim().length > 0 && !contextWindowExceeded;

  function patchCapabilities(patch: Partial<FlowStartModelCapabilities>) {
    onChange({
      capabilities: {
        ...(model.capabilities ?? {}),
        ...patch
      }
    });
  }

  function patchReasoning(patch: Partial<FlowStartModelReasoning>) {
    onChange({
      reasoning: {
        ...(model.reasoning ?? {}),
        ...patch
      }
    });
  }

  function patchSupportedEfforts(supported_efforts: string[]) {
    const efforts = uniqueDraftEffortList(supported_efforts);
    const selectableEfforts = efforts.filter(Boolean);
    const currentDefault = model.reasoning?.default_effort;

    patchReasoning({
      supported_efforts: efforts,
      default_effort:
        currentDefault && selectableEfforts.includes(currentDefault)
          ? currentDefault
          : selectableEfforts[0]
    });
  }

  function updateSupportedEffort(index: number, effort: string) {
    const efforts = uniqueDraftEffortList(model.reasoning?.supported_efforts);
    efforts[index] = effort.trim();
    patchSupportedEfforts(efforts);
  }

  function removeSupportedEffort(index: number) {
    patchSupportedEfforts(
      uniqueDraftEffortList(model.reasoning?.supported_efforts).filter(
        (_, effortIndex) => effortIndex !== index
      )
    );
  }

  function updateContextWindow(context_window: number | undefined) {
    const oldContextWindow = model.context_window;
    const outputWasLinked =
      oldContextWindow !== undefined &&
      model.max_output_tokens === defaultLlmMaxOutputTokens(oldContextWindow);
    const compactWasLinked =
      oldContextWindow !== undefined &&
      model.auto_compact_token_limit ===
        defaultAutoCompactTokenLimit(oldContextWindow);

    onChange({
      context_window,
      max_output_tokens:
        outputWasLinked && context_window !== undefined
          ? defaultLlmMaxOutputTokens(context_window)
          : model.max_output_tokens,
      auto_compact_token_limit:
        compactWasLinked && context_window !== undefined
          ? defaultAutoCompactTokenLimit(context_window)
          : model.auto_compact_token_limit
    });
  }

  function updateMaxContextWindow(max_context_window: number | undefined) {
    const oldContextWindow = model.context_window;
    const contextWasLinked =
      oldContextWindow !== undefined &&
      oldContextWindow === model.max_context_window;
    const context_window = contextWasLinked
      ? max_context_window
      : oldContextWindow;
    const outputWasLinked =
      oldContextWindow !== undefined &&
      model.max_output_tokens === defaultLlmMaxOutputTokens(oldContextWindow);
    const compactWasLinked =
      oldContextWindow !== undefined &&
      model.auto_compact_token_limit ===
        defaultAutoCompactTokenLimit(oldContextWindow);

    onChange({
      max_context_window,
      context_window,
      max_output_tokens:
        outputWasLinked && context_window !== undefined
          ? defaultLlmMaxOutputTokens(context_window)
          : model.max_output_tokens,
      auto_compact_token_limit:
        compactWasLinked && context_window !== undefined
          ? defaultAutoCompactTokenLimit(context_window)
          : model.auto_compact_token_limit
    });
  }

  const supportedEfforts = uniqueDraftEffortList(
    model.reasoning?.supported_efforts
  );
  const selectableSupportedEfforts = supportedEfforts.filter(Boolean);
  const defaultEffortOptions = effortOptions(selectableSupportedEfforts);

  return (
    <FloatingSettingsPanel
      open
      title={title}
      closeLabel={i18nText('agentFlow', 'auto.close', { value1: title })}
      triggerRef={triggerRef}
      className="agent-flow-start-input-fields__panel"
      defaultWidth={480}
      minWidth={400}
      initialHeight={640}
      gap={16}
      onClose={onClose}
      footer={
        <div className="agent-flow-start-input-fields__panel-footer">
          <Button onClick={onClose}>
            {i18nText('agentFlow', 'auto.cancel')}
          </Button>
          <Button
            aria-label={i18nText('agentFlow', 'auto.save_model')}
            disabled={!canSave}
            type="primary"
            onClick={onSave}
          >
            {i18nText('agentFlow', 'auto.save')}
          </Button>
        </div>
      }
    >
      <div className="agent-flow-start-input-fields__form">
        <div className="agent-flow-start-input-fields__form-row">
          <span>{i18nText('agentFlow', 'auto.model_id_label')}</span>
          <Input
            aria-label={i18nText('agentFlow', 'auto.model_id_input')}
            autoFocus
            placeholder="model-id"
            value={model.id}
            onChange={(event) => {
              const id = event.target.value;
              onChange(model.name === model.id ? { id, name: id } : { id });
            }}
          />
        </div>
        <div className="agent-flow-start-input-fields__form-row">
          <span>{i18nText('agentFlow', 'auto.model_name_label')}</span>
          <Input
            aria-label={i18nText('agentFlow', 'auto.model_name_input')}
            placeholder="Display name"
            value={model.name ?? ''}
            onChange={(event) =>
              onChange({ name: event.target.value || undefined })
            }
          />
        </div>
        <div className="agent-flow-start-input-fields__form-row">
          <span>{i18nText('agentFlow', 'auto.max_context_window')}</span>
          <TokenAmountInput
            label={i18nText('agentFlow', 'auto.model_max_context_window_input')}
            unitLabel={i18nText(
              'agentFlow',
              'auto.model_max_context_window_unit'
            )}
            placeholder="128"
            value={model.max_context_window}
            onChange={updateMaxContextWindow}
          />
        </div>
        <div className="agent-flow-start-input-fields__form-row">
          <span>{i18nText('agentFlow', 'auto.context_window')}</span>
          <TokenAmountInput
            label={i18nText('agentFlow', 'auto.model_context_window_input')}
            unitLabel={i18nText('agentFlow', 'auto.model_context_window_unit')}
            placeholder="128"
            value={model.context_window}
            onChange={updateContextWindow}
          />
          {contextWindowExceeded ? (
            <Typography.Text type="danger">
              {i18nText('agentFlow', 'auto.context_window_exceeds_max')}
            </Typography.Text>
          ) : null}
        </div>
        <div className="agent-flow-start-input-fields__form-row">
          <span>{i18nText('agentFlow', 'auto.max_output_tokens')}</span>
          <TokenAmountInput
            label={i18nText('agentFlow', 'auto.model_max_output_tokens_input')}
            unitLabel={i18nText(
              'agentFlow',
              'auto.model_max_output_tokens_unit'
            )}
            placeholder="32"
            value={model.max_output_tokens}
            onChange={(max_output_tokens) => onChange({ max_output_tokens })}
          />
        </div>
        <div className="agent-flow-start-input-fields__form-row">
          <span>{i18nText('agentFlow', 'auto.auto_compact_percent')}</span>
          <InputNumber
            aria-label={i18nText(
              'agentFlow',
              'auto.auto_compact_percent_input'
            )}
            max={100}
            min={1}
            placeholder="85"
            suffix="%"
            value={autoCompactPercent(model)}
            onChange={(percent) =>
              onChange({
                auto_compact_token_limit: autoCompactLimitFromPercent(
                  typeof percent === 'number' ? percent : null,
                  model
                )
              })
            }
          />
        </div>
        <div className="agent-flow-start-input-fields__toggles">
          <Typography.Text strong>
            {i18nText('agentFlow', 'auto.model_capabilities')}
          </Typography.Text>
          <label className="agent-flow-start-input-fields__toggle-row">
            <span>
              <Typography.Text strong>
                {i18nText('agentFlow', 'auto.reasoning')}
              </Typography.Text>
            </span>
            <Switch
              aria-label={i18nText(
                'agentFlow',
                'auto.model_reasoning_capability'
              )}
              checked={model.capabilities?.reasoning === true}
              onChange={(reasoning) => patchCapabilities({ reasoning })}
            />
          </label>
          <label className="agent-flow-start-input-fields__toggle-row">
            <span>
              <Typography.Text strong>
                {i18nText('agentFlow', 'auto.tool_call')}
              </Typography.Text>
            </span>
            <Switch
              aria-label={i18nText(
                'agentFlow',
                'auto.model_tool_call_capability'
              )}
              checked={model.capabilities?.tool_call === true}
              onChange={(tool_call) => patchCapabilities({ tool_call })}
            />
          </label>
          <label className="agent-flow-start-input-fields__toggle-row">
            <span>
              <Typography.Text strong>
                {i18nText('agentFlow', 'auto.multimodal')}
              </Typography.Text>
            </span>
            <Switch
              aria-label={i18nText(
                'agentFlow',
                'auto.model_multimodal_capability'
              )}
              checked={model.capabilities?.multimodal === true}
              onChange={(multimodal) => patchCapabilities({ multimodal })}
            />
          </label>
          <label className="agent-flow-start-input-fields__toggle-row">
            <span>
              <Typography.Text strong>
                {i18nText('agentFlow', 'auto.structured_output')}
              </Typography.Text>
            </span>
            <Switch
              aria-label={i18nText(
                'agentFlow',
                'auto.model_structured_output_capability'
              )}
              checked={model.capabilities?.structured_output === true}
              onChange={(structured_output) =>
                patchCapabilities({ structured_output })
              }
            />
          </label>
        </div>
        <div className="agent-flow-start-input-fields__form-row">
          <span>{i18nText('agentFlow', 'auto.default_reasoning_effort')}</span>
          <Select
            allowClear
            aria-label={i18nText(
              'agentFlow',
              'auto.model_default_reasoning_effort_input'
            )}
            options={defaultEffortOptions}
            value={
              model.reasoning?.default_effort &&
              selectableSupportedEfforts.includes(
                model.reasoning.default_effort
              )
                ? model.reasoning.default_effort
                : undefined
            }
            virtual={false}
            onChange={(default_effort?: string) =>
              patchReasoning({ default_effort })
            }
          />
        </div>
        <div className="agent-flow-start-input-fields__form-row">
          <span>
            {i18nText('agentFlow', 'auto.supported_reasoning_efforts')}
          </span>
          <div className="agent-flow-start-input-fields__option-list">
            {supportedEfforts.map((effort, index) => (
              <div
                className="agent-flow-start-input-fields__option-row"
                key={`supported-effort-${index}`}
              >
                <Input
                  aria-label={i18nText(
                    'agentFlow',
                    'auto.model_supported_reasoning_effort_input',
                    { value1: index + 1 }
                  )}
                  placeholder="medium"
                  value={effort}
                  onChange={(event) =>
                    updateSupportedEffort(index, event.target.value)
                  }
                />
                <Button
                  aria-label={i18nText(
                    'agentFlow',
                    'auto.remove_supported_reasoning_effort',
                    { value1: index + 1 }
                  )}
                  icon={<DeleteOutlined />}
                  size="small"
                  type="text"
                  onClick={() => removeSupportedEffort(index)}
                />
              </div>
            ))}
            <Button
              aria-label={i18nText(
                'agentFlow',
                'auto.add_supported_reasoning_effort'
              )}
              icon={<PlusOutlined />}
              size="small"
              onClick={() => patchSupportedEfforts([...supportedEfforts, ''])}
            >
              {i18nText('agentFlow', 'auto.add_items')}
            </Button>
          </div>
        </div>
      </div>
    </FloatingSettingsPanel>
  );
}
