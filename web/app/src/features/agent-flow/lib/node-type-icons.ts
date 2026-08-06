import { createElement, type CSSProperties, type ReactNode } from 'react';

import {
  ApiOutlined,
  BlockOutlined,
  BranchesOutlined,
  DatabaseOutlined,
  EditOutlined,
  FileTextOutlined,
  MessageOutlined,
  MergeCellsOutlined,
  PlaySquareOutlined,
  QuestionCircleOutlined,
  ReloadOutlined,
  SearchOutlined,
  SwapOutlined,
  SyncOutlined,
  ThunderboltOutlined,
  ToolOutlined,
  WarningOutlined
} from '@ant-design/icons';

const CODE_NODE_ICON_STYLE: CSSProperties = {
  display: 'inline-block',
  width: '1em',
  height: '1em',
  backgroundColor: 'currentColor',
  maskImage: 'url("/icons/code-node.svg")',
  maskPosition: 'center',
  maskRepeat: 'no-repeat',
  maskSize: 'contain',
  WebkitMaskImage: 'url("/icons/code-node.svg")',
  WebkitMaskPosition: 'center',
  WebkitMaskRepeat: 'no-repeat',
  WebkitMaskSize: 'contain'
};

function codeNodeIcon() {
  return createElement('span', {
    'aria-label': 'code',
    role: 'img',
    style: CODE_NODE_ICON_STYLE
  });
}

/** 节点类型图标统一源，画布节点和详情头必须使用同一套映射。 */
const NODE_TYPE_ICONS: Record<string, ReactNode> = {
  start: createElement(PlaySquareOutlined),
  answer: createElement(MessageOutlined),
  llm: createElement(ThunderboltOutlined),
  agents: createElement(ThunderboltOutlined),
  agent: createElement(ThunderboltOutlined),
  code: codeNodeIcon(),
  template_transform: createElement(FileTextOutlined),
  knowledge_retrieval: createElement(SearchOutlined),
  question_classifier: createElement(QuestionCircleOutlined),
  if_else: createElement(SwapOutlined),
  http_request: createElement(ApiOutlined),
  tools: createElement(ToolOutlined),
  tool: createElement(ToolOutlined),
  tool_result: createElement(MessageOutlined),
  data_model_list: createElement(DatabaseOutlined),
  data_model_get: createElement(DatabaseOutlined),
  data_model_create: createElement(DatabaseOutlined),
  data_model_update: createElement(DatabaseOutlined),
  data_model_delete: createElement(DatabaseOutlined),
  sql: createElement(DatabaseOutlined),
  variable_aggregator: createElement(MergeCellsOutlined),
  variable_assigner: createElement(EditOutlined),
  iteration: createElement(SyncOutlined),
  loop: createElement(ReloadOutlined),
  plugin_node: createElement(BlockOutlined),
  unresolved_node: createElement(WarningOutlined),
  fusion: createElement(ThunderboltOutlined),
  route: createElement(ThunderboltOutlined),
  branch: createElement(ThunderboltOutlined),
  stitched_context: createElement(MergeCellsOutlined),
  flow_run: createElement(BranchesOutlined)
};

export function getAgentFlowNodeTypeIcon(nodeType: string) {
  return NODE_TYPE_ICONS[nodeType] ?? null;
}
