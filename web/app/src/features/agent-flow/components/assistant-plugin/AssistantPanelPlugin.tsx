import CheckOutlined from '@ant-design/icons/es/icons/CheckOutlined';
import BranchesOutlined from '@ant-design/icons/es/icons/BranchesOutlined';
import HistoryOutlined from '@ant-design/icons/es/icons/HistoryOutlined';
import SelectOutlined from '@ant-design/icons/es/icons/SelectOutlined';
import SettingOutlined from '@ant-design/icons/es/icons/SettingOutlined';
import Sender from '@ant-design/x/es/sender';
import {
  Button,
  Dropdown,
  Flex,
  Progress,
  Tooltip,
  type MenuProps
} from 'antd';
import type { ComponentProps } from 'react';
import type {
  ConsoleAssistantPreference,
  ConsoleAssistantRunCapabilities
} from '@1flowbase/api-client';
import { AgentFlowDebugConsole } from '../debug-console/AgentFlowDebugConsole';
import { i18nText } from '../../../../shared/i18n/text';
import { formatLlmTokenCount } from '../../lib/model-options';
import './assistant-panel-plugin.css';

export interface AssistantPanelRuntime {
  run_capabilities: ConsoleAssistantRunCapabilities;
  preference: Pick<ConsoleAssistantPreference, 'model' | 'reasoning_effort'>;
  contextSnapshot?: {
    effective_context_window?: number | null;
    input_tokens?: number | null;
  } | null;
  onChangePreference: (
    patch: Pick<ConsoleAssistantPreference, 'model' | 'reasoning_effort'>
  ) => void | Promise<void>;
}
interface PanelAction {
  disabled?: boolean;
  onClick: () => void;
}
export type AssistantPanelPluginProps = Omit<
  ComponentProps<typeof AgentFlowDebugConsole>,
  'composerFooterActions' | 'headerActions'
> & {
  runtime?: AssistantPanelRuntime;
  selection?: { selecting: boolean; disabled?: boolean; onToggle: () => void };
  activity: PanelAction;
  history: PanelAction;
  settingsAction?: PanelAction;
  overlayZIndex?: number;
};

