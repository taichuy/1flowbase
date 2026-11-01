import windowWorkspaceStyles from '../../../../shared/ui/window-workspace/window-workspace.css?inline';
import editorShellStyles from '../../../agent-flow/components/editor/styles/shell.css?inline';
import dockPanelStyles from '../../../agent-flow/components/editor/styles/dock-panel.css?inline';
import conversationLogStyles from '../../../agent-flow/components/debug-console/conversation-log-panel.css?inline';
import debugMessageStyles from '../../../agent-flow/components/debug-console/conversation/debug-message.css?inline';
import providerTrajectoryStyles from '../../../agent-flow/components/debug-console/trajectory/provider-trajectory.css?inline';
import workflowTrajectoryStyles from '../../../agent-flow/components/debug-console/trajectory/workflow/workflow-trajectory.css?inline';
import clientTrajectoryStyles from '../../../agent-flow/components/debug-console/trajectory/client/client-trajectory.css?inline';
import trajectoryVirtualListStyles from '../../../agent-flow/components/debug-console/trajectory/virtual/trajectory-virtual-list.css?inline';
import readOnlyConversationInputStyles from '../../../agent-flow/components/debug-console/conversation/read-only/read-only-conversation-input.css?inline';
import runDetailStyles from './application-run-detail-panel.css?inline';
import applicationLogsStyles from '../../pages/application-logs-page.css?inline';

import jsonPreviewStyles from '../../../../shared/ui/json-preview/json-preview-block.css?inline';
import collapseShellStyles from '../../../../shared/ui/collapse-shell/collapse-shell.css?inline';
import monacoEditorStyles from 'monaco-editor/min/vs/editor/editor.main.css?inline';

export const shadowViewerStyles = [
  jsonPreviewStyles,
  collapseShellStyles,
  monacoEditorStyles,
  windowWorkspaceStyles,
  editorShellStyles,
  dockPanelStyles,
  conversationLogStyles,
  debugMessageStyles,
  providerTrajectoryStyles,
  workflowTrajectoryStyles,
  clientTrajectoryStyles,
  trajectoryVirtualListStyles,
  readOnlyConversationInputStyles,
  runDetailStyles,
  applicationLogsStyles
].join('\n');
