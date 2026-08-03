import { SearchOutlined } from '@ant-design/icons';
import { Button, Input, Popover } from 'antd';
import type { ReactElement, ReactNode } from 'react';
import { useMemo, useState } from 'react';

import {
  getNodePickerOptionDescription,
  getNodePickerOptionKey,
  type NodePickerOption
} from '../../lib/plugin-node-definitions';
import { getAgentFlowNodeTypeIcon } from '../../lib/node-type-icons';
import { calculateNodePickerMaxHeight } from './node-picker-layout';
import { i18nText } from '../../../../shared/i18n/text';

type BuiltinNodePickerOption = Extract<NodePickerOption, { kind: 'builtin' }>;
type NodePickerTab = 'builtin' | 'plugin';

interface NodePickerGroup {
  key: string;
  title: string;
  description: string;
}

const BUILTIN_NODE_PICKER_GROUPS: NodePickerGroup[] = [
  {
    key: 'io',
    title: i18nText('agentFlow', 'auto.start_stop_output'),
    description: i18nText('agentFlow', 'auto.define_input_entry_final_response')
  },
  {
    key: 'generation',
    title: i18nText('agentFlow', 'auto.model_and_generation'),
    description: i18nText(
      'agentFlow',
      'auto.invoke_models_retrieve_knowledge_organize_generated_content'
    )
  },
  {
    key: 'control',
    title: i18nText('agentFlow', 'auto.process_control'),
    description: i18nText(
      'agentFlow',
      'auto.branching_sorting_looping_batch_processing'
    )
  },
  {
    key: 'data',
    title: i18nText('agentFlow', 'auto.data_processing'),
    description: i18nText(
      'agentFlow',
      'auto.read_write_structured_data_maintain_process_variables'
    )
  },
  {
    key: 'external',
    title: i18nText('agentFlow', 'auto.external_capabilities'),
    description: i18nText(
      'agentFlow',
      'auto.call_interfaces_tools_out_system_capabilities'
    )
  }
];

interface NodePickerPopoverProps {
  ariaLabel: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onPickNode: (option: NodePickerOption) => void;
  options?: NodePickerOption[];
  buttonClassName?: string;
  buttonContent?: ReactNode;
  children?: ReactElement;
  placement?: 'top' | 'bottom' | 'left' | 'right' | 'rightTop';
}

