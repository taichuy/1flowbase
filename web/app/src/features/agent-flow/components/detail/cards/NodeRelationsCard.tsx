import { Typography } from 'antd';
import { HomeOutlined, PlusOutlined } from '@ant-design/icons';
import type { SchemaAdapter } from '../../../../../shared/schema-ui/v1/registry/create-renderer-registry';

import {
  getDirectDownstreamNodes
} from '../../../lib/document/relations';
import { useAgentFlowEditorStore } from '../../../store/editor/provider';
import {
  selectSelectedNodeId,
  selectWorkingDocument
} from '../../../store/editor/selectors';
import { useNodeInteractions } from '../../../hooks/interactions/use-node-interactions';
import { i18nText } from '../../../../../shared/i18n/text';

export function NodeRelationsCard({
  adapter
}: {
  adapter?: SchemaAdapter;
} = {}) {
  const document = useAgentFlowEditorStore(selectWorkingDocument);
  const selectedNodeId = useAgentFlowEditorStore(selectSelectedNodeId);
  const { openNodePicker } = useNodeInteractions();
  const downstreamNodes =
    (adapter?.getDerived('downstreamNodes') as Array<{
      id: string;
      alias: string;
    }>) ?? (selectedNodeId ? getDirectDownstreamNodes(document, selectedNodeId) : []);

  if (!selectedNodeId) {
    return null;
  }

  return (
    <div className="agent-flow-node-detail__section">
      <Typography.Title level={5} className="agent-flow-node-detail__section-title">
        {i18nText("agentFlow", "auto.next_step")}</Typography.Title>
      <Typography.Text className="agent-flow-node-detail__section-subtitle">
        {i18nText("agentFlow", "auto.add_next_node_workflow")}</Typography.Text>

      <div className="agent-flow-node-detail__relation-list" style={{ marginTop: 12 }}>
        <div className="agent-flow-node-detail__relation-source">
          <HomeOutlined />
        </div>
        <div className="agent-flow-node-detail__relation-line" />
        <div className="agent-flow-node-detail__relation-nodes">
          {downstreamNodes.map((node) => (
            <div key={node.id} className="agent-flow-node-detail__relation-item">
              <div className="agent-flow-node-detail__relation-item-icon">
                <HomeOutlined style={{ fontSize: 12 }} />
              </div>
              {node.alias}
            </div>
          ))}
          <div
            className="agent-flow-node-detail__relation-add"
            onClick={() => {
              if (adapter) {
                adapter.dispatch('openNodePicker', { nodeId: selectedNodeId });
                return;
              }

              openNodePicker(selectedNodeId);
            }}
          >
            <PlusOutlined /> {i18nText("agentFlow", "auto.add_parallel_node")}</div>
        </div>
      </div>
    </div>
  );
}
