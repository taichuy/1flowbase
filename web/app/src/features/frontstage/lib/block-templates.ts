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
    title: 'JSX 示例区块',
    description: '从最小、可运行的 JSX 示例开始。'
  },
  {
    id: 'data-table',
    title: 'Data Table',
    description: 'Render records from a data query as a controlled table.'
  },
  {
    id: 'create-form',
    title: 'Create Form',
    description: 'Collect form state and create a data record.'
  },
  {
    id: 'edit-form',
    title: 'Edit Form',
    description: 'Load a record, edit state, and update data.'
  },
  {
    id: 'search-table',
    title: 'Search Table',
    description: 'Search records and trigger row actions from a table.'
  }
] as const satisfies readonly FrontstageBuiltInJsBlockTemplate[];

type TemplateCodeFactory = (
  input: CreateFrontstageBuiltInJsBlockTemplateCodeInput
) => string;

const templateFactories: Record<
  FrontstageBuiltInJsBlockTemplateId,
  TemplateCodeFactory
> = {
  blank: createBlankTemplateCode,
  'data-table': createDataTableTemplateCode,
  'create-form': createCreateFormTemplateCode,
  'edit-form': createEditFormTemplateCode,
  'search-table': createSearchTableTemplateCode
};

export function listFrontstageBuiltInJsBlockTemplates(): FrontstageBuiltInJsBlockTemplateList {
  return FRONTSTAGE_BUILT_IN_JS_BLOCK_TEMPLATES.map((template) => ({
    ...template
  }));
}

export function createFrontstageBuiltInJsBlockTemplateCode(
  input: CreateFrontstageBuiltInJsBlockTemplateCodeInput
): string {
  const factory =
    templateFactories[input.templateId as FrontstageBuiltInJsBlockTemplateId];

  if (!factory) {
    throw new Error(
      `Unknown FrontStage built-in JS block template: ${String(
        input.templateId
      )}`
    );
  }

  return factory(input);
}

export function createBlankJsBlockTemplateCode(
  input: CreateBlankJsBlockTemplateCodeInput
): string {
  return createFrontstageBuiltInJsBlockTemplateCode({
    ...input,
    templateId: 'blank'
  });
}

function createBlankTemplateCode(
  input: CreateFrontstageBuiltInJsBlockTemplateCodeInput
): string {
  return `${createTemplateHeader(input)}
import { Stack, Text, Title } from '@1flowbase/block-renderer/antd-facade';

export default defineBlock({
  id: ${quoteJsString(input.blockId)},
  title: 'JSX 示例区块',

  render() {
    return (
      <Stack>
        <Title>JSX 示例区块</Title>
        <Text>点击区块右上角的编辑图标修改这段 JSX。</Text>
      </Stack>
    );
  }
});
`;
}

function createDataTableTemplateCode(
  input: CreateFrontstageBuiltInJsBlockTemplateCodeInput
): string {
  return `${createTemplateHeader(input)}
import { Button, Stack, Table, Text, Title } from '@1flowbase/block-renderer/antd-facade';

export default defineBlock({
  id: ${quoteJsString(input.blockId)},
  title: 'Data Table',
  initialState: {
    rows: []
  },

  async render(ctx) {
    const rows = Array.isArray(ctx.state.rows) ? ctx.state.rows : [];

    if (ctx.props.__example === true) {
      const result = await ctx.data.query('frontstage.data_model.record.list', {
        model: 'orders',
        page: 1,
        page_size: 20
      });
      const nextRows = Array.isArray(result.items) ? result.items : [];
      ctx.patch({ rows: nextRows });
      ctx.events.emit('orders.loaded', { count: nextRows.length });
    }

    return (
      <Stack>
        <Title>Data Table</Title>
        <Text>Query records and render them in a table.</Text>
        <Table
          rowKey="id"
          columns={[
            { key: 'name', title: 'Name', dataIndex: 'name' },
            { key: 'status', title: 'Status', dataIndex: 'status' }
          ]}
          dataSource={rows}
          permissions={{ data: ['query'], events: ['orders.loaded'] }}
        />
        <Button
          actionId="frontstage.data_model.record.list"
          actionPayload={{ model: 'orders', page: 1, page_size: 20 }}
        >
          Refresh
        </Button>
      </Stack>
    );
  }
});
`;
}

