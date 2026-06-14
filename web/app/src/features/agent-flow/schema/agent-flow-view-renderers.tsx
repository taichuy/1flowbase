/* eslint-disable react-refresh/only-export-components */

import type { FlowNodeDocument } from '@1flowbase/flow-schema';
import {
  BookOutlined,
  DownOutlined,
  EditOutlined,
  HomeOutlined,
  PlusOutlined,
  QuestionCircleOutlined
} from '@ant-design/icons';
import {
  Button,
  Card,
  Empty,
  Input,
  Modal,
  Select,
  Space,
  Switch,
  Tooltip,
  Typography
} from 'antd';
import { useState } from 'react';

import type {
  SchemaViewRenderer,
  SchemaViewRendererProps
} from '../../../shared/schema-ui/registry/create-renderer-registry';
import { NodeRunIOCard } from '../components/detail/last-run/NodeRunIOCard';
import { NodeRunMetadataCard } from '../components/detail/last-run/NodeRunMetadataCard';
import { NodeRunSummaryCard } from '../components/detail/last-run/NodeRunSummaryCard';
import { NodeRunEmptyState } from '../components/detail/last-run/NodeRunEmptyState';
import { LlmCardModelBadge } from '../components/nodes/LlmCardModelBadge';
import type { NodeLastRun } from '../api/runtime';
import { getAgentFlowNodeTypeIcon } from '../lib/node-type-icons';
import { getBuiltinNodeRuntimeContract } from '../lib/node-definitions/contracts';
import { getLlmExternalReasoningPolicy } from '../lib/llm-node-config';
import { i18nText } from '../../../shared/i18n/text';

function getNode(adapter: SchemaViewRendererProps['adapter']) {
  return adapter.getDerived('node') as FlowNodeDocument | null | undefined;
}

function defaultOutputEditorText(value: unknown) {
  if (typeof value === 'object' && value !== null && !Array.isArray(value)) {
    return JSON.stringify(value, null, 2);
  }

  return '{}';
}

function parseDefaultOutputEditorText(value: string) {
  const parsed = JSON.parse(value) as unknown;

  if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) {
    throw new Error('default output must be an object');
  }

  return parsed as Record<string, unknown>;
}

function renderSummaryView({ adapter, block }: SchemaViewRendererProps) {
  const node = getNode(adapter);
  const meta = adapter.getDerived('definitionMeta') as
    | { summary?: string; helpHref?: string | null }
    | null
    | undefined;

  if (!node) {
    return null;
  }

  return (
    <Card
      extra={
        meta?.helpHref ? (
          <Typography.Link href={meta.helpHref} target="_blank">
            <Space size={4}>
              <BookOutlined />
              {i18nText("agentFlow", "auto.help_documentation")}</Space>
          </Typography.Link>
        ) : null
      }
      title={block.title ?? i18nText("agentFlow", "auto.node_description")}
    >
      <Typography.Paragraph>
        {meta?.summary ?? node.description ?? i18nText("agentFlow", "auto.node_description_yet")}
      </Typography.Paragraph>
    </Card>
  );
}

function renderCardEyebrowView({ adapter }: SchemaViewRendererProps) {
  const node = getNode(adapter);
  if (!node) {
    return null;
  }

  const typeIcon = getAgentFlowNodeTypeIcon(node.type);

  return (
    <div className="agent-flow-node-card__header">
      <span className="agent-flow-node-card__header-main">
        {typeIcon ? (
          <span className="agent-flow-node-card__type-icon">{typeIcon}</span>
        ) : null}
        <span className="agent-flow-node-card__title">{node.alias}</span>
      </span>
    </div>
  );
}

function renderCardModelView({ adapter }: SchemaViewRendererProps) {
  const node = getNode(adapter);

  if (!node || node.type !== 'llm') {
    return null;
  }

  return <LlmCardModelBadge node={node} />;
}

