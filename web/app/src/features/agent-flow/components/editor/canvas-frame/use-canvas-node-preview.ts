import { useMutation, useQueryClient } from '@tanstack/react-query';
import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';
import { App } from 'antd';
import { useCallback, useEffect, useRef, useState } from 'react';

import { useAgentFlowDebugSession } from '../../../hooks/runtime/useAgentFlowDebugSession';
import {
  applicationRunNodeLastRunQueryKey,
  buildNodeDebugPreviewPlan,
  buildNodeDebugVariableConfirmationPlan,
  nodeLastRunToFlowDebugRunDetail,
  nodeLastRunQueryKey,
  startNodeDebugPreview,
  type NodeDebugPreviewPlan,
  type NodeDebugPreviewVariableCache
} from '../../../api/runtime';

import {
  environmentVariableNodeId,
  type AgentFlowEnvironmentVariable
} from '../../../lib/variables/application-environment-variables';

import { systemVariableNodeId } from '../../../lib/variables/system-variables';

import { useAuthStore } from '../../../../../state/auth-store';

import { i18nText } from '../../../../../shared/i18n/text';

import { getDocumentWithLatestViewport } from './document';

import type { RefObject } from 'react';

interface CanvasNodePreviewInput {
  applicationId: string;
  flowId: string;
  selectedNodeId: string | null;
  environmentVariables: AgentFlowEnvironmentVariable[];
  debugSession: ReturnType<typeof useAgentFlowDebugSession>;
  documentRef: RefObject<FlowAuthoringDocument>;
  viewportGetterRef: RefObject<
    (() => FlowAuthoringDocument['editor']['viewport']) | null
  >;
  viewportSnapshotRef: RefObject<FlowAuthoringDocument['editor']['viewport']>;
}

type NodePreviewAction = 'run' | 'debug';

