import { QuestionCircleOutlined, ReloadOutlined } from '@ant-design/icons';
import { useQuery } from '@tanstack/react-query';
import {
  Alert,
  Button,
  ConfigProvider,
  Empty,
  Input,
  InputNumber,
  Select,
  Slider,
  Switch,
  Tooltip,
  Typography
} from 'antd';
import type { InputNumberProps, ThemeConfig } from 'antd';
import { useEffect, useMemo, useState } from 'react';

import type { SchemaDynamicFormRendererProps } from '../../../../../shared/schema-ui/v1/registry/create-renderer-registry';
import {
  fetchModelProviderOptions,
  modelProviderOptionsQueryKey
} from '../../../api/model-provider-options';
import {
  DEFAULT_LLM_PARAMETERS,
  getLlmParameterDefaultValue,
  getLlmModelProvider,
  getLlmParameters,
  type LlmNodeParameters
} from '../../../lib/llm-node-config';
import {
  findLlmModelOption,
  findLlmProviderOption
} from '../../../lib/model-options';
import { i18nText } from '../../../../../shared/i18n/text';

type LlmParameterField = NonNullable<
  NonNullable<ReturnType<typeof findLlmProviderOption>>['parameterForm']
>['fields'][number];

const DEFAULT_CONTEXT_WINDOW_TOKENS = 16_000;
const NUMERIC_CONTROL_THEME: ThemeConfig = {
  token: {
    colorPrimary: '#1677ff',
    colorInfo: '#1677ff'
  },
  components: {
    Slider: {
      trackBg: '#91caff',
      trackHoverBg: '#69b1ff',
      handleColor: '#1677ff',
      handleActiveColor: '#0958d9',
      handleActiveOutlineColor: 'rgba(5, 145, 255, 0.12)'
    },
    InputNumber: {
      hoverBorderColor: '#4096ff',
      activeBorderColor: '#1677ff',
      activeShadow: '0 0 0 2px rgba(5, 145, 255, 0.1)'
    }
  }
};

function getNodeConfig(adapter: SchemaDynamicFormRendererProps['adapter']) {
  const node = adapter.getDerived('node') as
    | { config?: Record<string, unknown> }
    | null
    | undefined;
  return node?.config ?? {};
}

function getFieldValue(parameters: LlmNodeParameters, key: string) {
  return parameters.items[key]?.value;
}

function getFieldEnabled(
  parameters: LlmNodeParameters,
  key: string,
  alwaysEnabled: boolean
) {
  return alwaysEnabled ? true : Boolean(parameters.items[key]?.enabled);
}

function updateParameters(
  adapter: SchemaDynamicFormRendererProps['adapter'],
  nextParameters: LlmNodeParameters
) {
  adapter.setValue('config.llm_parameters', nextParameters);
}

function getNumericDefaultValue(field: LlmParameterField) {
  const defaultValue = getLlmParameterDefaultValue(field);

  return typeof defaultValue === 'number' && Number.isFinite(defaultValue)
    ? defaultValue
    : 0;
}

function getNumericValue(field: LlmParameterField, value: unknown) {
  return typeof value === 'number' && Number.isFinite(value)
    ? value
    : getNumericDefaultValue(field);
}

function clampNumericValue(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}

function formatNumericInputValue(value: unknown) {
  if (typeof value === 'number') {
    return Number.isFinite(value) ? String(value) : '';
  }

  if (typeof value === 'string') {
    return value.endsWith('.') ? value.slice(0, -1) : value;
  }

  return '';
}

function isMaxTokenField(field: LlmParameterField) {
  const normalizedKey = field.key.toLowerCase();

  return normalizedKey.includes('max') && normalizedKey.includes('token');
}

function getContextBoundedMax(
  field: LlmParameterField,
  contextWindow: number | null | undefined
) {
  if (!isMaxTokenField(field)) {
    return null;
  }

  const contextMax =
    typeof contextWindow === 'number' && Number.isFinite(contextWindow)
      ? contextWindow
      : DEFAULT_CONTEXT_WINDOW_TOKENS;

  return typeof field.max === 'number'
    ? Math.min(field.max, contextMax)
    : contextMax;
}

function getSliderBounds(
  field: LlmParameterField,
  currentValue: number,
  contextWindow: number | null | undefined
) {
  const min = typeof field.min === 'number' ? field.min : 0;
  const contextBoundedMax = getContextBoundedMax(field, contextWindow);
  const inferredMax =
    field.type === 'integer'
      ? Math.max(1, currentValue, getNumericDefaultValue(field), 4096)
      : Math.max(1, currentValue, getNumericDefaultValue(field));
  const max =
    contextBoundedMax ??
    (typeof field.max === 'number' ? field.max : inferredMax);

  return {
    min,
    max: max > min ? max : min + 1
  };
}