export function NodePickerPopover({
  ariaLabel,
  open,
  onOpenChange,
  onPickNode,
  options = [],
  buttonClassName,
  buttonContent = '+',
  children,
  placement = 'rightTop'
}: NodePickerPopoverProps) {
  const [searchValue, setSearchValue] = useState('');
  const [activeTab, setActiveTab] = useState<NodePickerTab>(() =>
    options.some((option) => option.kind === 'plugin_contribution') &&
    !options.some((option) => option.kind === 'builtin')
      ? 'plugin'
      : 'builtin'
  );
  const normalizedSearchValue = searchValue.trim().toLowerCase();
  const builtinOptions = options.filter(
    (option): option is BuiltinNodePickerOption => option.kind === 'builtin'
  );
  const pluginOptions = options.filter(
    (
      option
    ): option is Extract<NodePickerOption, { kind: 'plugin_contribution' }> =>
      option.kind === 'plugin_contribution'
  );
  const groupedBuiltinOptions = useMemo(
    () =>
      BUILTIN_NODE_PICKER_GROUPS.map((group) => {
        const groupSearchText =
          `${group.title} ${group.description}`.toLowerCase();
        const groupMatchesSearch =
          normalizedSearchValue.length > 0 &&
          groupSearchText.includes(normalizedSearchValue);
        const groupedOptions = builtinOptions
          .filter((option) => option.category === group.key)
          .filter(
            (option) =>
              groupMatchesSearch ||
              matchesNodePickerSearch(
                option,
                normalizedSearchValue,
                getNodePickerOptionDescription(option)
              )
          );

        return { ...group, options: groupedOptions };
      }).filter((group) => group.options.length > 0),
    [builtinOptions, normalizedSearchValue]
  );
  const groupedBuiltinOptionKeys = new Set(
    BUILTIN_NODE_PICKER_GROUPS.map((group) => group.key)
  );
  const uncategorizedBuiltinOptions = builtinOptions.filter(
    (option) =>
      !groupedBuiltinOptionKeys.has(option.category ?? '') &&
      matchesNodePickerSearch(
        option,
        normalizedSearchValue,
        getNodePickerOptionDescription(option)
      )
  );
  const filteredPluginOptions = pluginOptions.filter((option) =>
    matchesNodePickerSearch(
      option,
      normalizedSearchValue,
      getNodePickerOptionDescription(option)
    )
  );
  const hasVisibleOptions =
    activeTab === 'builtin'
      ? groupedBuiltinOptions.length > 0 ||
        uncategorizedBuiltinOptions.length > 0
      : filteredPluginOptions.length > 0;

  function closePicker() {
    setSearchValue('');
    onOpenChange(false);
  }

  function resolvePopupContainer(triggerNode: HTMLElement) {
    const canvas = triggerNode.closest<HTMLElement>('.agent-flow-canvas');

    if (!canvas) {
      return document.body;
    }

    const editorBody = triggerNode.closest<HTMLElement>(
      '.agent-flow-editor__body'
    );
    const bottomControlBoundary = editorBody
      ?.querySelector<HTMLElement>('.agent-flow-editor__variable-cache-trigger')
      ?.getBoundingClientRect().bottom;
    const canvasRect = canvas.getBoundingClientRect();
    const triggerRect = triggerNode.getBoundingClientRect();
    const anchorY =
      placement === 'bottom' ? triggerRect.bottom : triggerRect.top;
    const maxHeight = calculateNodePickerMaxHeight({
      canvasBottom: canvasRect.bottom,
      anchorY,
      bottomBoundary: bottomControlBoundary
    });

    canvas.style.setProperty(
      '--agent-flow-node-picker-max-height',
      `${maxHeight}px`
    );

    return canvas;
  }

  return (
    <Popover
      rootClassName="agent-flow-node-picker-popover"
      destroyOnHidden
      getPopupContainer={resolvePopupContainer}
      styles={{
        body: {
          boxSizing: 'border-box',
          maxHeight:
            'var(--agent-flow-node-picker-max-height, calc(100vh - 120px))',
          overflow: 'hidden',
          overscrollBehavior: 'contain'
        }
      }}
      trigger="click"
      open={open}
      placement={placement}
      onOpenChange={onOpenChange}
      content={
        <div className="agent-flow-node-picker">
          <div className="agent-flow-node-picker__header">
            <div
              aria-label={i18nText('agentFlow', 'auto.node_source')}
              className="agent-flow-node-picker__tabs"
              role="tablist"
            >
              <button
                aria-selected={activeTab === 'builtin'}
                className={`agent-flow-node-picker__tab${activeTab === 'builtin' ? ' agent-flow-node-picker__tab--active' : ''}`}
                role="tab"
                type="button"
                onClick={() => setActiveTab('builtin')}
              >
                {i18nText('agentFlow', 'auto.built_in')}
              </button>
              <button
                aria-selected={activeTab === 'plugin'}
                className={`agent-flow-node-picker__tab${activeTab === 'plugin' ? ' agent-flow-node-picker__tab--active' : ''}`}
                role="tab"
                type="button"
                onClick={() => setActiveTab('plugin')}
              >
                {i18nText('agentFlow', 'auto.expand')}
              </button>
            </div>
            <div className="agent-flow-node-picker__search">
              <Input
                allowClear
                aria-label={i18nText('agentFlow', 'auto.search_node')}
                placeholder={i18nText('agentFlow', 'auto.search_node')}
                prefix={<SearchOutlined />}
                size="small"
                value={searchValue}
                onChange={(event) => {
                  setSearchValue(event.target.value);
                }}
              />
            </div>
          </div>
          <div className="agent-flow-node-picker__list" role="menu">
            {activeTab === 'builtin' ? (
              <>
                {groupedBuiltinOptions.map((group) => (
                  <NodePickerSection key={group.key} title={group.title}>
                    {group.options.map((option) => (
                      <NodePickerOptionButton
                        key={getNodePickerOptionKey(option)}
                        option={option}
                        onPick={() => {
                          closePicker();
                          onPickNode(option);
                        }}
                      />
                    ))}
                  </NodePickerSection>
                ))}
                {uncategorizedBuiltinOptions.length > 0 ? (
                  <NodePickerSection
                    title={i18nText('agentFlow', 'auto.other_nodes')}
                  >
                    {uncategorizedBuiltinOptions.map((option) => (
                      <NodePickerOptionButton
                        key={getNodePickerOptionKey(option)}
                        option={option}
                        onPick={() => {
                          closePicker();
                          onPickNode(option);
                        }}
                      />
                    ))}
                  </NodePickerSection>
                ) : null}
              </>
            ) : (
              <>
                {filteredPluginOptions.length > 0 ? (
                  <NodePickerSection
                    title={i18nText('agentFlow', 'auto.plugin_node_label')}
                  >
                    {filteredPluginOptions.map((option) => (
                      <NodePickerOptionButton
                        key={getNodePickerOptionKey(option)}
                        option={option}
                        onPick={() => {
                          if (option.disabled) {
                            return;
                          }

                          closePicker();
                          onPickNode(option);
                        }}
                      />
                    ))}
                  </NodePickerSection>
                ) : null}
              </>
            )}
            {!hasVisibleOptions ? (
              <div className="agent-flow-node-picker__empty">
                {normalizedSearchValue.length > 0
                  ? i18nText('agentFlow', 'auto.no_matching_node')
                  : activeTab === 'plugin'
                    ? i18nText('agentFlow', 'auto.expansion_node_yet')
                    : i18nText('agentFlow', 'auto.built_nodes_yet')}
              </div>
            ) : null}
          </div>
        </div>
      }
    >
      {children ?? (
        <Button
          aria-label={ariaLabel}
          className={buttonClassName}
          size="small"
          type="text"
          onClick={(event) => {
            event.stopPropagation();
          }}
        >
          {buttonContent}
        </Button>
      )}
    </Popover>
  );
}

