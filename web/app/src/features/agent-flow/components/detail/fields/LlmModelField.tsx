import { useQuery } from '@tanstack/react-query';
import { Alert, Button, Divider, Empty, Select, Typography } from 'antd';
import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
  type ReactNode
} from 'react';

import type { SchemaFieldRendererProps } from '../../../../../shared/schema-ui/registry/create-renderer-registry';
import type { SchemaDynamicFormBlock } from '../../../../../shared/schema-ui/contracts/canvas-node-schema';
import {
  fetchModelProviderOptions,
  modelProviderOptionsQueryKey
} from '../../../api/model-provider-options';
import {
  buildLlmParameterState,
  getLlmModelProvider,
  getLlmParameters,
  resolveLlmParameterStateOnModelChange
} from '../../../lib/llm-node-config';
import {
  findLlmModelOption,
  findLlmProviderOption,
  formatLlmTokenCount,
  listLlmProviderOptions,
  type LlmModelGroup,
  type LlmModelOption,
  type LlmProviderOption
} from '../../../lib/model-options';
import { FloatingSettingsPanel } from '../FloatingSettingsPanel';
import { LlmParameterForm } from './LlmParameterForm';
import { i18nText } from '../../../../../shared/i18n/text';

const EMPTY_MODEL_PROVIDER = {
  provider_code: '',
  model_id: ''
} as const;

const MODEL_SETTINGS_MOUSE_LEAVE_CLOSE_DELAY_MS = 2_000;

