import type {
  SchemaBlock,
  CanvasNodeSchema
} from '../../../../shared/schema-ui/v1/contracts/canvas-node-schema';
import { PlusOutlined, QuestionCircleOutlined } from '@ant-design/icons';
import { SchemaRenderer } from '../../../../shared/schema-ui/v1/runtime/SchemaRenderer';
import { evaluateSchemaRule } from '../../../../shared/schema-ui/v1/runtime/rule-evaluator';
import type { SchemaAdapter } from '../../../../shared/schema-ui/v1/registry/create-renderer-registry';
import { useEffect, useRef } from 'react';
import { Button, Tag, Tooltip, Typography } from 'antd';

import { agentFlowRendererRegistry } from '../../schema/agent-flow-renderer-registry';
import type { AgentFlowIssue } from '../../lib/validate-document';
import { useAgentFlowEditorStore } from '../../store/editor/provider';
import {
  namedBindingEntriesFromValue,
  toNamedBinding
} from '../detail/fields/HttpRequestKeyValuesField';
import { useNodeSchemaRuntime } from './use-node-schema-runtime';
import { i18nText } from '../../../../shared/i18n/text';

function isSectionBlock(
  block: SchemaBlock
): block is Extract<SchemaBlock, { kind: 'section' }> {
  return block.kind === 'section';
}

function isFieldBlock(
  block: SchemaBlock
): block is Extract<SchemaBlock, { kind: 'field' }> {
  return block.kind === 'field';
}

function isInlineFieldRenderer(renderer: string) {
  return (
    renderer === 'text' || renderer === 'number' || renderer === 'selector'
  );
}

function hasEmbeddedLabel(renderer: string) {
  return (
    renderer === 'templated_text' ||
    renderer === 'code_source' ||
    renderer === 'output_contract_definition' ||
    renderer === 'start_input_fields' ||
    renderer === 'start_model_list' ||
    renderer === 'llm_mcp_instances' ||
    renderer === 'llm_tool_registrations'
  );
}

function isPolicyFieldRenderer(renderer: string) {
  return (
    renderer === 'llm_context_policy' ||
    renderer === 'llm_external_reasoning_policy' ||
    renderer === 'switch' ||
    renderer === 'http_request_curl_import'
  );
}

function getFieldLabelTag() {
  return null;
}

function getFieldHelp(path: string) {
  if (path === 'config.context_policy') {
    return i18nText('agentFlow', 'auto.llm_context_help');
  }
  if (path === 'config.protocol_context') {
    return i18nText('agentFlow', 'auto.llm_protocol_context_help');
  }
  return null;
}

function createHttpRequestKeyValueEntry() {
  return { name: '', value: { kind: 'templated_text' as const, value: '' } };
}

function getFieldLabelAction(
  block: Extract<SchemaBlock, { kind: 'field' }>,
  adapter: SchemaAdapter
) {
  if (block.renderer !== 'http_request_key_values') {
    return null;
  }

  const actionLabel =
    block.path === 'bindings.headers'
      ? i18nText('agentFlow', 'auto.add_request_header')
      : i18nText('agentFlow', 'auto.add_request_param');

  return (
    <Tooltip title={actionLabel}>
      <Button
        aria-label={actionLabel}
        className="agent-flow-editor__inspector-field-label-action"
        icon={<PlusOutlined />}
        size="small"
        type="text"
        onClick={() => {
          const entries = namedBindingEntriesFromValue(
            adapter.getValue(block.path)
          );
          adapter.setValue(
            block.path,
            toNamedBinding([...entries, createHttpRequestKeyValueEntry()])
          );
        }}
      />
    </Tooltip>
  );
}

function fieldOwnsCollectionHeader(block: SchemaBlock) {
  if (!isSectionBlock(block) || block.blocks.length !== 1) {
    return false;
  }

  const [field] = block.blocks;

  return (
    field !== undefined &&
    isFieldBlock(field) &&
    (field.renderer === 'start_input_fields' ||
      field.renderer === 'start_model_list' ||
      field.renderer === 'output_contract_definition')
  );
}

function shouldRenderSectionTitle(
  block: Extract<SchemaBlock, { kind: 'section' }>
) {
  return block.title !== 'Inputs' && !fieldOwnsCollectionHeader(block);
}

function resolveFocusableFieldKey(fieldKey: string) {
  if (
    fieldKey === 'config.model' ||
    fieldKey === 'config.provider_code' ||
    fieldKey === 'config.provider_instance_id'
  ) {
    return 'config.model_provider';
  }

  return fieldKey;
}

function getRootValues(adapter: SchemaAdapter) {
  return (
    (adapter.getDerived('rootValues') as Record<string, unknown> | null) ?? {}
  );
}

function getFieldIssues(adapter: SchemaAdapter) {
  return (
    (adapter.getDerived('fieldIssues') as Record<
      string,
      AgentFlowIssue[]
    > | null) ?? {}
  );
}

function shouldRenderFieldBlock(
  block: Extract<SchemaBlock, { kind: 'field' }>,
  adapter: SchemaAdapter,
  capabilities: readonly string[]
) {
  return evaluateSchemaRule(block.visibleWhen, {
    values: getRootValues(adapter),
    capabilities
  });
}