function renderCardDescriptionView({ adapter }: SchemaViewRendererProps) {
  const node = getNode(adapter);

  if (!node || node.type === 'llm') {
    return null;
  }

  const description = node.description?.trim();
  const meta = adapter.getDerived('definitionMeta') as
    | { summary?: string; helpHref?: string | null }
    | null
    | undefined;
  const contract = getBuiltinNodeRuntimeContract(node.type);
  const displayContent =
    description ||
    contract?.card.description ||
    meta?.summary ||
    i18nText("agentFlow", "auto.node_configuration_shown_here");

  return (
    <div className="agent-flow-node-card__description">{displayContent}</div>
  );
}

function OutputContractView({ adapter, block }: SchemaViewRendererProps) {
  const node = getNode(adapter);
  const [expanded, setExpanded] = useState(false);
  const outputs =
    (adapter.getValue('config.output_contract') as Array<{
      key: string;
      title: string;
      valueType: string;
    }>) ??
    node?.outputs ??
    [];

  if (!node) {
    return null;
  }

  const title = node.type === 'start' ? i18nText("agentFlow", "auto.input_field") : i18nText("agentFlow", "auto.output_variable");
  return (
    <div className="agent-flow-node-detail__section">
      <div className="agent-flow-node-detail__section-header">
        <Typography.Title
          level={5}
          className="agent-flow-node-detail__section-title"
        >
          {block.title ?? title}
        </Typography.Title>
        <Button
          aria-expanded={expanded}
          aria-label={block.title ?? title}
          className="agent-flow-node-detail__section-toggle"
          icon={
            <DownOutlined
              className={
                expanded
                  ? 'agent-flow-node-detail__section-toggle-icon agent-flow-node-detail__section-toggle-icon--expanded'
                  : 'agent-flow-node-detail__section-toggle-icon'
              }
            />
          }
          size="small"
          type="text"
          onClick={() => setExpanded((current) => !current)}
        />
      </div>
      {expanded ? (
        outputs.length > 0 ? (
          <div className="agent-flow-node-detail__list">
            {outputs.map((output) => (
              <div key={output.key} className="agent-flow-node-detail__list-item">
                <div className="agent-flow-node-detail__list-item-left">
                  <span className="agent-flow-node-detail__list-item-icon">
                    {'{x}'}
                  </span>
                  <span className="agent-flow-node-detail__list-item-name">
                    {output.key}
                  </span>
                </div>
                <span className="agent-flow-node-detail__list-item-type">
                  {output.valueType}
                </span>
              </div>
            ))}
          </div>
        ) : (
          <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description={i18nText("agentFlow", "auto.no_fields_yet")} />
        )
      ) : null}
    </div>
  );
}

