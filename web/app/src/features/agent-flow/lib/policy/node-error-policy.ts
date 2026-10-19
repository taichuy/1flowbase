import type { FlowNodeDocument, FlowNodeType } from '@1flowbase/flow-schema';

import { i18nText } from '../../../../shared/i18n/text';

export const ERROR_BRANCH_SOURCE_HANDLE = 'error';

function nodeSupportsErrorPolicy(nodeType: FlowNodeType) {
  return nodeType !== 'start';
}

export function nodeUsesErrorBranch(
  node: Pick<FlowNodeDocument, 'type' | 'config'>
) {
  return (
    nodeSupportsErrorPolicy(node.type) &&
    node.config.error_policy === 'error_branch'
  );
}

export function getCommonErrorBranchSourceHandle(
  node: Pick<FlowNodeDocument, 'type' | 'config'>
): { id: string; title: string } | null {
  if (!nodeUsesErrorBranch(node)) {
    return null;
  }

  return {
    id: ERROR_BRANCH_SOURCE_HANDLE,
    title: i18nText('agentFlow', 'auto.exception_branch_handle')
  };
}
