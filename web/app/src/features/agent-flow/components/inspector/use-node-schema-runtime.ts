import { useCurrentDocumentAnalysis } from '../../api/analysis/context';
import { useMemo } from 'react';

import { createAgentFlowNodeSchemaAdapter } from '../../schema/node-schema-adapter';
import { resolveAgentFlowNodeSchema } from '../../schema/node-schema-registry';
import { useAgentFlowEditorStore } from '../../store/editor/provider';
import {
  selectSelectedNodeId,
  selectWorkingDocument
} from '../../store/editor/selectors';

export function useNodeSchemaRuntime(enabled = true) {
  const { analysis } = useCurrentDocumentAnalysis();
  const document = useAgentFlowEditorStore(selectWorkingDocument);
  const selectedNodeId = useAgentFlowEditorStore(selectSelectedNodeId);
  const setWorkingDocument = useAgentFlowEditorStore(
    (state) => state.setWorkingDocument
  );
  const messages = useAgentFlowEditorStore((state) => state.messages);
  const selectedNode = selectedNodeId
    ? (document.graph.nodes.find((node) => node.id === selectedNodeId) ?? null)
    : null;
  const schema = useMemo(
    () =>
      enabled && selectedNode
        ? resolveAgentFlowNodeSchema(selectedNode.type)
        : null,
    [enabled, selectedNode]
  );
  const adapter = useMemo(
    () =>
      enabled && selectedNodeId
        ? createAgentFlowNodeSchemaAdapter({
            document,
            analysis,
            nodeId: selectedNodeId,
            setWorkingDocument,
            messages,
            dispatch: () => undefined
          })
        : null,
    [analysis, document, enabled, messages, selectedNodeId, setWorkingDocument]
  );

  return {
    document,
    selectedNodeId,
    selectedNode,
    schema,
    adapter
  };
}
