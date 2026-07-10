import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';

import {
  validateDocument,
  type AgentFlowIssue
} from '../../agent-flow/lib/validate-document';

export function validateWorkflowDocument(
  document: FlowAuthoringDocument
): AgentFlowIssue[] {
  const issues: AgentFlowIssue[] = validateDocument(document).filter(
    (issue) =>
      issue.id !== 'global-start-count' && issue.id !== 'global-answer-missing'
  );
  const startNodes = document.graph.nodes.filter(
    (node) => node.type === 'workflow_start'
  );
  const endNodes = document.graph.nodes.filter(
    (node) => node.type === 'workflow_end'
  );
  const agentFlowBoundaryNodes = document.graph.nodes.filter(
    (node) => node.type === 'start' || node.type === 'answer'
  );

  if (startNodes.length !== 1) {
    issues.push({
      id: 'workflow-start-count',
      scope: 'global',
      level: 'error',
      nodeId: null,
      sectionKey: null,
      fieldKey: null,
      title: 'Workflow Start 配置错误',
      message: '每个 Workflow 草稿必须且只能保留一个 Workflow Start 节点。'
    });
  }

  if (endNodes.length === 0) {
    issues.push({
      id: 'workflow-end-count',
      scope: 'global',
      level: 'error',
      nodeId: null,
      sectionKey: null,
      fieldKey: null,
      title: 'Workflow End 配置错误',
      message: '每个 Workflow 草稿至少需要一个 Workflow End 节点。'
    });
  }

  for (const node of agentFlowBoundaryNodes) {
    issues.push({
      id: `workflow-node-family-${node.id}`,
      scope: 'node',
      level: 'error',
      nodeId: node.id,
      sectionKey: null,
      fieldKey: null,
      title: '节点类型不属于 Workflow',
      message: `Workflow 草稿不能包含 ${node.type} 节点。`
    });
  }

  return issues;
}