function PolicyGroupView({ adapter }: SchemaViewRendererProps) {
  const node = getNode(adapter);
  const retryEnabled = Boolean(adapter.getValue('config.retry_enabled'));
  const errorPolicy =
    (adapter.getValue('config.error_policy') as string | undefined) ?? 'none';
  const [defaultOutputOpen, setDefaultOutputOpen] = useState(false);
  const [defaultOutputText, setDefaultOutputText] = useState(() =>
    defaultOutputEditorText(adapter.getValue('config.error_default_output'))
  );
  const [defaultOutputError, setDefaultOutputError] = useState<string | null>(
    null
  );
  const externalReasoningPolicy = getLlmExternalReasoningPolicy({
    external_reasoning_policy: adapter.getValue(
      'config.external_reasoning_policy'
    )
  });

  function openDefaultOutputEditor() {
    setDefaultOutputText(
      defaultOutputEditorText(adapter.getValue('config.error_default_output'))
    );
    setDefaultOutputError(null);
    setDefaultOutputOpen(true);
  }

  function saveDefaultOutput() {
    try {
      adapter.setValue(
        'config.error_default_output',
        parseDefaultOutputEditorText(defaultOutputText)
      );
      setDefaultOutputError(null);
      setDefaultOutputOpen(false);
    } catch {
      setDefaultOutputError(
        i18nText('agentFlow', 'auto.default_output_json_invalid')
      );
    }
  }

  const errorPolicyOptions = [
    {
      value: 'none',
      label: i18nText("agentFlow", "auto.none"),
      description: i18nText("agentFlow", "auto.exception_occurs_handled_node_stop_running")
    },
    {
      value: 'default_value',
      label: i18nText("agentFlow", "auto.default_value"),
      description: i18nText("agentFlow", "auto.specifies_output_content_exception_occurs")
    },
    {
      value: 'error_branch',
      label: i18nText("agentFlow", "auto.abnormal_branch"),
      description: i18nText("agentFlow", "auto.exception_occurs_exception_branch_executed")
    }
  ] satisfies Array<{ value: string; label: string; description: string }>;

  return (
    <div className="agent-flow-node-detail__policies">
      {node?.type === 'llm' ? (
        <div
          className="agent-flow-node-detail__policy-row"
          data-testid="node-policy-row"
        >
          <Typography.Text className="agent-flow-node-detail__policy-label">
            {i18nText("agentFlow", "auto.follow_external_reasoning")}
            <Tooltip title="使用外部传入推理强度">
              <QuestionCircleOutlined
                aria-label="使用外部传入推理强度"
                className="agent-flow-node-detail__policy-help"
              />
            </Tooltip>
          </Typography.Text>
          <Switch
            aria-label={i18nText("agentFlow", "auto.follow_external_reasoning")}
            checked={externalReasoningPolicy.follow_external_reasoning}
            className="agent-flow-node-detail__policy-control"
            onChange={(checked) =>
              adapter.setValue('config.external_reasoning_policy', {
                follow_external_reasoning: checked
              })
            }
          />
        </div>
      ) : null}
      <div
        className="agent-flow-node-detail__policy-row"
        data-testid="node-policy-row"
      >
        <Typography.Text className="agent-flow-node-detail__policy-label">
          {i18nText("agentFlow", "auto.retry_on_failure")}</Typography.Text>
        <Switch
          aria-label={i18nText("agentFlow", "auto.retry_on_failure")}
          checked={retryEnabled}
          className="agent-flow-node-detail__policy-control"
          onChange={(checked) =>
            adapter.setValue('config.retry_enabled', checked)
          }
        />
      </div>
      <div
        className="agent-flow-node-detail__policy-row agent-flow-node-detail__policy-row--select"
        data-testid="node-policy-row"
      >
        <Typography.Text className="agent-flow-node-detail__policy-label">
          {i18nText("agentFlow", "auto.exception_handling")}</Typography.Text>
        <div
          className="agent-flow-node-detail__policy-select-shell agent-flow-node-detail__policy-select-shell--compact"
          data-testid="node-policy-error"
        >
          <Select
            aria-label={i18nText("agentFlow", "auto.exception_handling")}
            className="agent-flow-node-detail__policy-control agent-flow-node-detail__policy-select"
            options={errorPolicyOptions}
            optionRender={(option) => {
              const policy = option.data as (typeof errorPolicyOptions)[number];

              return (
                <div className="agent-flow-node-detail__policy-option">
                  <div className="agent-flow-node-detail__policy-option-title">
                    {policy.label}
                  </div>
                  <div className="agent-flow-node-detail__policy-option-description">
                    {policy.description}
                  </div>
                </div>
              );
            }}
            classNames={{
              popup: {
                root: 'agent-flow-node-detail__policy-dropdown'
              }
            }}
            popupMatchSelectWidth={false}
            value={errorPolicy}
            onChange={(value) => {
              adapter.setValue('config.error_policy', value);

              if (value === 'default_value') {
                openDefaultOutputEditor();
              }
            }}
          />
        </div>
        {errorPolicy === 'default_value' ? (
          <Tooltip
            title={i18nText('agentFlow', 'auto.edit', {
              value1: i18nText('agentFlow', 'auto.default_output')
            })}
          >
            <Button
              aria-label={i18nText('agentFlow', 'auto.edit', {
                value1: i18nText('agentFlow', 'auto.default_output')
              })}
              icon={<EditOutlined />}
              shape="circle"
              size="small"
              type="text"
              onClick={openDefaultOutputEditor}
            />
          </Tooltip>
        ) : null}
      </div>
      <Modal
        cancelText={i18nText('agentFlow', 'auto.cancel')}
        okText={i18nText('agentFlow', 'auto.save')}
        open={defaultOutputOpen}
        title={i18nText('agentFlow', 'auto.default_output')}
        onCancel={() => setDefaultOutputOpen(false)}
        onOk={saveDefaultOutput}
      >
        <Input.TextArea
          aria-label={i18nText('agentFlow', 'auto.default_output')}
          autoSize={{ minRows: 5, maxRows: 10 }}
          placeholder={i18nText(
            'agentFlow',
            'auto.default_output_json_placeholder'
          )}
          value={defaultOutputText}
          onChange={(event) => {
            setDefaultOutputText(event.target.value);
            setDefaultOutputError(null);
          }}
        />
        {defaultOutputError ? (
          <Typography.Text type="danger">{defaultOutputError}</Typography.Text>
        ) : null}
      </Modal>
    </div>
  );
}

