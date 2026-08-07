import { BookOutlined, CloseOutlined } from '@ant-design/icons';
import { Button, Divider, Space, Tooltip } from 'antd';

import type { CanvasNodeSchema } from '../../../../shared/schema-ui/v1/contracts/canvas-node-schema';
import { SchemaRenderer } from '../../../../shared/schema-ui/v1/runtime/SchemaRenderer';
import type { SchemaAdapter } from '../../../../shared/schema-ui/v1/registry/create-renderer-registry';

import { agentFlowRendererRegistry } from '../../schema/agent-flow-renderer-registry';
import { useNodeDetailActions } from '../../hooks/interactions/use-node-detail-actions';
import { NodeActionMenu } from './NodeActionMenu';
import { NodeDebugButton } from './NodeDebugButton';
import { NodeRunButton } from './NodeRunButton';
import { getAgentFlowNodeTypeIcon } from '../../lib/node-type-icons';
import { i18nText } from '../../../../shared/i18n/text';

function findHeaderField(
  schema: CanvasNodeSchema,
  path: string
) {
  return schema.detail.header.blocks.find(
    (block) => block.kind === 'field' && block.path === path
  );
}

export function NodeDetailHeader({
  schema,
  adapter,
  onClose,
  onDebugNode,
  onRunNode,
  debugLoading = false,
  previewActionsDisabled = false,
  runLoading = false
}: {
  schema: CanvasNodeSchema;
  adapter: SchemaAdapter;
  onClose: () => void;
  onDebugNode?: (() => void) | undefined;
  onRunNode?: (() => void) | undefined;
  debugLoading?: boolean;
  previewActionsDisabled?: boolean;
  runLoading?: boolean;
}) {
  const definitionMeta = adapter.getDerived('definitionMeta') as
    | { helpHref?: string | null }
    | null
    | undefined;
  const node = adapter.getDerived('node') as
    | { type?: string | null }
    | null
    | undefined;
  const nodeTypeIcon = node?.type ? getAgentFlowNodeTypeIcon(node.type) : null;
  const detailActions = useNodeDetailActions();
  const aliasField = findHeaderField(schema, 'alias');
  const descriptionField = findHeaderField(schema, 'description');

  return (
    <header
      className="agent-flow-node-detail__header"
      data-testid="node-detail-header"
    >
      <div className="agent-flow-node-detail__header-top">
        <div className="agent-flow-node-detail__title-section">
          <div className="agent-flow-node-detail__icon-wrapper">
            {nodeTypeIcon}
          </div>
          {aliasField ? (
            <SchemaRenderer
              adapter={adapter}
              blocks={[aliasField]}
              registry={agentFlowRendererRegistry}
            />
          ) : null}
        </div>
        <Space className="agent-flow-node-detail__actions" size={4}>
          {onRunNode ? (
            <NodeRunButton
              disabled={previewActionsDisabled}
              onRunNode={onRunNode}
              loading={runLoading}
            />
          ) : null}
          {onDebugNode ? (
            <NodeDebugButton
              disabled={previewActionsDisabled}
              onDebugNode={onDebugNode}
              loading={debugLoading}
            />
          ) : null}
          {definitionMeta?.helpHref ? (
            <Tooltip title={i18nText('agentFlow', 'auto.documentation')}>
              <Button
                aria-label={i18nText('agentFlow', 'auto.help_documentation')}
                href={definitionMeta.helpHref}
                icon={<BookOutlined />}
                target="_blank"
                type="text"
              />
            </Tooltip>
          ) : null}
          <NodeActionMenu
            onLocate={detailActions.locateSelectedNode}
            onCopy={detailActions.duplicateSelectedNode}
            onDelete={detailActions.deleteSelectedNode}
          />
          <Divider orientation="vertical" className="agent-flow-node-detail__divider" />
          <Button
            aria-label={i18nText("agentFlow", "auto.close_node_details")}
            icon={<CloseOutlined />}
            type="text"
            onClick={onClose}
          />
        </Space>
      </div>
      {descriptionField ? (
        <SchemaRenderer
          adapter={adapter}
          blocks={[descriptionField]}
          registry={agentFlowRendererRegistry}
        />
      ) : null}
    </header>
  );
}
