import {
  useEffect,
  useMemo,
  useState,
  type MouseEvent as ReactMouseEvent
} from 'react';
import {
  Collapse,
  Empty,
  Input,
  Button,
  Space,
  Tag,
  Tooltip,
  Typography
} from 'antd';

import type { AgentFlowVariableGroup } from '../../../api/runtime';
import { i18nText } from '../../../../../shared/i18n/text';

function formatValue(value: unknown): string {
  if (typeof value === 'string') return value;
  if (
    typeof value === 'number' ||
    typeof value === 'boolean' ||
    value === null ||
    value === undefined
  ) {
    return String(value);
  }

  return JSON.stringify(value, null, 2);
}

function parseEditableValue(rawValue: string): unknown {
  if (rawValue === '') {
    return '';
  }

  try {
    return JSON.parse(rawValue);
  } catch {
    return rawValue;
  }
}

export interface SelectedVariableInfo {
  label: string;
  value: unknown;
  key: string;
  isReadOnly?: boolean;
}

const DEFAULT_SIDEBAR_WIDTH = 270;
const MIN_SIDEBAR_WIDTH = 140;

type SelectableVariableItem = AgentFlowVariableGroup['items'][number] & {
  selectionKey: string;
};

export function DebugVariablesPane({
  groups,
  onSelectedChange,
  onSelectedValueChange,
  onLoadFullValue,
  sidebarWidth,
  sidebarMinWidth,
  sidebarMaxWidth,
  onSidebarResizeStart
}: {
  groups: AgentFlowVariableGroup[];
  onSelectedChange?: (info: SelectedVariableInfo | null) => void;
  onSelectedValueChange?: (key: string, value: unknown) => void;
  onLoadFullValue?: (artifactRef: string) => Promise<unknown>;
  sidebarWidth?: number;
  sidebarMinWidth?: number;
  sidebarMaxWidth?: number;
  onSidebarResizeStart?: (event: ReactMouseEvent<HTMLDivElement>) => void;
}) {
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [selectedValueText, setSelectedValueText] = useState('');
  const effectiveSidebarWidth = useMemo(() => {
    const minWidth = sidebarMinWidth ?? MIN_SIDEBAR_WIDTH;
    const maxWidth = sidebarMaxWidth ?? Number.POSITIVE_INFINITY;
    const baseWidth = sidebarWidth ?? DEFAULT_SIDEBAR_WIDTH;

    return Math.max(minWidth, Math.min(baseWidth, maxWidth));
  }, [sidebarWidth, sidebarMaxWidth, sidebarMinWidth]);
  const allItems = useMemo<SelectableVariableItem[]>(
    () =>
      groups.flatMap((group, groupIndex) =>
        group.items.map((item, itemIndex) => ({
          ...item,
          selectionKey: `${groupIndex}:${itemIndex}:${item.key}`
        }))
      ),
    [groups]
  );
  const selectedItem = useMemo(
    () =>
      selectedKey
        ? allItems.find((item) => item.selectionKey === selectedKey)
        : null,
    [selectedKey, allItems]
  );

  useEffect(() => {
    const nextKey = allItems.at(0)?.selectionKey ?? null;

    if (allItems.length === 0) {
      if (selectedKey !== null) {
        setSelectedKey(null);
      }
      return;
    }

    if (selectedKey === null) {
      setSelectedKey(nextKey);
      return;
    }

    const exists = allItems.some((item) => item.selectionKey === selectedKey);
    if (!exists) {
      setSelectedKey(nextKey);
    }
  }, [allItems, selectedKey]);

  useEffect(() => {
    if (!selectedItem) {
      setSelectedValueText('');
      return;
    }

    setSelectedValueText(formatValue(selectedItem.value));
  }, [selectedItem]);

  // 通知父级选中项变化
  useEffect(() => {
    if (selectedItem) {
      onSelectedChange?.({
        label: selectedItem.label,
        value: selectedItem.value,
        key: selectedItem.key,
        isReadOnly: selectedItem.isReadOnly
      });
    } else {
      onSelectedChange?.(null);
    }
  }, [selectedKey, selectedItem, onSelectedChange]);

  function handleVariableValueBlur() {
    if (!selectedItem || selectedItem.isReadOnly) {
      return;
    }

    const nextValue = parseEditableValue(selectedValueText);

    onSelectedValueChange?.(selectedItem.key, nextValue);
  }

  async function handleLoadFullValue() {
    if (!selectedItem?.artifactRef || !onLoadFullValue) {
      return;
    }

    const fullValue = await onLoadFullValue(selectedItem.artifactRef);
    setSelectedValueText(formatValue(fullValue));
    onSelectedValueChange?.(selectedItem.key, fullValue);
    onSelectedChange?.({
      label: selectedItem.label,
      value: fullValue,
      key: selectedItem.key,
      isReadOnly: selectedItem.isReadOnly
    });
  }

  if (groups.length === 0) {
    return (
      <div className="agent-flow-editor__debug-console-pane">
        <Empty
          description={i18nText("agentFlow", "auto.currently_variable_snapshot")}
          image={Empty.PRESENTED_IMAGE_SIMPLE}
        />
      </div>
    );
  }

  const defaultGroupKeys = groups.map(
    (group, index) => `${index}:${group.title}`
  );

  return (
    <div className="agent-flow-editor__debug-console-pane agent-flow-editor__debug-variables-pane">
      <div
        className="agent-flow-editor__debug-variables-sidebar"
        style={{ width: effectiveSidebarWidth }}
        data-testid="agent-flow-editor-variable-cache-sidebar"
      >
        <Collapse
          defaultActiveKey={defaultGroupKeys}
          expandIconPlacement="end"
          className="agent-flow-editor__debug-variables-collapse"
          ghost
          size="small"
          items={groups.map((group, groupIndex) => {
            const groupKey = `${groupIndex}:${group.title}`;

            return {
              key: groupKey,
              label: (
                <Typography.Text
                  ellipsis={{ tooltip: false }}
                  className="agent-flow-editor__debug-variables-group-title"
                >
                  {group.title}
                </Typography.Text>
              ),
              children: (
                <div className="agent-flow-editor__debug-variables-group">
                  {group.items.map((item, itemIndex) => {
                    const selectionKey = `${groupIndex}:${itemIndex}:${item.key}`;

                    return (
                      <div
                        key={selectionKey}
                        className={`agent-flow-editor__debug-variables-item ${
                          selectedKey === selectionKey ? 'is-selected' : ''
                        }`}
                        onClick={() => setSelectedKey(selectionKey)}
                      >
                        <Tooltip title={item.label} placement="top">
                          <span className="agent-flow-editor__debug-variables-item-text">
                            <Typography.Text ellipsis={{ tooltip: false }}>
                              {item.label}
                            </Typography.Text>
                            {item.helperText ? (
                              <Typography.Text
                                className="agent-flow-editor__debug-variables-item-helper"
                                ellipsis={{ tooltip: false }}
                                type="secondary"
                              >
                                {item.helperText}
                              </Typography.Text>
                            ) : null}
                          </span>
                        </Tooltip>
                      </div>
                    );
                  })}
                </div>
              )
            };
          })}
        />
      </div>
      <div
        aria-label={i18nText("agentFlow", "auto.adjust_variable_list_width")}
        aria-orientation="vertical"
        className="agent-flow-editor__debug-variables-resize-handle"
        onMouseDown={onSidebarResizeStart}
        role="separator"
      />
      <div className="agent-flow-editor__debug-variables-detail">
        {selectedItem ? (
          <>
            {selectedItem.isTruncated && selectedItem.artifactRef ? (
              <Space
                className="agent-flow-editor__debug-variables-artifact-toolbar"
                size={8}
                wrap
              >
                <Tag color="warning">{i18nText("agentFlow", "auto.truncated")}</Tag>
                <Button size="small" onClick={handleLoadFullValue}>
                  {i18nText("agentFlow", "auto.load_full_value")}</Button>
              </Space>
            ) : null}
            <Input.TextArea
              key={selectedItem.selectionKey}
              style={{ height: '100%' }}
              aria-label={i18nText("agentFlow", "auto.variable_value_edit_box")}
              className="agent-flow-editor__debug-variables-detail-value"
              disabled={selectedItem.isReadOnly || selectedItem.isTruncated}
              onBlur={handleVariableValueBlur}
              onChange={(event) => setSelectedValueText(event.target.value)}
              value={selectedValueText}
              placeholder={
                selectedItem.isReadOnly ? i18nText("agentFlow", "auto.system_variables_edited") : undefined
              }
            />
          </>
        ) : (
          <div className="agent-flow-editor__debug-variables-detail-empty">
            <Empty
              description={i18nText("agentFlow", "auto.select_variable_left_view_details")}
              image={Empty.PRESENTED_IMAGE_SIMPLE}
            />
          </div>
        )}
      </div>
    </div>
  );
}
