import { Tabs } from 'antd';
import { useMemo } from 'react';

import { SchemaDockPanel } from '../../../../shared/schema-ui/v1/overlay-shell/SchemaDockPanel';
import { createAgentFlowNodeSchemaAdapter } from '../../schema/node-schema-adapter';
import { resolveAgentFlowNodeSchema } from '../../schema/node-schema-registry';
import { useNodeInteractions } from '../../hooks/interactions/use-node-interactions';
import type { AgentFlowEnvironmentVariable } from '../../lib/variables/application-environment-variables';
import type { AgentFlowIssue } from '../../lib/validate-document';
import { useAgentFlowEditorStore } from '../../store/editor/provider';
import { NodeDetailHeader } from './NodeDetailHeader';
import { NodeConfigTab } from './tabs/NodeConfigTab';
import { NodeLastRunTab } from './tabs/NodeLastRunTab';
import { i18nText } from '../../../../shared/i18n/text';

const nodeDetailShellSchema = {
  schemaVersion: '1.0.0',
  shellType: 'dock_panel',
  title: i18nText('agentFlow', 'auto.node_details')
} as const;

export function NodeDetailPanel({
  onClose,
  onDebugNode,
  onRunNode,
  applicationId,
  activeRunId,
  debugLoading = false,
  environmentVariables = [],
  issues = [],
  onResolveRunScope,
  previewActionsDisabled = false,
  runLoading = false,
  workflowTriggerContext = null,
  activeTab,
  onTabChange
}: {
  onClose: () => void;
  onDebugNode?: (() => void) | undefined;
  onRunNode?: (() => void) | undefined;
  applicationId?: string;
  activeRunId?: string | null;
  debugLoading?: boolean;
  environmentVariables?: AgentFlowEnvironmentVariable[];
  issues?: AgentFlowIssue[];
  onResolveRunScope?: ((runId: string | null) => void) | undefined;
  previewActionsDisabled?: boolean;
  runLoading?: boolean;
  workflowTriggerContext?: unknown;
  activeTab: 'config' | 'lastRun';
  onTabChange: (tab: 'config' | 'lastRun') => void;
}) {
  const document = useAgentFlowEditorStore((state) => state.workingDocument);
  const selectedNodeId = useAgentFlowEditorStore(
    (state) => state.selectedNodeId
  );
  const setWorkingDocument = useAgentFlowEditorStore(
    (state) => state.setWorkingDocument
  );
  const messages = useAgentFlowEditorStore((state) => state.messages);
  const { openNodePicker } = useNodeInteractions();
  const runtime = useMemo(() => {
    if (!selectedNodeId) {
      return { selectedNodeId: null, schema: null, adapter: null };
    }

    const selectedNode = document.graph.nodes.find(
      (node) => node.id === selectedNodeId
    );

    if (!selectedNode) {
      return { selectedNodeId: null, schema: null, adapter: null };
    }

    const schema = resolveAgentFlowNodeSchema(selectedNode.type);
    const adapter = createAgentFlowNodeSchemaAdapter({
      document,
      nodeId: selectedNodeId,
      environmentVariables,
      issues,
      messages,
      workflowTriggerContext,
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

    return {
      selectedNodeId,
      schema,
      adapter
    };
  }, [
    document,
    environmentVariables,
    issues,
    messages,
    openNodePicker,
    selectedNodeId,
    setWorkingDocument,
    workflowTriggerContext
  ]);

  if (!runtime.selectedNodeId || !runtime.schema || !runtime.adapter) {
    return null;
  }

  return (
    <SchemaDockPanel
      bodyClassName="agent-flow-node-detail__body"
      className="agent-flow-node-detail"
      headerless
      schema={nodeDetailShellSchema}
    >
      <div
        className="agent-flow-node-detail__body"
        data-testid="node-detail-body"
      >
        <NodeDetailHeader
          adapter={runtime.adapter}
          debugLoading={debugLoading}
          onClose={onClose}
          onDebugNode={onDebugNode}
          onRunNode={onRunNode}
          previewActionsDisabled={previewActionsDisabled}
          runLoading={runLoading}
          schema={runtime.schema}
        />
        <Tabs
          activeKey={activeTab}
          onChange={(key) => onTabChange(key as 'config' | 'lastRun')}
          items={[
            {
              key: 'config',
              label: i18nText('agentFlow', 'auto.settings'),
              children: (
                <NodeConfigTab
                  adapter={runtime.adapter}
                  schema={runtime.schema}
                />
              )
            },
            {
              key: 'lastRun',
              label: i18nText('agentFlow', 'auto.last_run'),
              forceRender: true,
              children: (
                <NodeLastRunTab
                  activeRunId={activeRunId}
                  adapter={runtime.adapter}
                  applicationId={applicationId}
                  nodeId={runtime.selectedNodeId}
                  onResolveRunScope={onResolveRunScope}
                  schema={runtime.schema}
                />
              )
            }
          ]}
        />
      </div>
    </SchemaDockPanel>
  );
}
