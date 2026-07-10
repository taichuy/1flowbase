import {
  Descriptions,
  Drawer,
  Empty,
  Space,
  Tabs,
  Tag,
  Typography
} from 'antd';
import type { DescriptionsProps } from 'antd';
import type { FC, ReactNode } from 'react';

import type {
  FrontstageBlockBasicConfiguration,
  FrontstageBlockCodeConfiguration,
  FrontstageBlockConfigurationModel,
  FrontstageBlockContextConfiguration,
  FrontstageBlockDataConfiguration,
  FrontstageBlockLimitsConfiguration
} from '../lib/block-configuration';
import { i18nText } from '../../../shared/i18n/text';

export interface BlockConfigurationDrawerProps {
  open: boolean;
  onClose: () => void;
  model: FrontstageBlockConfigurationModel | null;
}

function asText(value: unknown, fallback = i18nText("frontstage", "auto.not_configured")): string {
  if (value === null || value === undefined) {
    return fallback;
  }

  if (typeof value === 'string') {
    const trimmed = value.trim();
    return trimmed.length > 0 ? trimmed : fallback;
  }

  if (
    typeof value === 'number' ||
    typeof value === 'boolean' ||
    typeof value === 'bigint'
  ) {
    return String(value);
  }

  return i18nText("frontstage", "auto.configured");
}

function formatFieldCount(count: number): string {
  return i18nText("frontstage", "auto.field_count", { value1: String(count) });
}

function formatPagination(value: unknown): string {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    return i18nText("frontstage", "auto.not_configured");
  }

  const entries = Object.entries(value).filter(
    ([, entry]) => entry !== null && entry !== undefined
  );
  if (entries.length === 0) {
    return i18nText("frontstage", "auto.not_configured");
  }

  return entries.map(([key, entry]) => `${key} ${asText(entry)}`).join(', ');
}

function formatRuntime(runtime: FrontstageBlockCodeConfiguration['runtime']) {
  return (
    <Space size={6} wrap>
      <Tag>{runtime.kind}</Tag>
      <Typography.Text>{asText(runtime.entry)}</Typography.Text>
      <Typography.Text type="secondary">
        {i18nText("frontstage", "auto.hint_with_value", { value1: runtime.hint })}
      </Typography.Text>
    </Space>
  );
}

function renderTags(values: readonly string[], emptyText: string): ReactNode {
  if (values.length === 0) {
    return <Typography.Text type="secondary">{emptyText}</Typography.Text>;
  }

  return (
    <Space size={4} wrap>
      {values.map((value) => (
        <Tag key={value}>{value}</Tag>
      ))}
    </Space>
  );
}

function renderOperationState(enabled: boolean): ReactNode {
  return (
    <Tag color={enabled ? 'success' : 'default'}>
      {enabled ? i18nText("frontstage", "auto.enabled") : i18nText("frontstage", "auto.disabled")}
    </Tag>
  );
}

function renderDescriptions(items: DescriptionsProps['items']) {
  return <Descriptions column={1} size="small" items={items} />;
}

function getSectionModel<T>(
  model: FrontstageBlockConfigurationModel,
  sectionId: string
): T {
  const section = model.sections.find((item) => item.id === sectionId);
  return section?.model as T;
}

function BasicSection({ model }: { model: FrontstageBlockBasicConfiguration }) {
  return (
    <div data-testid="frontstage-block-configuration-section-basic">
      {renderDescriptions([
        { key: 'block-id', label: i18nText("frontstage", "auto.block_id"), children: model.blockId },
        {
          key: 'source-id',
          label: i18nText("frontstage", "auto.source_id"),
          children: asText(model.sourceId)
        },
        {
          key: 'title',
          label: i18nText("frontstage", "auto.title"),
          children: model.title.value ?? model.title.placeholder
        },
        {
          key: 'description',
          label: i18nText("frontstage", "auto.description"),
          children: model.description.value ?? model.description.placeholder
        },
        { key: 'code-ref', label: i18nText("frontstage", "auto.code_ref"), children: model.codeRef },
        {
          key: 'source-code-ref',
          label: i18nText("frontstage", "auto.source_code_ref"),
          children: asText(model.sourceCodeRef)
        },
        { key: 'width', label: i18nText("frontstage", "auto.width"), children: asText(model.width) },
        { key: 'height', label: i18nText("frontstage", "auto.height"), children: asText(model.height) },
        { key: 'order', label: i18nText("frontstage", "auto.order"), children: asText(model.order) }
      ])}
    </div>
  );
}