// Hosts own execution and persistence. This plugin owns the assistant interaction surface.
export function AssistantPanelPlugin({
  runtime,
  selection,
  activity,
  history,
  settingsAction,
  overlayZIndex,
  ...conversation
}: AssistantPanelPluginProps) {
  const selectedModel =
    runtime?.run_capabilities.models.find(
      (model) => model.id === runtime.preference.model
    ) ?? runtime?.run_capabilities.models[0];
  const selectedReasoningEffort =
    runtime?.preference.reasoning_effort ??
    selectedModel?.default_reasoning_effort ??
    selectedModel?.reasoning_efforts[0];
  const contextWindow =
    runtime?.contextSnapshot?.effective_context_window ??
    selectedModel?.context_window ??
    null;
  const contextTokenUsage = runtime?.contextSnapshot?.input_tokens ?? null;
  const measuredContextTokenUsage = contextTokenUsage ?? 0;
  const contextUsagePercent =
    contextWindow && contextWindow > 0
      ? Math.min(
          100,
          Math.round((measuredContextTokenUsage / contextWindow) * 1000) / 10
        )
      : 0;
  const contextVisualPercent =
    measuredContextTokenUsage > 0 ? Math.max(1, contextUsagePercent) : 0;
  const remainingContextPercent = Math.max(
    0,
    Math.round((100 - contextUsagePercent) * 10) / 10
  );
  const runtimePreferenceMenuItems: MenuProps['items'] = runtime
    ? [
        {
          key: 'model',
          label: (
            <span className="assistant-panel-plugin__runtime-menu-row">
              <span>{i18nText('appShell', 'auto.assistant_model')}</span>
              <span className="assistant-panel-plugin__runtime-menu-value">
                {selectedModel?.name ?? selectedModel?.id ?? '-'}
              </span>
            </span>
          ),
          children: runtime.run_capabilities.models.map((model) => ({
            key: `model:${model.id}`,
            label: (
              <span className="assistant-panel-plugin__runtime-menu-option">
                <span>{model.name ?? model.id}</span>
                {model.id === selectedModel?.id ? <CheckOutlined /> : null}
              </span>
            )
          }))
        },
        ...(runtime.run_capabilities.reasoning_effort_enabled &&
        selectedModel?.reasoning_efforts.length
          ? [
              {
                key: 'reasoning-effort',
                label: (
                  <span className="assistant-panel-plugin__runtime-menu-row">
                    <span>
                      {i18nText('appShell', 'auto.assistant_reasoning_effort')}
                    </span>
                    <span className="assistant-panel-plugin__runtime-menu-value">
                      {selectedReasoningEffort ?? '-'}
                    </span>
                  </span>
                ),
                children: selectedModel.reasoning_efforts.map((effort) => ({
                  key: `reasoning-effort:${effort}`,
                  label: (
                    <span className="assistant-panel-plugin__runtime-menu-option">
                      <span>{effort}</span>
                      {effort === selectedReasoningEffort ? (
                        <CheckOutlined />
                      ) : null}
                    </span>
                  )
                }))
              }
            ]
          : []),
        { type: 'divider' },
        {
          key: 'reset-defaults',
          label: i18nText('appShell', 'auto.assistant_reset_defaults')
        }
      ]
    : [];
  return (
    <AgentFlowDebugConsole
      {...conversation}
      composerFooterActions={
        <Flex
          align="center"
          className="assistant-panel-plugin__composer-actions"
          gap={8}
          justify="space-between"
        >
          <Flex align="center" gap={4}>
            <Tooltip
              title={i18nText('appShell', 'auto.assistant_select_page_content')}
            >
              <Button
                aria-label={i18nText(
                  'appShell',
                  'auto.assistant_select_page_content'
                )}
                disabled={!selection || selection.disabled}
                icon={<SelectOutlined />}
                size="small"
                type={selection?.selecting ? 'primary' : 'text'}
                onClick={selection?.onToggle}
              />
            </Tooltip>
          </Flex>
          {runtime?.run_capabilities.model_selection_enabled ? (
            <Flex align="center" gap={8}>
              {contextWindow ? (
                <Tooltip
                  color="#ffffff"
                  styles={{
                    container: {
                      border: '1px solid var(--border-subtle)',
                      borderRadius: '0.5rem',
                      boxShadow: 'var(--shadow-float)',
                      padding: '0.5rem 0.625rem'
                    }
                  }}
                  title={
                    <span className="assistant-panel-plugin__context-tooltip">
                      {contextTokenUsage !== null ? (
                        <span>
                          {i18nText(
                            'appShell',
                            'auto.assistant_context_remaining_percent',
                            {
                              value1: remainingContextPercent
                            }
                          )}
                        </span>
                      ) : null}
                      <span className="assistant-panel-plugin__context-tooltip-total">
                        {i18nText('appShell', 'auto.assistant_context_total', {
                          value2: formatLlmTokenCount(contextWindow) ?? '0',
                          value1: formatLlmTokenCount(contextTokenUsage) ?? '—'
                        })}
                      </span>
                    </span>
                  }
                >
                  <span className="assistant-panel-plugin__context-progress">
                    <Progress
                      percent={contextVisualPercent}
                      showInfo={false}
                      size={18}
                      trailColor="var(--border-default)"
                      type="circle"
                    />
                  </span>
                </Tooltip>
              ) : null}
              <Dropdown
                overlayStyle={{
                  zIndex: overlayZIndex
                }}
                placement="topLeft"
                trigger={['click']}
                menu={{
                  items: runtimePreferenceMenuItems,
                  onClick: ({ key }) => {
                    const selection = String(key);
                    if (selection === 'reset-defaults') {
                      void runtime.onChangePreference({
                        model: null,
                        reasoning_effort: null
                      });
                      return;
                    }
                    if (selection.startsWith('model:')) {
                      const modelId = selection.slice('model:'.length);
                      const model = runtime.run_capabilities.models.find(
                        (candidate) => candidate.id === modelId
                      );
                      if (model) {
                        void runtime.onChangePreference({
                          model: model.id,
                          reasoning_effort:
                            model.default_reasoning_effort ?? null
                        });
                      }
                      return;
                    }
                    if (selection.startsWith('reasoning-effort:')) {
                      const reasoning_effort = selection.slice(
                        'reasoning-effort:'.length
                      );
                      if (
                        selectedModel?.reasoning_efforts.includes(
                          reasoning_effort
                        )
                      ) {
                        void runtime.onChangePreference({
                          model: selectedModel.id,
                          reasoning_effort
                        });
                      }
                    }
                  }
                }}
              >
                <Sender.Switch
                  rootClassName="assistant-panel-plugin__runtime-preferences"
                  value={false}
                >
                  <span className="assistant-panel-plugin__model-label">
                    {selectedModel?.name ?? selectedModel?.id ?? '-'}
                  </span>
                  {runtime.run_capabilities.reasoning_effort_enabled &&
                  selectedReasoningEffort ? (
                    <span className="assistant-panel-plugin__runtime-preferences-effort">
                      {selectedReasoningEffort}
                    </span>
                  ) : null}
                </Sender.Switch>
              </Dropdown>
            </Flex>
          ) : null}
        </Flex>
      }
      headerActions={
        <>
          <Button
            aria-label={i18nText('appShell', 'auto.assistant_activity')}
            disabled={activity.disabled}
            icon={<BranchesOutlined />}
            size="small"
            type="text"
            onClick={activity.onClick}
          />
          <Button
            aria-label={i18nText('appShell', 'auto.assistant_history')}
            disabled={history.disabled}
            icon={<HistoryOutlined />}
            size="small"
            type="text"
            onClick={history.onClick}
          />
          {settingsAction ? (
            <Button
              aria-label={i18nText('appShell', 'auto.assistant_settings')}
              disabled={settingsAction.disabled}
              icon={<SettingOutlined />}
              size="small"
              type="text"
              onClick={settingsAction.onClick}
            />
          ) : null}
        </>
      }
    />
  );
}