export function NodeInspector({
  schema,
  adapter
}: {
  schema?: CanvasNodeSchema;
  adapter?: SchemaAdapter;
} = {}) {
  const rootRef = useRef<HTMLElement | null>(null);
  const setSelection = useAgentFlowEditorStore((state) => state.setSelection);
  const focusFieldKey = useAgentFlowEditorStore(
    (state) => state.focusedFieldKey
  );
  const runtime = useNodeSchemaRuntime(!schema || !adapter);
  const activeSchema = schema ?? runtime.schema;
  const activeAdapter = adapter ?? runtime.adapter;
  const configBlocks = activeSchema?.detail.tabs.config.blocks ?? [];
  const fieldIssuesByKey = activeAdapter ? getFieldIssues(activeAdapter) : {};

  useEffect(() => {
    if (!focusFieldKey || !rootRef.current) {
      return;
    }

    const timer = window.setTimeout(() => {
      const resolvedFieldKey = resolveFocusableFieldKey(focusFieldKey);
      const focusTarget = rootRef.current?.querySelector<HTMLElement>(
        `[data-field-key="${resolvedFieldKey}"] [aria-label]`
      );
      focusTarget?.focus();
      setSelection({
        focusedFieldKey: null
      });
    }, 0);

    return () => window.clearTimeout(timer);
  }, [focusFieldKey, setSelection]);

  if (!activeSchema || !activeAdapter) {
    return null;
  }

  return (
    <section ref={rootRef} className="agent-flow-node-detail__inspector">
      {configBlocks.map((block, blockIndex) => {
        if (!isSectionBlock(block)) {
          return (
            <SchemaRenderer
              key={`config-block-${blockIndex}`}
              adapter={activeAdapter}
              blocks={[block]}
              registry={agentFlowRendererRegistry}
              capabilities={activeSchema.capabilities}
            />
          );
        }

        return (
          <div
            key={`section-${blockIndex}-${block.title}`}
            className="agent-flow-node-detail__section agent-flow-node-detail__inspector-section"
            data-section-key={block.title}
          >
            {shouldRenderSectionTitle(block) ? (
              <div className="agent-flow-node-detail__section-header">
                <Typography.Title
                  level={5}
                  className="agent-flow-node-detail__section-title"
                >
                  {block.title}
                </Typography.Title>
              </div>
            ) : null}
            <div className="agent-flow-editor__inspector-fields">
              {block.blocks.map((childBlock, index) => {
                if (isFieldBlock(childBlock)) {
                  const fieldIssues = fieldIssuesByKey[childBlock.path] ?? [];
                  const hasError = fieldIssues.some(
                    (issue) => issue.level === 'error'
                  );
                  const labelTag = getFieldLabelTag();
                  const labelHelp = getFieldHelp(childBlock.path);
                  const labelAction = getFieldLabelAction(
                    childBlock,
                    activeAdapter
                  );

                  if (
                    !shouldRenderFieldBlock(
                      childBlock,
                      activeAdapter,
                      activeSchema.capabilities
                    )
                  ) {
                    return null;
                  }

                  return (
                    <div
                      key={childBlock.path}
                      className={[
                        'agent-flow-editor__inspector-field',
                        isInlineFieldRenderer(childBlock.renderer)
                          ? 'agent-flow-editor__inspector-field--inline'
                          : null,
                        isPolicyFieldRenderer(childBlock.renderer)
                          ? 'agent-flow-editor__inspector-field--policy'
                          : null,
                        hasError
                          ? 'agent-flow-editor__inspector-field--error'
                          : null
                      ]
                        .filter(Boolean)
                        .join(' ')}
                      data-field-key={childBlock.path}
                      data-testid={`inspector-field-${childBlock.path}`}
                    >
                      {!hasEmbeddedLabel(childBlock.renderer) && (
                        <div
                          className="agent-flow-editor__inspector-field-label-header"
                        >
                          <Typography.Text
                            strong
                            className="agent-flow-editor__inspector-field-label"
                          >
                            {childBlock.label}
                            {labelTag ? (
                              <Tag
                                variant="filled"
                                className="agent-flow-editor__inspector-field-label-tag"
                              >
                                {labelTag}
                              </Tag>
                            ) : null}
                            {labelHelp ? (
                              <Tooltip title={labelHelp}>
                                <QuestionCircleOutlined
                                  aria-label={labelHelp}
                                  className="agent-flow-editor__inspector-field-help"
                                />
                              </Tooltip>
                            ) : null}
                          </Typography.Text>
                          {labelAction}
                        </div>
                      )}
                      <div className="agent-flow-editor__inspector-field-control">
                        <SchemaRenderer
                          adapter={activeAdapter}
                          blocks={[childBlock]}
                          registry={agentFlowRendererRegistry}
                          capabilities={activeSchema.capabilities}
                        />
                      </div>
                      {fieldIssues.length > 0 ? (
                        <div className="agent-flow-editor__inspector-field-issues">
                          {fieldIssues.map((issue) => (
                            <Typography.Text
                              key={issue.id}
                              type={
                                issue.level === 'error' ? 'danger' : 'warning'
                              }
                            >
                              {issue.message}
                            </Typography.Text>
                          ))}
                        </div>
                      ) : null}
                    </div>
                  );
                }

                return (
                  <SchemaRenderer
                    key={`${block.title}-${index}`}
                    adapter={activeAdapter}
                    blocks={[childBlock]}
                    registry={agentFlowRendererRegistry}
                    capabilities={activeSchema.capabilities}
                  />
                );
              })}
            </div>
          </div>
        );
      })}
    </section>
  );
}