interface NodePickerSectionProps {
  title: string;
  children: ReactNode;
}

function NodePickerSection({ title, children }: NodePickerSectionProps) {
  return (
    <section className="agent-flow-node-picker__section">
      <div className="agent-flow-node-picker__section-head">
        <div className="agent-flow-node-picker__section-label">{title}</div>
      </div>
      <div className="agent-flow-node-picker__section-items">{children}</div>
    </section>
  );
}

interface NodePickerOptionButtonProps {
  option: NodePickerOption;
  onPick: () => void;
}

function NodePickerOptionButton({
  option,
  onPick
}: NodePickerOptionButtonProps) {
  const icon =
    option.kind === 'builtin'
      ? getAgentFlowNodeTypeIcon(option.type)
      : getAgentFlowNodeTypeIcon('plugin_node');
  const description = getNodePickerOptionDescription(option);

  return (
    <button
      aria-label={option.label}
      className="agent-flow-node-picker__item"
      disabled={option.disabled}
      role="menuitem"
      type="button"
      onClick={onPick}
    >
      <span className="agent-flow-node-picker__icon" aria-hidden="true">
        {icon}
      </span>
      <span className="agent-flow-node-picker__text">
        <span className="agent-flow-node-picker__name">{option.label}</span>
        {description ? (
          <span className="agent-flow-node-picker__description">
            {description}
          </span>
        ) : null}
      </span>
    </button>
  );
}

function matchesNodePickerSearch(
  option: NodePickerOption,
  normalizedSearchValue: string,
  description: string | null
) {
  if (normalizedSearchValue.length === 0) {
    return true;
  }

  const searchText = [
    option.label,
    option.kind === 'builtin' ? option.type : option.plugin.contribution_code,
    option.category,
    [
      ...option.field_contract.config_fields,
      ...option.field_contract.input_fields,
      ...option.field_contract.output_fields
    ]
      .map((field) => field.key)
      .join(' '),
    description
  ]
    .filter((value): value is string => Boolean(value))
    .join(' ')
    .toLowerCase();

  return searchText.includes(normalizedSearchValue);
}