function DataSection({ model }: { model: FrontstageBlockDataConfiguration }) {
  return (
    <div data-testid="frontstage-block-configuration-section-data">
      {renderDescriptions([
        { key: 'model', label: i18nText("frontstage", "auto.model"), children: asText(model.model) },
        {
          key: 'fields',
          label: i18nText("frontstage", "auto.fields"),
          children: formatFieldCount(model.fields.length)
        },
        {
          key: 'query',
          label: i18nText("frontstage", "auto.query"),
          children: renderOperationState(model.operations.query.enabled)
        },
        {
          key: 'create',
          label: i18nText("frontstage", "auto.create"),
          children: renderOperationState(model.operations.create.enabled)
        },
        {
          key: 'update',
          label: i18nText("frontstage", "auto.update"),
          children: renderOperationState(model.operations.update.enabled)
        },
        {
          key: 'delete',
          label: i18nText("frontstage", "auto.delete"),
          children: renderOperationState(model.operations.delete.enabled)
        },
        { key: 'filter', label: i18nText("frontstage", "auto.filter"), children: asText(model.filter) },
        { key: 'sort', label: i18nText("frontstage", "auto.sort"), children: asText(model.sort) },
        {
          key: 'pagination',
          label: i18nText("frontstage", "auto.pagination"),
          children: formatPagination(model.pagination)
        }
      ])}
    </div>
  );
}

function CodeSection({ model }: { model: FrontstageBlockCodeConfiguration }) {
  return (
    <div data-testid="frontstage-block-configuration-section-code">
      {renderDescriptions([
        { key: 'code-ref', label: i18nText("frontstage", "auto.code_ref"), children: model.codeRef },
        {
          key: 'source-code-ref',
          label: i18nText("frontstage", "auto.source_code_ref"),
          children: asText(model.sourceCodeRef)
        },
        {
          key: 'runtime',
          label: i18nText("frontstage", "auto.runtime"),
          children: formatRuntime(model.runtime)
        },
        {
          key: 'catalog-id',
          label: i18nText("frontstage", "auto.catalog_id"),
          children: asText(model.contribution.catalogId)
        },
        {
          key: 'catalog-title',
          label: i18nText("frontstage", "auto.catalog_title"),
          children: asText(model.contribution.catalogTitle)
        },
        {
          key: 'contribution',
          label: i18nText("frontstage", "auto.contribution"),
          children: model.contribution.code
        },
        {
          key: 'plugin',
          label: i18nText("frontstage", "auto.plugin"),
          children: asText(model.contribution.pluginId)
        },
        {
          key: 'provider',
          label: i18nText("frontstage", "auto.provider"),
          children: asText(model.contribution.providerCode)
        },
        {
          key: 'template',
          label: i18nText("frontstage", "auto.template"),
          children: asText(model.template.id)
        }
      ])}
    </div>
  );
}

function ContextSection({
  model
}: {
  model: FrontstageBlockContextConfiguration;
}) {
  return (
    <div data-testid="frontstage-block-configuration-section-context">
      {renderDescriptions([
        {
          key: 'catalog',
          label: i18nText("frontstage", "auto.catalog"),
          children: model.catalog.available ? i18nText("frontstage", "auto.matched") : i18nText("frontstage", "auto.not_matched")
        },
        {
          key: 'primitives',
          label: i18nText("frontstage", "auto.primitives"),
          children: renderTags(model.catalog.primitives, i18nText("frontstage", "auto.no_primitives"))
        },
        {
          key: 'input-schema',
          label: i18nText("frontstage", "auto.input_schema"),
          children: asText(model.catalog.inputSchema.type)
        },
        {
          key: 'ctx-current-user',
          label: i18nText("frontstage", "auto.current_user"),
          children: model.ctx.currentUser.path
        },
        { key: 'ctx-page', label: i18nText("frontstage", "auto.page"), children: model.ctx.page.path },
        { key: 'ctx-params', label: i18nText("frontstage", "auto.params"), children: model.ctx.params.path },
        { key: 'ctx-props', label: i18nText("frontstage", "auto.props"), children: model.ctx.props.path },
        { key: 'ctx-state', label: i18nText("frontstage", "auto.state"), children: model.ctx.state.path },
        {
          key: 'ctx-data',
          label: i18nText("frontstage", "auto.data"),
          children: (
            <Space size={4} wrap>
              <Typography.Text>{model.ctx.data.path}</Typography.Text>
              {renderTags(model.ctx.data.models, i18nText("frontstage", "auto.no_allowed_data_models"))}
              {renderTags(
                model.ctx.data.operations,
                i18nText("frontstage", "auto.no_allowed_data_operations")
              )}
            </Space>
          )
        },
        {
          key: 'ctx-actions',
          label: i18nText("frontstage", "auto.actions"),
          children: (
            <Space size={4} wrap>
              <Typography.Text>{model.ctx.actions.path}</Typography.Text>
              {renderTags(model.ctx.actions.allowed, i18nText("frontstage", "auto.no_allowed_actions"))}
            </Space>
          )
        },
        {
          key: 'ctx-events',
          label: i18nText("frontstage", "auto.events"),
          children: (
            <Space size={4} wrap>
              <Typography.Text>{model.ctx.events.path}</Typography.Text>
              {renderTags(model.ctx.events.allowed, i18nText("frontstage", "auto.no_allowed_events"))}
            </Space>
          )
        }
      ])}
    </div>
  );
}