/** Owns preview planning, missing-variable confirmation, and execution receipts. */
export function useCanvasNodePreview({
  applicationId,
  flowId,
  selectedNodeId,
  environmentVariables,
  debugSession,
  documentRef,
  viewportGetterRef,
  viewportSnapshotRef
}: CanvasNodePreviewInput) {
  const { message } = App.useApp();
  const queryClient = useQueryClient();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const actorUserId = useAuthStore(
    (state) => state.actor?.id ?? state.me?.id ?? null
  );
  const runNodeFromCanvasRef = useRef<(nodeId: string) => void>(() => {});
  const [nodeDetailTab, setNodeDetailTab] = useState<'config' | 'lastRun'>(
    'config'
  );
  const [pendingNodePreview, setPendingNodePreview] = useState<{
    action: NodePreviewAction;
    nodeId: string;
    inputPayload: NodeDebugPreviewPlan['input_payload'];
    fields: NodeDebugPreviewPlan['missing_fields'];
  } | null>(null);
  const handleCanvasRunNode = useCallback((nodeId: string) => {
    runNodeFromCanvasRef.current(nodeId);
  }, []);
  const nodePreviewMutation = useMutation({
    mutationFn: async ({
      nodeId,
      inputPayload
    }: {
      action: NodePreviewAction;
      nodeId: string;
      inputPayload: Record<string, Record<string, unknown>>;
    }) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      return startNodeDebugPreview(
        applicationId,
        nodeId,
        {
          input_payload: inputPayload,
          document: getDocumentWithLatestViewport(
            documentRef.current,
            viewportGetterRef.current?.() ?? viewportSnapshotRef.current
          ),
          debug_session_id: debugSession.debugSessionId
        },
        csrfToken
      );
    },
    onSuccess: async (lastRun, variables) => {
      queryClient.setQueryData(
        nodeLastRunQueryKey(applicationId, variables.nodeId),
        lastRun
      );
      queryClient.setQueryData(
        applicationRunNodeLastRunQueryKey(
          applicationId,
          lastRun.flow_run.id,
          variables.nodeId
        ),
        lastRun
      );
      debugSession.rememberExternalRunDetail(
        nodeLastRunToFlowDebugRunDetail(lastRun)
      );
      setNodeDetailTab('lastRun');
      await queryClient.invalidateQueries({
        queryKey: ['applications', applicationId, 'runtime']
      });
    },
    onError(error) {
      message.error(
        error instanceof Error && error.message
          ? error.message
          : i18nText('agentFlow', 'auto.debug_run_failed')
      );
    }
  });
  function buildNodeDebugRuntimeVariableCache(): NodeDebugPreviewVariableCache {
    const cache = debugSession.getNodePreviewVariableCache();
    const environmentPayload = environmentVariables.reduce<
      Record<string, unknown>
    >((payload, variable) => {
      payload[variable.name] = variable.value;
      return payload;
    }, {});
    const systemPayload: Record<string, unknown> = {
      conversation_id: debugSession.debugSessionId,
      dialog_count: 0,
      user_id: actorUserId ?? '',
      application_id: applicationId,
      workflow_id: flowId,
      workflow_run_id: debugSession.activeRunId ?? '',
      model_parameters: {}
    };

    return {
      ...cache,
      [environmentVariableNodeId]: {
        ...environmentPayload,
        ...(cache[environmentVariableNodeId] ?? {})
      },
      [systemVariableNodeId]: {
        ...systemPayload,
        ...(cache[systemVariableNodeId] ?? {})
      }
    };
  }

  function runNodePreview(
    action: NodePreviewAction,
    nodeId: string,
    inputPayload: Record<string, Record<string, unknown>>
  ) {
    debugSession.rememberNodePreviewInputs(inputPayload);
    nodePreviewMutation.mutate({ action, nodeId, inputPayload });
  }

  function handleRunNode(nodeId: string) {
    const plan = buildNodeDebugPreviewPlan(
      documentRef.current,
      nodeId,
      buildNodeDebugRuntimeVariableCache()
    );

    if (plan.missing_fields.length > 0) {
      setPendingNodePreview({
        action: 'run',
        nodeId,
        inputPayload: plan.input_payload,
        fields: plan.missing_fields
      });
      return;
    }

    runNodePreview('run', nodeId, plan.input_payload);
  }

  useEffect(() => {
    runNodeFromCanvasRef.current = handleRunNode;
  });

  function handleDebugNode(nodeId: string) {
    const plan = buildNodeDebugVariableConfirmationPlan(
      documentRef.current,
      nodeId,
      buildNodeDebugRuntimeVariableCache()
    );

    if (plan.fields.length > 0) {
      setPendingNodePreview({
        action: 'debug',
        nodeId,
        inputPayload: plan.input_payload,
        fields: plan.fields
      });
      return;
    }

    runNodePreview('debug', nodeId, plan.input_payload);
  }

  function handleSubmitNodePreviewVariables(
    inputPayload: Record<string, Record<string, unknown>>
  ) {
    if (!pendingNodePreview) {
      return;
    }

    const mergedInputPayload = { ...pendingNodePreview.inputPayload };

    for (const [nodeId, payload] of Object.entries(inputPayload)) {
      mergedInputPayload[nodeId] = {
        ...(mergedInputPayload[nodeId] ?? {}),
        ...payload
      };
    }

    const { action, nodeId } = pendingNodePreview;

    setPendingNodePreview(null);
    runNodePreview(action, nodeId, mergedInputPayload);
  }

  function handleRunSelectedNode() {
    if (!selectedNodeId) {
      return;
    }

    handleRunNode(selectedNodeId);
  }

  function handleDebugSelectedNode() {
    if (!selectedNodeId) {
      return;
    }

    handleDebugNode(selectedNodeId);
  }

  return {
    nodeDetailTab,
    setNodeDetailTab,
    pendingNodePreview,
    setPendingNodePreview,
    nodePreviewMutation,
    handleCanvasRunNode,
    handleSubmitNodePreviewVariables,
    handleRunSelectedNode,
    handleDebugSelectedNode
  };
}
