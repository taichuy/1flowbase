export type FrontstageBuiltInJsBlockTemplateId =
  | 'blank'
  | 'data-table'
  | 'create-form'
  | 'edit-form'
  | 'search-table';

export interface FrontstageBuiltInJsBlockTemplate {
  id: FrontstageBuiltInJsBlockTemplateId;
  title: string;
  description: string;
}

export type FrontstageBuiltInJsBlockTemplateList =
  readonly FrontstageBuiltInJsBlockTemplate[];

export interface CreateFrontstageBuiltInJsBlockTemplateCodeInput {
  templateId: FrontstageBuiltInJsBlockTemplateId;
  blockId: string;
  codeRef: string;
  contributionCode: string;
}

export type CreateBlankJsBlockTemplateCodeInput = Omit<
  CreateFrontstageBuiltInJsBlockTemplateCodeInput,
  'templateId'
>;

export const FRONTSTAGE_BUILT_IN_JS_BLOCK_TEMPLATES = [
  {
    id: 'blank',
    title: '代码示例区块',
    description: '从最小、可运行的 TSX 示例开始。'
  },
  {
    id: 'data-table',
    title: 'Data Table',
    description: '连接列表接口后呈现结构化数据。'
  },
  {
    id: 'create-form',
    title: 'Create Form',
    description: '连接写接口后提交结构化表单。'
  },
  {
    id: 'edit-form',
    title: 'Edit Form',
    description: '组合读取与写入接口编辑实体。'
  },
  {
    id: 'search-table',
    title: 'Search Table',
    description: '通过查询参数筛选并呈现数据。'
  }
] as const satisfies readonly FrontstageBuiltInJsBlockTemplate[];

export function listFrontstageBuiltInJsBlockTemplates(): FrontstageBuiltInJsBlockTemplateList {
  return FRONTSTAGE_BUILT_IN_JS_BLOCK_TEMPLATES.map((template) => ({
    ...template
  }));
}

export function createFrontstageBuiltInJsBlockTemplateCode(
  input: CreateFrontstageBuiltInJsBlockTemplateCodeInput
): string {
  const template = FRONTSTAGE_BUILT_IN_JS_BLOCK_TEMPLATES.find(
    (item) => item.id === input.templateId
  );
  if (!template) {
    throw new Error(
      `Unknown FrontStage built-in JS block template: ${String(input.templateId)}`
    );
  }
  return createModuleTemplate(template.title, template.description);
}

export function createBlankJsBlockTemplateCode(
  input: CreateBlankJsBlockTemplateCodeInput
): string {
  return createFrontstageBuiltInJsBlockTemplateCode({
    ...input,
    templateId: 'blank'
  });
}

function createModuleTemplate(title: string, description: string): string {
  return `import type {
  BlockContext,
  BlockModule,
  BlockResult
} from '@1flowbase/block-sdk';

import {
  Stack,
  Text,
  Title
} from '@1flowbase/block-renderer/antd-facade';

async function main(_ctx: BlockContext): Promise<BlockResult> {
  return {
    view: (
      <Stack>
        <Title>${escapeJsxText(title)}</Title>
        <Text>${escapeJsxText(description)}</Text>
      </Stack>
    ),
    outputs: {}
  };
}

/**
 * @1flowbase-context
 * inputs: 无
 * interfaces: 无
 * outputs: 无
 */
export default {
  main
} satisfies BlockModule;
`;
}

function escapeJsxText(value: string): string {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;');
}