const LLM_PARAMETERS_BLOCK: SchemaDynamicFormBlock = {
  kind: 'dynamic_form',
  form_key: 'llm_parameters',
  title: i18nText('agentFlow', 'auto.llm_parameters'),
  empty_text: i18nText('agentFlow', 'auto.select_model_first_adjust_parameters')
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function getNodeConfig(adapter: SchemaFieldRendererProps['adapter']) {
  const node = adapter.getDerived('node') as
    | { config?: Record<string, unknown> }
    | null
    | undefined;
  return isRecord(node?.config) ? node.config : {};
}

function getModelSearchText(
  provider: LlmProviderOption,
  group: LlmModelGroup,
  model: LlmModelOption
) {
  return [
    provider.label,
    provider.providerCode,
    group.label,
    model.label,
    model.value
  ]
    .filter(Boolean)
    .join(' ')
    .toLowerCase();
}

function buildModelSelection(nextModel: LlmModelOption) {
  return {
    provider_code: nextModel.providerCode,
    model_id: nextModel.value
  };
}

function buildOutputLabel(value: number | null | undefined) {
  const formattedValue = formatLlmTokenCount(value);
  return formattedValue
    ? i18nText('agentFlow', 'auto.output', { value1: formattedValue })
    : null;
}

function ContextMarker({ value }: { value: number | null | undefined }) {
  const formattedValue = formatLlmTokenCount(value);

  if (!formattedValue) {
    return null;
  }

  return (
    <span
      className="agent-flow-model-meta-pill agent-flow-model-meta-pill--context"
      aria-label={i18nText('agentFlow', 'auto.context', {
        value1: formattedValue
      })}
      title={i18nText('agentFlow', 'auto.context', { value1: formattedValue })}
    >
      {formattedValue}
    </span>
  );
}

function ModelChip({
  providerLabel,
  modelLabel,
  providerIcon,
  metaItems = [],
  placeholder = i18nText('agentFlow', 'auto.select_supplier_model')
}: {
  providerLabel?: string | null;
  modelLabel?: string | null;
  providerIcon?: string | null;
  metaItems?: ReactNode[];
  placeholder?: string;
}) {
  const visibleMetaItems = metaItems.filter(Boolean);

  return (
    <div
      className={`agent-flow-model-chip${modelLabel ? '' : ' agent-flow-model-chip--empty'}`}
    >
      <span className="agent-flow-model-chip__provider" aria-hidden="true">
        {providerIcon ? (
          <img
            className="agent-flow-model-chip__provider-image"
            src={providerIcon}
            alt=""
          />
        ) : (
          '◎'
        )}
      </span>
      <span className="agent-flow-model-chip__content">
        <span className="agent-flow-model-chip__eyebrow">
          {providerLabel || i18nText('agentFlow', 'auto.model_supplier')}
        </span>
        <span className="agent-flow-model-chip__label">
          {modelLabel || placeholder}
        </span>
        {visibleMetaItems.length > 0 ? (
          <span className="agent-flow-model-chip__meta">
            {visibleMetaItems.map((item, index) => (
              <span key={index} className="agent-flow-model-chip__meta-item">
                {item}
              </span>
            ))}
          </span>
        ) : null}
      </span>
    </div>
  );
}

export function LlmModelField({ adapter, block }: SchemaFieldRendererProps) {
  const [open, setOpen] = useState(false);
  const [dropdownOpen, setDropdownOpen] = useState(false);
  const [searchText, setSearchText] = useState('');
  const [expandedProviders, setExpandedProviders] = useState<string[]>([]);
  const triggerRef = useRef<HTMLButtonElement | null>(null);
  const mouseLeaveCloseTimerRef = useRef<ReturnType<typeof setTimeout> | null>(
    null
  );
  const providerOptionsQuery = useQuery({
    queryKey: modelProviderOptionsQueryKey,
    queryFn: fetchModelProviderOptions,
    staleTime: 60_000
  });
  const config = getNodeConfig(adapter);
  const modelProvider = getLlmModelProvider(config);
  const currentParameters = getLlmParameters(config);
  const providerCode = modelProvider.provider_code.trim();
  const modelValue = modelProvider.model_id.trim();
  const providerOptions = useMemo(
    () => listLlmProviderOptions(providerOptionsQuery.data),
    [providerOptionsQuery.data]
  );
  const selectedProvider = findLlmProviderOption(
    providerOptionsQuery.data,
    providerCode
  );
  const selectedModel = findLlmModelOption(
    providerOptionsQuery.data,
    providerCode,
    modelValue
  );
  const providerUnavailable = Boolean(
    providerCode && providerOptionsQuery.isSuccess && selectedProvider === null
  );
  const modelUnavailable = Boolean(
    providerCode &&
    modelValue &&
    providerOptionsQuery.isSuccess &&
    selectedModel === null
  );
  const filteredProviders = useMemo(() => {
    const normalizedSearch = searchText.trim().toLowerCase();

    if (!normalizedSearch) {
      return providerOptions;
    }

    return providerOptions
      .map((provider) => ({
        ...provider,
        modelGroups: provider.modelGroups
          .map((group) => ({
            ...group,
            models: group.models.filter((model) =>
              getModelSearchText(provider, group, model).includes(
                normalizedSearch
              )
            )
          }))
          .filter((group) => group.models.length > 0)
      }))
      .filter((provider) => provider.modelGroups.length > 0);
  }, [providerOptions, searchText]);
  const selectOptions = useMemo(
    () =>
      providerOptions.flatMap((provider) =>
        provider.modelGroups.flatMap((group) =>
          group.models.map((model) => ({
            value: model.selectionValue,
            label: model.label
          }))
        )
      ),
    [providerOptions]
  );

  useEffect(() => {
    if (open) {
      return;
    }

    setDropdownOpen(false);
    setSearchText('');
  }, [open]);

  useEffect(() => {
    return () => {
      clearMouseLeaveCloseTimer();
    };
  }, []);

  function clearMouseLeaveCloseTimer() {
    if (!mouseLeaveCloseTimerRef.current) {
      return;
    }

    clearTimeout(mouseLeaveCloseTimerRef.current);
    mouseLeaveCloseTimerRef.current = null;
  }

  function closeFloatingPanel() {
    clearMouseLeaveCloseTimer();
    setOpen(false);
  }

  function scheduleMouseLeaveClose() {
    if (!open) {
      return;
    }

    clearMouseLeaveCloseTimer();
    mouseLeaveCloseTimerRef.current = setTimeout(() => {
      mouseLeaveCloseTimerRef.current = null;
      setOpen(false);
    }, MODEL_SETTINGS_MOUSE_LEAVE_CLOSE_DELAY_MS);
  }

  function clearSelection() {
    adapter.setValue('config.model_provider', EMPTY_MODEL_PROVIDER);
    adapter.setValue('config.llm_parameters', buildLlmParameterState(null));
  }

  function selectModel(nextModel: LlmModelOption) {
    const nextProvider =
      providerOptions.find(
        (provider) => provider.providerCode === nextModel.providerCode
      ) ?? null;

    adapter.setValue('config.model_provider', buildModelSelection(nextModel));
    adapter.setValue(
      'config.llm_parameters',
      resolveLlmParameterStateOnModelChange({
        currentProviderCode: providerCode,
        nextProviderCode: nextModel.providerCode,
        currentParameters,
        nextSchema: nextProvider?.parameterForm
      })
    );
    setDropdownOpen(false);
    setSearchText('');
    setExpandedProviders([nextModel.providerCode]);
  }

  function toggleProvider(providerValue: string) {
    setExpandedProviders((current) =>
      current.includes(providerValue)
        ? current.filter((value) => value !== providerValue)
        : [...current, providerValue]
    );
  }

  function keepDropdownFocus(event: ReactMouseEvent<HTMLButtonElement>) {
    event.preventDefault();
  }

  function handleDropdownOpenChange(nextOpen: boolean) {
    setDropdownOpen(nextOpen);

    if (nextOpen) {
      setExpandedProviders((current) => {
        const allProviderValues = providerOptions.map(
          (provider) => provider.value
        );

        if (searchText.trim().length > 0) {
          return filteredProviders.map((provider) => provider.value);
        }

        if (current.length > 0) {
          return current;
        }

        return allProviderValues;
      });
      return;
    }

    setSearchText('');
  }

  function openFloatingPanel() {
    clearMouseLeaveCloseTimer();
    setOpen(true);
  }

  const floatingPanel = (
    <FloatingSettingsPanel
      open={open}
      title={i18nText('agentFlow', 'auto.model_settings')}
      closeLabel={i18nText('agentFlow', 'auto.close_model_settings')}
      triggerRef={triggerRef}
      dragHandleTestId="agent-flow-model-settings-drag-handle"
      leftResizeHandleTestId="agent-flow-model-settings-resize-handle-left"
      rightResizeHandleTestId="agent-flow-model-settings-resize-handle"
      onMouseEnter={clearMouseLeaveCloseTimer}
      onMouseLeave={scheduleMouseLeaveClose}
      onClose={closeFloatingPanel}
    >
      {providerOptionsQuery.isError ? (
        <Alert
          className="agent-flow-model-settings__notice"
          type="error"
          showIcon
          message={i18nText(
            'agentFlow',
            'auto.model_supplier_list_failed_load'
          )}
        />
      ) : null}
      {providerUnavailable ? (
        <Alert
          className="agent-flow-model-settings__notice"
          type="error"
          showIcon
          message={i18nText(
            'agentFlow',
            'auto.model_provider_referenced_node_unavailable'
          )}
        />
      ) : null}
      {modelUnavailable ? (
        <Alert
          className="agent-flow-model-settings__notice"
          type="error"
          showIcon
          message={i18nText(
            'agentFlow',
            'auto.model_referenced_node_supplier_s_valid_model_list'
          )}
        />
      ) : null}

      <div className="agent-flow-model-settings__section">
        <div className="agent-flow-model-settings__header">
          <Typography.Title
            level={5}
            className="agent-flow-model-settings__section-title"
          >
            {i18nText('agentFlow', 'auto.model')}
          </Typography.Title>
          {providerCode || modelValue ? (
            <Button type="link" onClick={clearSelection}>
              {i18nText('agentFlow', 'auto.clear')}
            </Button>
          ) : null}
        </div>
        <Typography.Text className="agent-flow-model-settings__section-subtitle">
          {i18nText(
            'agentFlow',
            'auto.nodes_store_stable_provider_model_semantics_resolve_currently_available_instances'
          )}
        </Typography.Text>
        <Select
          aria-label={i18nText('agentFlow', 'auto.select_supplier_model')}
          className="agent-flow-model-settings__select"
          placeholder={i18nText('agentFlow', 'auto.select_supplier_model')}
          value={selectedModel?.selectionValue}
          open={dropdownOpen}
          options={selectOptions}
          showSearch
          allowClear={false}
          filterOption={false}
          popupMatchSelectWidth
          onOpenChange={handleDropdownOpenChange}
          onSearch={setSearchText}
          popupRender={() => (
            <div className="agent-flow-model-settings__dropdown">
              {filteredProviders.length === 0 ? (
                <div className="agent-flow-model-settings__empty">
                  <Empty
                    image={Empty.PRESENTED_IMAGE_SIMPLE}
                    description={
                      searchText.trim().length > 0
                        ? i18nText('agentFlow', 'auto.matching_model_results')
                        : i18nText(
                            'agentFlow',
                            'auto.currently_available_models'
                          )
                    }
                  />
                </div>
              ) : (
                <div className="agent-flow-model-settings__provider-sections">
                  {filteredProviders.map((provider) => {
                    const providerExpanded =
                      searchText.trim().length > 0 ||
                      expandedProviders.includes(provider.value);

                    return (
                      <section
                        key={provider.value}
                        className="agent-flow-model-settings__provider-section"
                      >
                        <button
                          type="button"
                          className="agent-flow-model-settings__provider-head"
                          aria-expanded={providerExpanded}
                          onMouseDown={keepDropdownFocus}
                          onClick={() => toggleProvider(provider.value)}
                        >
                          <div className="agent-flow-model-settings__provider-head-main">
                            <span
                              className="agent-flow-model-settings__provider-icon"
                              aria-hidden="true"
                            >
                              {provider.icon ? (
                                <img
                                  className="agent-flow-model-settings__provider-icon-image"
                                  data-testid="agent-flow-model-settings-provider-icon-image"
                                  src={provider.icon}
                                  alt=""
                                />
                              ) : (
                                '◎'
                              )}
                            </span>
                            <div className="agent-flow-model-settings__provider-copy">
                              <Typography.Text strong>
                                {provider.label}
                              </Typography.Text>
                              <div className="agent-flow-model-settings__provider-meta">
                                {i18nText(
                                  'agentFlow',
                                  'auto.master_instance_aggregation'
                                )}
                                {provider.modelGroups.length}{' '}
                                {i18nText('agentFlow', 'auto.models')}
                              </div>
                            </div>
                          </div>
                          <span
                            className="agent-flow-model-settings__provider-caret"
                            aria-hidden="true"
                          >
                            {providerExpanded ? '▾' : '▸'}
                          </span>
                        </button>
                        {providerExpanded
                          ? provider.modelGroups.map((group) => (
                              <div
                                key={group.key}
                                className="agent-flow-model-settings__provider-group"
                              >
                                <div className="agent-flow-model-settings__group-head">
                                  <span>{group.label}</span>
                                </div>
                                <div className="agent-flow-model-settings__options">
                                  {group.models.map((option) => {
                                    const active =
                                      option.providerCode === providerCode &&
                                      option.value === modelValue;

                                    return (
                                      <button
                                        key={option.selectionValue}
                                        type="button"
                                        aria-label={`${provider.label} ${group.label} ${option.label}`}
                                        className={[
                                          'agent-flow-model-settings__option',
                                          active
                                            ? 'agent-flow-model-settings__option--active'
                                            : null
                                        ]
                                          .filter(Boolean)
                                          .join(' ')}
                                        onMouseDown={keepDropdownFocus}
                                        onClick={() => selectModel(option)}
                                      >
                                        <span className="agent-flow-model-settings__option-main">
                                          {option.label}
                                        </span>
                                        <span className="agent-flow-model-settings__option-meta">
                                          <span>{option.value}</span>
                                          <ContextMarker
                                            value={
                                              option.effectiveContextWindow
                                            }
                                          />
                                          {buildOutputLabel(
                                            option.maxOutputTokens
                                          ) ? (
                                            <span>
                                              {buildOutputLabel(
                                                option.maxOutputTokens
                                              )}
                                            </span>
                                          ) : null}
                                        </span>
                                      </button>
                                    );
                                  })}
                                </div>
                              </div>
                            ))
                          : null}
                      </section>
                    );
                  })}
                </div>
              )}
              <button
                type="button"
                className="agent-flow-model-settings__provider-link"
                onMouseDown={keepDropdownFocus}
                onClick={() =>
                  window.location.assign('/settings/model-providers')
                }
              >
                {i18nText('agentFlow', 'auto.model_supplier_settings')}
              </button>
            </div>
          )}
        />
      </div>

      <Divider />

      <div className="agent-flow-model-settings__section">
        <Typography.Title
          level={5}
          className="agent-flow-model-settings__section-title"
        >
          {i18nText('agentFlow', 'auto.parameters')}
        </Typography.Title>
        <LlmParameterForm adapter={adapter} block={LLM_PARAMETERS_BLOCK} />
      </div>
    </FloatingSettingsPanel>
  );

  return (
    <>
      <button
        type="button"
        aria-label={block.label}
        className="agent-flow-model-field__trigger"
        onClick={openFloatingPanel}
        onMouseEnter={clearMouseLeaveCloseTimer}
        onMouseLeave={scheduleMouseLeaveClose}
        ref={triggerRef}
      >
        <ModelChip
          providerIcon={
            selectedModel?.providerIcon || selectedProvider?.icon || null
          }
          providerLabel={
            modelProvider.provider_label?.trim() ||
            selectedModel?.providerLabel ||
            selectedProvider?.label ||
            providerCode ||
            null
          }
          modelLabel={
            modelProvider.model_label?.trim() ||
            selectedModel?.label ||
            modelValue ||
            null
          }
          metaItems={[
            <ContextMarker value={selectedModel?.effectiveContextWindow} />
          ]}
        />
        <span
          className="agent-flow-model-field__trigger-caret"
          aria-hidden="true"
        >
          ▾
        </span>
      </button>
      {floatingPanel}
    </>
  );
}
