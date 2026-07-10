import { CloseOutlined } from '@ant-design/icons';
import { Button, Space, Typography } from 'antd';
import { useMemo } from 'react';

import { SchemaDockPanel } from '../../../shared/schema-ui/overlay-shell/SchemaDockPanel';
import { SchemaRenderer } from '../../../shared/schema-ui/runtime/SchemaRenderer';
import { createAgentFlowNodeSchemaAdapter } from '../../agent-flow/schema/node-schema-adapter';
import { agentFlowRendererRegistry } from '../../agent-flow/schema/agent-flow-renderer-registry';
import { resolveAgentFlowNodeSchema } from '../../agent-flow/schema/node-schema-registry';
import { NodeConfigTab } from '../../agent-flow/components/detail/tabs/NodeConfigTab';
import { useNodeInteractions } from '../../agent-flow/hooks/interactions/use-node-interactions';
import type { AgentFlowEnvironmentVariable } from '../../agent-flow/lib/variables/application-environment-variables';
import type { AgentFlowIssue } from '../../agent-flow/lib/validate-document';
import { useAgentFlowEditorStore } from '../../agent-flow/store/editor/provider';
import type { WorkflowTriggerContext } from '../lib/trigger-context';

const shellSchema = {
  schemaVersion: '1.0.0',
  shellType: 'dock_panel',
  title: 'Workflow node details'
} as const;

export function WorkflowNodeDetailPanel({
  environmentVariables = [],
  issues = [],
  triggerContext,
  onClose
}: {
  environmentVariables?: AgentFlowEnvironmentVariable[];
  issues?: AgentFlowIssue[];
  triggerContext: WorkflowTriggerContext;
  onClose: () => void;
}) {
  const document = useAgentFlowEditorStore((state) => state.workingDocument);
  const selectedNodeId = useAgentFlowEditorStore(
    (state) => state.selectedNodeId
  );
  const setWorkingDocument = useAgentFlowEditorStore(
    (state) => state.setWorkingDocument
  );
  const { openNodePicker } = useNodeInteractions();
  const runtime = useMemo(() => {
    if (!selectedNodeId) {
      return null;
    }

    const node = document.graph.nodes.find(
      (candidate) => candidate.id === selectedNodeId
    );

    if (!node) {
      return null;
    }

    const schema = resolveAgentFlowNodeSchema(node.type);
    const adapter = createAgentFlowNodeSchemaAdapter({
      document,
      nodeId: selectedNodeId,
      environmentVariables,
      issues,
      workflowTriggerContext: triggerContext,
      setWorkingDocument,
      dispatch(actionKey, payload) {
        if (actionKey === 'openNodePicker') {
          openNodePicker(
            (payload as { nodeId?: string } | undefined)?.nodeId ??
              selectedNodeId
          );
        }
      }
    });

    return { adapter, schema };
  }, [
    document,
    environmentVariables,
    issues,
    openNodePicker,
    selectedNodeId,
    setWorkingDocument,
    triggerContext
  ]);

  if (!runtime) {
    return null;
  }

  const aliasBlock = runtime.schema.detail.header.blocks.find(
    (block) => block.kind === 'field' && block.path === 'alias'
  );

  return (
    <SchemaDockPanel
      bodyClassName="agent-flow-node-detail__body"
      className="agent-flow-node-detail"
      headerless
      schema={shellSchema}
    >
      <div
        className="agent-flow-node-detail__body"
        data-testid="workflow-node-detail-body"
      >
        <header className="agent-flow-node-detail__header">
          <div className="agent-flow-node-detail__header-top">
            <Typography.Text strong>Workflow</Typography.Text>
            <Space size={4}>
              <Button
                aria-label="Close Workflow node details"
                icon={<CloseOutlined />}
                size="small"
                type="text"
                onClick={onClose}
              />
            </Space>
          </div>
          {aliasBlock ? (
            <SchemaRenderer
              adapter={runtime.adapter}
              blocks={[aliasBlock]}
              registry={agentFlowRendererRegistry}
            />
          ) : null}
        </header>
        <NodeConfigTab adapter={runtime.adapter} schema={runtime.schema} />
      </div>
    </SchemaDockPanel>
  );
}
