import { useQuery } from '@tanstack/react-query';
import { DeleteOutlined, PlusOutlined } from '@ant-design/icons';
import { Button, Dropdown, Tag, Typography, type MenuProps } from 'antd';

import {
  agentFlowMcpInstanceOptionsQueryKey,
  fetchAgentFlowMcpInstanceOptions
} from '../../../api/mcp-instance-options';
import type { SchemaFieldRendererProps } from '../../../../../shared/schema-ui/v1/registry/create-renderer-registry';
import { i18nText } from '../../../../../shared/i18n/text';
import '../../../../../shared/ui/structured-list/structured-list.css';

export function LlmMcpInstancesField({
  adapter,
  block
}: SchemaFieldRendererProps) {
  const optionsQuery = useQuery({
    queryKey: agentFlowMcpInstanceOptionsQueryKey,
    queryFn: fetchAgentFlowMcpInstanceOptions
  });
  const rawValue = adapter.getValue(block.path);
  const value = Array.isArray(rawValue)
    ? rawValue.filter((item): item is string => typeof item === 'string')
    : [];
  const options = optionsQuery.data ?? [];
  const optionsById = new Map(options.map((option) => [option.value, option]));
  const menuItems: MenuProps['items'] = options.map((option) => ({
    key: option.value,
    label: option.label
  }));

  const addOccurrence: MenuProps['onClick'] = ({ key }) => {
    adapter.setValue(block.path, [...value, key]);
  };

  const removeOccurrence = (occurrenceIndex: number) => {
    adapter.setValue(
      block.path,
      value.filter((_, index) => index !== occurrenceIndex)
    );
  };

  return (
    <div className="agent-flow-mcp-instances">
      <div
        className="agent-flow-mcp-instances__toolbar"
        data-testid="agent-flow-mcp-instances-toolbar"
      >
        <Typography.Text strong className="agent-flow-mcp-instances__label">
          {block.label}
        </Typography.Text>
        <div className="agent-flow-mcp-instances__actions">
          <Dropdown
            disabled={options.length === 0}
            menu={{ items: menuItems, onClick: addOccurrence }}
            placement="bottomRight"
            trigger={['click']}
          >
            <Button
              aria-label={i18nText('agentFlow', 'auto.add_mcp_instance')}
              className="agent-flow-mcp-instances__add"
              disabled={options.length === 0}
              icon={<PlusOutlined />}
              loading={optionsQuery.isLoading}
              shape="circle"
              size="small"
              type="text"
            />
          </Dropdown>
        </div>
      </div>
      <div className="structured-list structured-list--bordered structured-list--small">
        {value.length > 0 ? (
          <ul aria-label={block.label} className="structured-list__items">
            {value.map((instanceId, occurrenceIndex) => {
              const option = optionsById.get(instanceId);
              const displayName = option?.label ?? instanceId;
              const unavailable = optionsQuery.isSuccess && !option;

              return (
                <li
                  className="structured-list__item"
                  data-testid={`agent-flow-mcp-instance-occurrence-${occurrenceIndex}`}
                  key={`${instanceId}-${occurrenceIndex}`}
                >
                  <span className="structured-list__content">
                    {displayName}
                    {unavailable ? (
                      <Tag
                        color="warning"
                        className="agent-flow-mcp-instances__status"
                      >
                        {i18nText('agentFlow', 'auto.mcp_instance_unavailable')}
                      </Tag>
                    ) : null}
                  </span>
                  <span className="structured-list__actions">
                    <Button
                      aria-label={i18nText('agentFlow', 'auto.delete_item', {
                        value1: displayName
                      })}
                      danger
                      icon={<DeleteOutlined />}
                      size="small"
                      type="text"
                      onClick={() => removeOccurrence(occurrenceIndex)}
                    />
                  </span>
                </li>
              );
            })}
          </ul>
        ) : (
          <div className="structured-list__empty">
            {i18nText('agentFlow', 'auto.no_mcp_instance_mounts')}
          </div>
        )}
      </div>
    </div>
  );
}