function LlmNumericControl({
  field,
  value,
  contextWindow,
  nextParameters,
  restoreDefaultValue
}: {
  field: LlmParameterField;
  value: unknown;
  contextWindow: number | null | undefined;
  nextParameters: (nextValue: unknown) => void;
  restoreDefaultValue: () => void;
}) {
  const rawNumericValue = getNumericValue(field, value);
  const { min, max } = getSliderBounds(field, rawNumericValue, contextWindow);
  const numericValue = clampNumericValue(rawNumericValue, min, max);
  const step = field.step ?? (field.type === 'integer' ? 1 : 0.1);
  const [draftValue, setDraftValue] = useState(numericValue);

  useEffect(() => {
    setDraftValue(numericValue);
  }, [numericValue]);

  function normalizeNextValue(nextValue: number) {
    return clampNumericValue(nextValue, min, max);
  }

  const onChange: InputNumberProps['onChange'] = (next) => {
    if (typeof next !== 'number' || Number.isNaN(next)) {
      return;
    }

    setDraftValue(normalizeNextValue(next));
  };

  return (
    <ConfigProvider theme={NUMERIC_CONTROL_THEME}>
      <div className="agent-flow-llm-parameter-form__numeric-control">
        <Slider
          min={min}
          max={max}
          step={step}
          value={typeof draftValue === 'number' ? draftValue : 0}
          onChange={onChange}
          onChangeComplete={(next) => {
            const nextValue = Array.isArray(next) ? (next[0] ?? min) : next;

            nextParameters(normalizeNextValue(nextValue));
          }}
        />
        <div className="agent-flow-llm-parameter-form__number-actions">
          <InputNumber
            aria-label={i18nText("agentFlow", "auto.current_value", { value1: field.label })}
            min={min}
            max={max}
            step={step}
            precision={field.precision}
            value={draftValue}
            formatter={(next) => formatNumericInputValue(next)}
            onChange={onChange}
            onBlur={() => {
              nextParameters(draftValue);
            }}
            onPressEnter={() => {
              nextParameters(draftValue);
            }}
          />
          <Tooltip title={i18nText("agentFlow", "auto.restore_defaults")}>
            <Button
              type="text"
              size="small"
              aria-label={i18nText("agentFlow", "auto.restore_default_value", { value1: field.label })}
              className="agent-flow-llm-parameter-form__default-icon"
              icon={<ReloadOutlined />}
              onClick={restoreDefaultValue}
            />
          </Tooltip>
        </div>
      </div>
    </ConfigProvider>
  );
}

function renderFieldControl({
  field,
  value,
  contextWindow,
  nextParameters,
  restoreDefaultValue
}: {
  field: LlmParameterField;
  value: unknown;
  contextWindow: number | null | undefined;
  nextParameters: (nextValue: unknown) => void;
  restoreDefaultValue: () => void;
}) {
  if (
    field.control === 'slider' ||
    field.type === 'integer' ||
    field.type === 'number' ||
    field.control === 'number'
  ) {
    return (
      <LlmNumericControl
        field={field}
        value={value}
        contextWindow={contextWindow}
        nextParameters={nextParameters}
        restoreDefaultValue={restoreDefaultValue}
      />
    );
  }

  if (field.control === 'switch' || field.type === 'boolean') {
    return (
      <Switch
        checked={Boolean(value)}
        onChange={(checked) => nextParameters(checked)}
      />
    );
  }

  if (field.control === 'select' || field.type === 'enum') {
    return (
      <Select
        style={{ width: '100%' }}
        value={value as string | number | boolean | undefined}
        options={(field.options ?? []).map((option) => ({
          label: option.label,
          value: option.value
        }))}
        onChange={(next) => nextParameters(next)}
      />
    );
  }

  if (field.control === 'textarea') {
    return (
      <Input.TextArea
        rows={4}
        value={typeof value === 'string' ? value : String(value ?? '')}
        placeholder={field.placeholder}
        onChange={(event) => nextParameters(event.target.value)}
      />
    );
  }

  if (field.control === 'json_editor' || field.type === 'json') {
    return (
      <Input.TextArea
        rows={6}
        value={
          typeof value === 'string'
            ? value
            : JSON.stringify(value ?? field.default_value ?? {}, null, 2)
        }
        placeholder={field.placeholder}
        onChange={(event) => nextParameters(event.target.value)}
      />
    );
  }

  return (
    <Input
      value={typeof value === 'string' ? value : String(value ?? '')}
      placeholder={field.placeholder}
      onChange={(event) => nextParameters(event.target.value)}
    />
  );
}