function createCreateFormTemplateCode(
  input: CreateFrontstageBuiltInJsBlockTemplateCodeInput
): string {
  return `${createTemplateHeader(input)}
import { Button, Form, FormItem, Input, Stack, Text, Title } from '@1flowbase/block-renderer/antd-facade';

export default defineBlock({
  id: ${quoteJsString(input.blockId)},
  title: 'Create Form',
  initialState: {
    draft: {
      name: '',
      status: 'draft'
    }
  },

  async render(ctx) {
    const draft = isRecord(ctx.state.draft) ? ctx.state.draft : {};

    if (ctx.props.__example === true) {
      ctx.patch({ draft: { ...draft, status: 'ready' } });
      const created = await ctx.actions.invoke('frontstage.data_model.record.create', {
        model: 'orders',
        values: draft
      });
      ctx.events.emit('orders.created', { id: created.record.id });
    }

    return (
      <Stack>
        <Title>Create Form</Title>
        <Text>Collect values and create a record.</Text>
        <Form
          layout="vertical"
          permissions={{ actions: ['frontstage.data_model.record.create'] }}
        >
          <FormItem name="name" label="Name">
            <Input value={draft.name} />
          </FormItem>
          <FormItem name="status" label="Status">
            <Input value={draft.status} />
          </FormItem>
        </Form>
        <Button
          actionId="frontstage.data_model.record.create"
          actionPayload={{ model: 'orders', values: draft }}
        >
          Create
        </Button>
      </Stack>
    );
  }
});

function isRecord(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}
`;
}

function createEditFormTemplateCode(
  input: CreateFrontstageBuiltInJsBlockTemplateCodeInput
): string {
  return `${createTemplateHeader(input)}
import { Button, Form, FormItem, Input, Stack, Text, Title } from '@1flowbase/block-renderer/antd-facade';

export default defineBlock({
  id: ${quoteJsString(input.blockId)},
  title: 'Edit Form',
  initialState: {
    record: null
  },

  async render(ctx) {
    const record = isRecord(ctx.state.record) ? ctx.state.record : {};
    const recordId = typeof ctx.params.recordId === 'string' ? ctx.params.recordId : 'record-id';

    if (ctx.props.__example === true) {
      const loaded = await ctx.data.query('frontstage.data_model.record.get', {
        model: 'orders',
        record_id: recordId
      });
      const nextRecord = isRecord(loaded.record) ? loaded.record : {};
      ctx.patch({ record: nextRecord });
      await ctx.actions.invoke('frontstage.data_model.record.update', {
        model: 'orders',
        record_id: recordId,
        values: nextRecord
      });
    }

    return (
      <Stack>
        <Title>Edit Form</Title>
        <Text>Load a record and submit updates.</Text>
        <Form
          layout="vertical"
          permissions={{
            data: ['query'],
            actions: ['frontstage.data_model.record.update']
          }}
        >
          <FormItem name="name" label="Name">
            <Input value={record.name} />
          </FormItem>
          <FormItem name="status" label="Status">
            <Input value={record.status} />
          </FormItem>
        </Form>
        <Button
          actionId="frontstage.data_model.record.update"
          actionPayload={{ model: 'orders', record_id: recordId, values: record }}
        >
          Save
        </Button>
      </Stack>
    );
  }
});

function isRecord(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}
`;
}

function createSearchTableTemplateCode(
  input: CreateFrontstageBuiltInJsBlockTemplateCodeInput
): string {
  return `${createTemplateHeader(input)}
import { Button, Form, FormItem, Input, Stack, Table, Text, Title } from '@1flowbase/block-renderer/antd-facade';

export default defineBlock({
  id: ${quoteJsString(input.blockId)},
  title: 'Search Table',
  initialState: {
    query: '',
    rows: []
  },

  async render(ctx) {
    const query = typeof ctx.state.query === 'string' ? ctx.state.query : '';
    const rows = Array.isArray(ctx.state.rows) ? ctx.state.rows : [];

    if (ctx.props.__example === true) {
      ctx.patch({ query });
      const result = await ctx.data.query('frontstage.data_model.record.list', {
        model: 'orders',
        filter: { name: { $contains: query } },
        page: 1,
        page_size: 20
      });
      const nextRows = Array.isArray(result.items) ? result.items : [];
      ctx.patch({ rows: nextRows });
      ctx.events.emit('orders.search', { query });
    }

    return (
      <Stack>
        <Title>Search Table</Title>
        <Text>Search records and handle row actions.</Text>
        <Form layout="inline">
          <FormItem name="query" label="Keyword">
            <Input value={query} />
          </FormItem>
          <Button
            actionId="frontstage.data_model.record.list"
            actionPayload={{ model: 'orders', page: 1, page_size: 20 }}
          >
            Search
          </Button>
        </Form>
        <Table
          rowKey="id"
          columns={[
            { key: 'name', title: 'Name', dataIndex: 'name' },
            { key: 'status', title: 'Status', dataIndex: 'status' }
          ]}
          dataSource={rows}
          permissions={{ data: ['query'], events: ['orders.search'] }}
        />
      </Stack>
    );
  }
});
`;
}

function createTemplateHeader(
  input: CreateFrontstageBuiltInJsBlockTemplateCodeInput
): string {
  return `import { defineBlock } from '@1flowbase/block-sdk';

// blockId: ${quoteJsString(input.blockId)}
// codeRef: ${quoteJsString(input.codeRef)}
// contributionCode: ${quoteJsString(input.contributionCode)}
`;
}

function quoteJsString(value: string): string {
  return `'${value
    .replaceAll('\\', '\\\\')
    .replaceAll("'", "\\'")
    .replaceAll('\r', '\\r')
    .replaceAll('\n', '\\n')}'`;
}