function LimitsSection({
  model
}: {
  model: FrontstageBlockLimitsConfiguration;
}) {
  return (
    <div data-testid="frontstage-block-configuration-section-limits">
      {renderDescriptions([
        {
          key: 'timeout',
          label: i18nText("frontstage", "auto.timeout"),
          children:
            model.timeoutMs === null
              ? i18nText("frontstage", "auto.not_configured")
              : `${model.timeoutMs} ms`
        },
        {
          key: 'render-depth',
          label: i18nText("frontstage", "auto.max_render_depth"),
          children: asText(model.maxRenderDepth)
        },
        {
          key: 'render-nodes',
          label: i18nText("frontstage", "auto.max_render_nodes"),
          children: asText(model.maxRenderNodes)
        },
        {
          key: 'event-chain-depth',
          label: i18nText("frontstage", "auto.max_event_chain_depth"),
          children: asText(model.maxEventChainDepth)
        },
        {
          key: 'actions',
          label: i18nText("frontstage", "auto.allowed_actions"),
          children: renderTags(model.allowedActions, i18nText("frontstage", "auto.no_allowed_actions"))
        },
        {
          key: 'events',
          label: i18nText("frontstage", "auto.allowed_events"),
          children: renderTags(model.allowedEvents, i18nText("frontstage", "auto.no_allowed_events"))
        },
        {
          key: 'data-models',
          label: i18nText("frontstage", "auto.allowed_data_models"),
          children: renderTags(
            model.allowedQueries,
            i18nText("frontstage", "auto.no_allowed_data_models")
          )
        },
        {
          key: 'data-operations',
          label: i18nText("frontstage", "auto.allowed_data_operations"),
          children: renderTags(
            model.allowedDataOperations,
            i18nText("frontstage", "auto.no_allowed_data_operations")
          )
        }
      ])}
    </div>
  );
}

export const BlockConfigurationDrawer: FC<BlockConfigurationDrawerProps> = ({
  open,
  onClose,
  model
}) => {
  const items = model
    ? [
        {
          key: 'basic',
          label: i18nText("frontstage", "auto.basic"),
          children: (
            <BasicSection
              model={getSectionModel<FrontstageBlockBasicConfiguration>(
                model,
                'basic'
              )}
            />
          )
        },
        {
          key: 'data',
          label: i18nText("frontstage", "auto.data"),
          children: (
            <DataSection
              model={getSectionModel<FrontstageBlockDataConfiguration>(
                model,
                'data'
              )}
            />
          )
        },
        {
          key: 'code',
          label: i18nText("frontstage", "auto.code"),
          children: (
            <CodeSection
              model={getSectionModel<FrontstageBlockCodeConfiguration>(
                model,
                'code'
              )}
            />
          )
        },
        {
          key: 'context',
          label: i18nText("frontstage", "auto.context"),
          children: (
            <ContextSection
              model={getSectionModel<FrontstageBlockContextConfiguration>(
                model,
                'context'
              )}
            />
          )
        },
        {
          key: 'limits',
          label: i18nText("frontstage", "auto.limits"),
          children: (
            <LimitsSection
              model={getSectionModel<FrontstageBlockLimitsConfiguration>(
                model,
                'limits'
              )}
            />
          )
        }
      ]
    : [];

  return (
    <Drawer
      open={open}
      onClose={onClose}
      placement="right"
      title={i18nText("frontstage", "auto.block_configuration")}
      width={640}
    >
      {model ? (
        <Space direction="vertical" size={12} style={{ width: '100%' }}>
          <Space direction="vertical" size={2}>
            <Typography.Text type="secondary" style={{ fontSize: 12 }}>
              {i18nText("frontstage", "auto.block")}
            </Typography.Text>
            <Typography.Text strong>{model.blockId}</Typography.Text>
            <Typography.Text type="secondary" style={{ fontSize: 12 }}>
              {i18nText("frontstage", "auto.code_ref_with_value", { value1: model.codeRef })}
            </Typography.Text>
          </Space>
          <Tabs items={items} />
        </Space>
      ) : (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={i18nText("frontstage", "auto.select_block_for_configuration")}
        />
      )}
    </Drawer>
  );
};