export function LlmParameterForm({
  adapter,
  block
}: SchemaDynamicFormRendererProps) {
  const providerOptionsQuery = useQuery({
    queryKey: modelProviderOptionsQueryKey,
    queryFn: fetchModelProviderOptions,
    staleTime: 60_000
  });
  const config = getNodeConfig(adapter);
  const modelProvider = getLlmModelProvider(config);
  const parameters = getLlmParameters(config);
  const selectedProvider = findLlmProviderOption(
    providerOptionsQuery.data,
    modelProvider.provider_code
  );
  const selectedModel = findLlmModelOption(
    providerOptionsQuery.data,
    modelProvider.provider_code,
    modelProvider.model_id
  );
  const parameterForm = selectedProvider?.parameterForm ?? null;
  const contextWindow = selectedModel?.effectiveContextWindow ?? null;

  const groupedFields = useMemo(() => {
    if (!parameterForm) {
      return [];
    }

    const sortedFields = [...parameterForm.fields].sort(
      (left, right) => (left.order ?? 0) - (right.order ?? 0)
    );
    const groups = new Map<string, typeof sortedFields>();

    for (const field of sortedFields) {
      const key = field.group || 'general';
      const group = groups.get(key) ?? [];
      group.push(field);
      groups.set(key, group);
    }

    return [...groups.entries()];
  }, [parameterForm]);

  if (providerOptionsQuery.isPending) {
    return (
      <Typography.Text type="secondary">{i18nText("agentFlow", "auto.loading_parameter_schema")}</Typography.Text>
    );
  }

  if (providerOptionsQuery.isError) {
    return <Alert type="error" showIcon message={i18nText("agentFlow", "auto.parameter_schema_failed_load")} />;
  }

  if (!modelProvider.model_id) {
    return (
      <Empty
        image={Empty.PRESENTED_IMAGE_SIMPLE}
        description={block.empty_text ?? i18nText("agentFlow", "auto.select_model_adjusting_llm_parameters")}
      />
    );
  }

  if (!selectedProvider) {
    return (
      <Alert
        type="warning"
        showIcon
        message={i18nText("agentFlow", "auto.model_provider_unavailable_parameter_form_rendered")}
      />
    );
  }

  if (!selectedModel) {
    return (
      <Alert
        type="warning"
        showIcon
        message={i18nText("agentFlow", "auto.model_unavailable_parameter_form_rendered")}
      />
    );
  }

  if (!parameterForm || parameterForm.fields.length === 0) {
    return (
      <Empty
        image={Empty.PRESENTED_IMAGE_SIMPLE}
        description={i18nText("agentFlow", "auto.provider_tunable_parameters")}
      />
    );
  }

  return (
    <div className="agent-flow-llm-parameter-form">
      {groupedFields.map(([group, fields]) => (
        <div key={group} className="agent-flow-llm-parameter-form__group">
          {group !== 'general' ? (
            <Typography.Text
              type="secondary"
              className="agent-flow-llm-parameter-form__group-title"
            >
              {group}
            </Typography.Text>
          ) : null}
          <div className="agent-flow-llm-parameter-form__rows">
            {fields.map((field) => {
              const alwaysEnabled = field.send_mode === 'always';
              const enabled = getFieldEnabled(
                parameters,
                field.key,
                alwaysEnabled
              );
              const value = getFieldValue(parameters, field.key);
              const defaultValue = getLlmParameterDefaultValue(field);
              const nextParameters = (
                nextValue: unknown,
                nextEnabled = enabled
              ) =>
                updateParameters(adapter, {
                  schema_version: parameterForm.schema_version,
                  items: {
                    ...parameters.items,
                    [field.key]: {
                      enabled: alwaysEnabled ? true : nextEnabled,
                      value: nextValue
                    }
                  }
                });

              return (
                <div
                  key={field.key}
                  className="agent-flow-llm-parameter-form__row"
                >
                  <div className="agent-flow-llm-parameter-form__row-head">
                    <div className="agent-flow-llm-parameter-form__row-label">
                      <span className="agent-flow-llm-parameter-form__label-line">
                        <Typography.Text strong>{field.label}</Typography.Text>
                        {field.description ? (
                          <Tooltip title={field.description}>
                            <QuestionCircleOutlined
                              className="agent-flow-llm-parameter-form__help-icon"
                              aria-label={i18nText("agentFlow", "auto.field_description", { value1: field.label })}
                            />
                          </Tooltip>
                        ) : null}
                      </span>
                    </div>
                    <div className="agent-flow-llm-parameter-form__row-toggle">
                      {!alwaysEnabled ? (
                        <Switch
                          checked={enabled}
                          onChange={(checked) =>
                            nextParameters(
                              parameters.items[field.key]?.value ??
                                value ??
                                defaultValue,
                              checked
                            )
                          }
                        />
                      ) : (
                        <Typography.Text
                          type="secondary"
                          className="agent-flow-llm-parameter-form__row-fixed"
                        >
                          {i18nText("agentFlow", "auto.always_on")}</Typography.Text>
                      )}
                    </div>
                  </div>
                  <div className="agent-flow-llm-parameter-form__row-control">
                    {renderFieldControl({
                      field,
                      value,
                      contextWindow,
                      nextParameters: (nextValue) => nextParameters(nextValue),
                      restoreDefaultValue: () => nextParameters(defaultValue)
                    })}
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      ))}
      {Object.keys(parameters.items).length === 0 ? (
        <Typography.Text type="secondary">
          {DEFAULT_LLM_PARAMETERS.schema_version}
        </Typography.Text>
      ) : null}
    </div>
  );
}