function renderRelationsView({ adapter, block }: SchemaViewRendererProps) {
  const node = getNode(adapter);
  const downstreamNodes =
    (adapter.getDerived('downstreamNodes') as Array<{
      id: string;
      alias: string;
    }>) ?? [];

  if (!node) {
    return null;
  }

  return (
    <div className="agent-flow-node-detail__section">
      <Typography.Title
        level={5}
        className="agent-flow-node-detail__section-title"
      >
        {block.title ?? i18nText("agentFlow", "auto.next_step")}
      </Typography.Title>
      <Typography.Text className="agent-flow-node-detail__section-subtitle">
        {i18nText("agentFlow", "auto.add_next_node_workflow")}</Typography.Text>
      <div
        className="agent-flow-node-detail__relation-list"
        style={{ marginTop: 12 }}
      >
        <div className="agent-flow-node-detail__relation-source">
          <HomeOutlined />
        </div>
        <div className="agent-flow-node-detail__relation-line" />
        <div className="agent-flow-node-detail__relation-nodes">
          {downstreamNodes.map((downstreamNode) => (
            <div
              key={downstreamNode.id}
              className="agent-flow-node-detail__relation-item"
            >
              <div className="agent-flow-node-detail__relation-item-icon">
                <HomeOutlined style={{ fontSize: 12 }} />
              </div>
              {downstreamNode.alias}
            </div>
          ))}
          <div
            className="agent-flow-node-detail__relation-add"
            onClick={() =>
              adapter.dispatch('openNodePicker', { nodeId: node.id })
            }
          >
            <PlusOutlined /> {i18nText("agentFlow", "auto.add_parallel_node")}</div>
        </div>
      </div>
    </div>
  );
}

function renderRuntimeSummaryView({ adapter }: SchemaViewRendererProps) {
  const lastRun = adapter.getDerived('lastRun') as
    | NodeLastRun
    | null
    | undefined;
  const emptyDescription =
    (adapter.getDerived('lastRunEmptyDescription') as string | null) ??
    i18nText("agentFlow", "auto.node_running_records_yet");

  return lastRun ? (
    <NodeRunSummaryCard lastRun={lastRun} />
  ) : (
    <NodeRunEmptyState description={emptyDescription} />
  );
}

function renderRuntimeIoView({ adapter }: SchemaViewRendererProps) {
  const lastRun = adapter.getDerived('lastRun') as
    | NodeLastRun
    | null
    | undefined;

  return lastRun ? (
    <NodeRunIOCard lastRun={lastRun} />
  ) : null;
}

function renderRuntimeMetadataView({ adapter }: SchemaViewRendererProps) {
  const lastRun = adapter.getDerived('lastRun') as
    | NodeLastRun
    | null
    | undefined;

  return lastRun ? (
    <NodeRunMetadataCard lastRun={lastRun} />
  ) : null;
}

export const agentFlowViewRenderers = {
  card_eyebrow: renderCardEyebrowView,
  card_model: renderCardModelView,
  card_description: renderCardDescriptionView,
  summary: renderSummaryView,
  output_contract: OutputContractView,
  policy_group: PolicyGroupView,
  relations: renderRelationsView,
  runtime_summary: renderRuntimeSummaryView,
  runtime_io: renderRuntimeIoView,
  runtime_metadata: renderRuntimeMetadataView
} satisfies Record<string, SchemaViewRenderer>;
