import type { ConsoleFrontstageInterfaceCapability } from '@1flowbase/api-client';
import {
  Alert,
  Button,
  Divider,
  Empty,
  Input,
  InputNumber,
  Select,
  Space,
  Spin,
  Tag,
  Typography,
  message
} from 'antd';
import type { ReactNode } from 'react';
import { useEffect, useState } from 'react';

import { i18nText } from '../../../../shared/i18n/text';
import { bindFrontstageInterfaceCapability } from '../../lib/jsx-studio/interface-binding';
import {
  createFrontstageJsxBindingSnippet,
  type FrontstageJsxEditorProjection
} from '../../lib/jsx-studio/editor-projection';
import { generateFrontstageInterfaceSource } from '../../lib/jsx-studio/openapi-codegen';
import type { FrontstageBlockInstance } from '../../lib/page-document';
import type { FrontstageBlockHeightMode } from '../../lib/page-document';

export type FrontstageJsxStudioSection =
  | 'code'
  | 'interfaces'
  | 'variables'
  | 'components'
  | 'configuration'
  | 'run';

export function JsxStudioResourcePanel({
  block,
  pageBlocks,
  interfaceCapabilities,
  interfaceCapabilitiesError,
  interfaceCapabilitiesLoading,
  onInsertCode,
  onSaveBlock,
  projection,
  runPanel,
  section
}: {
  block: FrontstageBlockInstance;
  pageBlocks: readonly FrontstageBlockInstance[];
  interfaceCapabilities: readonly ConsoleFrontstageInterfaceCapability[];
  interfaceCapabilitiesError: Error | null;
  interfaceCapabilitiesLoading: boolean;
  onInsertCode: (source: string) => void;
  onSaveBlock: (block: FrontstageBlockInstance) => Promise<boolean | void>;
  projection: FrontstageJsxEditorProjection;
  runPanel?: ReactNode;
  section: Exclude<FrontstageJsxStudioSection, 'code'>;
}) {
  if (section === 'interfaces') {
    return (
      <InterfaceConnectorPanel
        block={block}
        interfaceCapabilities={interfaceCapabilities}
        error={interfaceCapabilitiesError}
        loading={interfaceCapabilitiesLoading}
        projection={projection}
        onInsertCode={onInsertCode}
        onSaveBlock={onSaveBlock}
      />
    );
  }

  if (section === 'variables') {
    return (
      <VariablesPanel
        block={block}
        pageBlocks={pageBlocks}
        onInsertCode={onInsertCode}
        onSaveBlock={onSaveBlock}
      />
    );
  }

  if (section === 'components') {
    return (
      <ComponentsPanel
        components={projection.components}
        onInsertCode={onInsertCode}
      />
    );
  }

  if (section === 'configuration') {
    return <ConfigurationPanel block={block} onSaveBlock={onSaveBlock} />;
  }

  return runPanel ? (
    <div className="frontstage-jsx-studio__resource-scroll">{runPanel}</div>
  ) : (
    <Empty
      image={Empty.PRESENTED_IMAGE_SIMPLE}
      description={i18nText('frontstage', 'auto.no_run_preview')}
    />
  );
}

function InterfaceConnectorPanel({
  block,
  interfaceCapabilities,
  error,
  loading,
  onInsertCode,
  onSaveBlock,
  projection
}: {
  block: FrontstageBlockInstance;
  interfaceCapabilities: readonly ConsoleFrontstageInterfaceCapability[];
  error: Error | null;
  loading: boolean;
  onInsertCode: (source: string) => void;
  onSaveBlock: (block: FrontstageBlockInstance) => Promise<boolean | void>;
  projection: FrontstageJsxEditorProjection;
}) {
  const [pendingBindingId, setPendingBindingId] = useState<string | null>(null);
  const [selectedOperationId, setSelectedOperationId] = useState<string>();
  const bindings = block.interfaces ?? [];
  const operationsById = new Map(
    interfaceCapabilities.map((operation) => [operation.interface_id, operation])
  );
  const selectedOperation = selectedOperationId
    ? operationsById.get(selectedOperationId)
    : undefined;

  if (loading) {
    return <Spin />;
  }

  if (error) {
    return (
      <Alert
        type="error"
        showIcon
        message={i18nText('frontstage', 'auto.capability_catalog_load_failed')}
      />
    );
  }

  if (interfaceCapabilities.length === 0) {
    return (
      <Empty
        image={Empty.PRESENTED_IMAGE_SIMPLE}
        description={i18nText('frontstage', 'auto.no_data_capabilities')}
      />
    );
  }

  const bindCapability = async (
    operation: ConsoleFrontstageInterfaceCapability
  ) => {
    const alias = createUniqueBindingAlias(
      operation.interface_id,
      bindings.map((item) => item.alias)
    );
    setPendingBindingId(operation.interface_id);
    try {
      const saved = await onSaveBlock(
        bindFrontstageInterfaceCapability(block, alias, operation)
      );
      if (saved !== false) {
        onInsertCode(generateFrontstageInterfaceSource(operation, alias).source);
        setSelectedOperationId(undefined);
        void message.success(
          i18nText('frontstage', 'auto.interface_bound_and_inserted')
        );
      }
    } finally {
      setPendingBindingId(null);
    }
  };

  const removeBinding = async (bindingKey: string) => {
    setPendingBindingId(bindingKey);
    try {
      await onSaveBlock({
        ...block,
        interfaces: bindings.filter((binding) => binding.alias !== bindingKey)
      });
    } finally {
      setPendingBindingId(null);
    }
  };

  return (
    <div className="frontstage-jsx-studio__resource-scroll">
      <ResourceHeading
        title={i18nText('frontstage', 'auto.interface_connector')}
        description={i18nText(
          'frontstage',
          'auto.interface_connector_description'
        )}
      />
      {projection.bindings.length > 0 ? (
        <section className="frontstage-jsx-studio__resource-section">
          <Typography.Text strong>
            {i18nText('frontstage', 'auto.bound_interfaces')}
          </Typography.Text>
          {projection.bindings.map((binding) => (
            <div
              className="frontstage-jsx-studio__binding-row"
              key={binding.binding.alias}
            >
              <div className="frontstage-jsx-studio__binding-copy">
                <Typography.Text code>{binding.binding.alias}</Typography.Text>
                <Typography.Text type="secondary" ellipsis>
                  {binding.operation
                    ? `${binding.operation.method.toUpperCase()} ${binding.operation.path}`
                    : binding.binding.operation_id}
                </Typography.Text>
                {binding.status !== 'current' ? (
                  <Tag color="warning">{binding.status}</Tag>
                ) : null}
              </div>
              <Space size={4}>
                <Button
                  size="small"
                  onClick={() =>
                    onInsertCode(createFrontstageJsxBindingSnippet(binding))
                  }
                >
                  {i18nText('frontstage', 'auto.insert_code')}
                </Button>
                <Button
                  danger
                  size="small"
                  loading={pendingBindingId === binding.binding.alias}
                  onClick={() => void removeBinding(binding.binding.alias)}
                >
                  {i18nText('frontstage', 'auto.unbind')}
                </Button>
              </Space>
            </div>
          ))}
        </section>
      ) : null}

      <section className="frontstage-jsx-studio__resource-section">
        <Typography.Text strong>
          {i18nText('frontstage', 'auto.interfaces')}
        </Typography.Text>
        <Select
          allowClear
          showSearch
          aria-label={i18nText('frontstage', 'auto.interfaces')}
          placeholder={i18nText('frontstage', 'auto.select_interface')}
          value={selectedOperationId}
          getPopupContainer={(triggerNode) => {
            const resourceSection = triggerNode.closest(
              '.frontstage-jsx-studio__resource-section'
            );
            return resourceSection instanceof HTMLElement
              ? resourceSection
              : triggerNode;
          }}
          options={interfaceCapabilities.map((operation) => {
            const isBound = bindings.some(
              (binding) => binding.operation_id === operation.interface_id
            );
            return {
              value: operation.interface_id,
              label: `${operation.method.toUpperCase()} ${operation.path}`,
              disabled: isBound || !operation.bindable
            };
          })}
          filterOption={(input, option) => {
            const operation = option?.value
              ? operationsById.get(String(option.value))
              : undefined;
            if (!operation) return false;
            return [
              operation.name,
              operation.interface_id,
              operation.method,
              operation.path,
              operation.short_description
            ]
              .filter(Boolean)
              .join(' ')
              .toLocaleLowerCase()
              .includes(input.trim().toLocaleLowerCase());
          }}
          onChange={(value) => setSelectedOperationId(value)}
          style={{ width: '100%' }}
        />
        <Button
          type="primary"
          disabled={!selectedOperation}
          loading={pendingBindingId === selectedOperationId}
          onClick={() => {
            if (selectedOperation) void bindCapability(selectedOperation);
          }}
        >
          {i18nText('frontstage', 'auto.bind_and_insert')}
        </Button>
      </section>
    </div>
  );
}

function VariablesPanel({
  block,
  pageBlocks,
  onInsertCode,
  onSaveBlock
}: {
  block: FrontstageBlockInstance;
  pageBlocks: readonly FrontstageBlockInstance[];
  onInsertCode: (source: string) => void;
  onSaveBlock: (block: FrontstageBlockInstance) => Promise<boolean | void>;
}) {
  const [outputName, setOutputName] = useState('');
  const [outputType, setOutputType] = useState<
    'string' | 'number' | 'boolean' | 'object'
  >('string');
  const [inputName, setInputName] = useState('');
  const [sourceKey, setSourceKey] = useState<string>();
  const [scope, setScope] = useState<'tab' | 'page'>('tab');
  const [saving, setSaving] = useState(false);
  const ports = block.ports ?? { inputs: [], outputs: [] };
  const sources = pageBlocks.flatMap((candidate) =>
    candidate.id === block.id
      ? []
      : (candidate.ports?.outputs ?? []).map((output) => ({
          key: `${candidate.id}:${output.name}`,
          blockId: candidate.id,
          output
        }))
  );
  const savePorts = async (
    nextPorts: NonNullable<FrontstageBlockInstance['ports']>
  ) => {
    setSaving(true);
    try {
      await onSaveBlock({ ...block, ports: nextPorts });
    } finally {
      setSaving(false);
    }
  };
  const addOutput = () => {
    const name = normalizePortName(outputName);
    if (!name || ports.outputs.some((port) => port.name === name)) return;
    void savePorts({
      ...ports,
      outputs: [...ports.outputs, { name, schema: { type: outputType } }]
    });
    setOutputName('');
  };
  const connectInput = () => {
    const name = normalizePortName(inputName);
    const source = sources.find((item) => item.key === sourceKey);
    if (!name || !source || ports.inputs.some((port) => port.name === name))
      return;
    void savePorts({
      ...ports,
      inputs: [
        ...ports.inputs,
        {
          name,
          schema: { ...source.output.schema },
          source: {
            block_id: source.blockId,
            output: source.output.name,
            scope
          }
        }
      ]
    });
    setInputName('');
  };
  const variables = [
    'ctx.currentUser',
    'ctx.workspace',
    'ctx.application',
    'ctx.page',
    ...ports.inputs.map((port) => `ctx.inputs.${port.name}`),
    'ctx.params',
    'ctx.props',
    'ctx.state',
    'ctx.theme',
    'ctx.ui'
  ];

  return (
    <div className="frontstage-jsx-studio__resource-scroll">
      <ResourceHeading
        title={i18nText('frontstage', 'auto.variables')}
        description={i18nText('frontstage', 'auto.variables_description')}
      />
      {variables.map((variable) => (
        <div className="frontstage-jsx-studio__insert-row" key={variable}>
          <Typography.Text code>{variable}</Typography.Text>
          <Button size="small" onClick={() => onInsertCode(variable)}>
            {i18nText('frontstage', 'auto.insert_code')}
          </Button>
        </div>
      ))}
      <Divider />
      <Typography.Text strong>
        {i18nText('frontstage', 'auto.output_ports')}
      </Typography.Text>
      {ports.outputs.map((port) => (
        <div className="frontstage-jsx-studio__insert-row" key={port.name}>
          <Typography.Text code>{port.name}</Typography.Text>
          <Button
            danger
            size="small"
            disabled={saving}
            onClick={() =>
              void savePorts({
                ...ports,
                outputs: ports.outputs.filter((item) => item.name !== port.name)
              })
            }
          >
            {i18nText('frontstage', 'auto.delete')}
          </Button>
        </div>
      ))}
      <Space.Compact block>
        <Input
          aria-label={i18nText('frontstage', 'auto.output_ports')}
          value={outputName}
          onChange={(event) => setOutputName(event.target.value)}
        />
        <Select
          value={outputType}
          options={['string', 'number', 'boolean', 'object'].map((value) => ({
            value,
            label: value
          }))}
          onChange={setOutputType}
          style={{ width: 120 }}
        />
        <Button loading={saving} onClick={addOutput}>
          {i18nText('frontstage', 'auto.add_port')}
        </Button>
      </Space.Compact>
      <Divider />
      <Typography.Text strong>
        {i18nText('frontstage', 'auto.input_ports')}
      </Typography.Text>
      {ports.inputs.map((port) => (
        <div className="frontstage-jsx-studio__insert-row" key={port.name}>
          <Typography.Text code>{port.name}</Typography.Text>
          <Tag>{port.source?.scope ?? 'unbound'}</Tag>
          {port.source &&
          !sources.some(
            (source) =>
              source.blockId === port.source?.block_id &&
              source.output.name === port.source?.output
          ) ? (
            <Tag color="error">
              {i18nText('frontstage', 'auto.signal_source_missing')}
            </Tag>
          ) : null}
          <Button
            danger
            size="small"
            disabled={saving}
            onClick={() =>
              void savePorts({
                ...ports,
                inputs: ports.inputs.filter((item) => item.name !== port.name)
              })
            }
          >
            {i18nText('frontstage', 'auto.delete')}
          </Button>
        </div>
      ))}
      <Input
        aria-label={i18nText('frontstage', 'auto.input_ports')}
        value={inputName}
        onChange={(event) => setInputName(event.target.value)}
      />
      <Select
        value={sourceKey}
        options={sources.map((source) => ({
          value: source.key,
          label: `${source.blockId}.${source.output.name}`
        }))}
        onChange={setSourceKey}
        style={{ width: '100%' }}
      />
      <Select
        value={scope}
        options={[
          { value: 'tab', label: i18nText('frontstage', 'auto.tab_scope') },
          { value: 'page', label: i18nText('frontstage', 'auto.page_scope') }
        ]}
        onChange={setScope}
        style={{ width: '100%' }}
      />
      <Button
        type="primary"
        loading={saving}
        disabled={!sourceKey}
        onClick={connectInput}
      >
        {i18nText('frontstage', 'auto.connect')}
      </Button>
    </div>
  );
}

function ComponentsPanel({
  components,
  onInsertCode
}: {
  components: readonly string[];
  onInsertCode: (source: string) => void;
}) {
  return (
    <div className="frontstage-jsx-studio__resource-scroll">
      <ResourceHeading
        title={i18nText('frontstage', 'auto.components')}
        description={i18nText('frontstage', 'auto.components_description')}
      />
      {components.length === 0 ? (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={i18nText('frontstage', 'auto.no_available_components')}
        />
      ) : (
        components.map((component) => (
          <div className="frontstage-jsx-studio__insert-row" key={component}>
            <Typography.Text code>{component}</Typography.Text>
            <Button
              size="small"
              onClick={() => onInsertCode(`<${component}></${component}>`)}
            >
              {i18nText('frontstage', 'auto.insert_code')}
            </Button>
          </div>
        ))
      )}
    </div>
  );
}

function ConfigurationPanel({
  block,
  onSaveBlock
}: {
  block: FrontstageBlockInstance;
  onSaveBlock: (block: FrontstageBlockInstance) => Promise<boolean | void>;
}) {
  const [title, setTitle] = useState(readString(block.props.title));
  const [description, setDescription] = useState(
    readString(block.props.description)
  );
  const [heightMode, setHeightMode] = useState<FrontstageBlockHeightMode>(
    block.presentation.heightMode
  );
  const [fixedHeight, setFixedHeight] = useState<number>(
    block.presentation.height ?? 320
  );
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    setTitle(readString(block.props.title));
    setDescription(readString(block.props.description));
    setHeightMode(block.presentation.heightMode);
    setFixedHeight(block.presentation.height ?? 320);
  }, [
    block.id,
    block.presentation.height,
    block.presentation.heightMode,
    block.props.description,
    block.props.title
  ]);

  const saveConfiguration = async () => {
    const props = { ...block.props };
    assignOptionalString(props, 'title', title);
    assignOptionalString(props, 'description', description);
    setSaving(true);
    try {
      const saved = await onSaveBlock({
        ...block,
        props,
        presentation: {
          heightMode,
          height: heightMode === 'fixed' ? fixedHeight : null
        }
      });
      if (saved !== false) {
        void message.success(
          i18nText('frontstage', 'auto.block_configuration_saved')
        );
      }
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="frontstage-jsx-studio__resource-scroll">
      <ResourceHeading
        title={i18nText('frontstage', 'auto.configuration')}
        description={i18nText(
          'frontstage',
          'auto.structured_configuration_description'
        )}
      />
      <Space direction="vertical" size={12} style={{ width: '100%' }}>
        <label className="frontstage-jsx-studio__field">
          <span>{i18nText('frontstage', 'auto.title')}</span>
          <Input
            value={title}
            onChange={(event) => setTitle(event.target.value)}
          />
        </label>
        <label className="frontstage-jsx-studio__field">
          <span>{i18nText('frontstage', 'auto.description')}</span>
          <Input.TextArea
            autoSize={{ minRows: 3, maxRows: 6 }}
            value={description}
            onChange={(event) => setDescription(event.target.value)}
          />
        </label>
        <label className="frontstage-jsx-studio__field">
          <span>{i18nText('frontstage', 'auto.height_mode')}</span>
          <Select
            aria-label={i18nText('frontstage', 'auto.height_mode')}
            value={heightMode}
            options={[
              {
                value: 'auto',
                label: i18nText('frontstage', 'auto.auto_height')
              },
              {
                value: 'fixed',
                label: i18nText('frontstage', 'auto.fixed_height')
              }
            ]}
            onChange={(value) => setHeightMode(value)}
          />
        </label>
        {heightMode === 'fixed' ? (
          <label className="frontstage-jsx-studio__field">
            <span>{i18nText('frontstage', 'auto.fixed_height')}</span>
            <InputNumber
              aria-label={i18nText('frontstage', 'auto.fixed_height')}
              min={120}
              max={2400}
              step={20}
              value={fixedHeight}
              onChange={(value) => setFixedHeight(value ?? 320)}
              style={{ width: '100%' }}
            />
          </label>
        ) : null}
        <Divider style={{ margin: '4px 0' }} />
        <Typography.Text type="secondary">Block ID</Typography.Text>
        <Typography.Text code copyable>
          {block.id}
        </Typography.Text>
        <Typography.Text type="secondary">codeRef</Typography.Text>
        <Typography.Text code copyable>
          {block.codeRef}
        </Typography.Text>
        <Button
          type="primary"
          loading={saving}
          onClick={() => void saveConfiguration()}
        >
          {i18nText('frontstage', 'auto.save_configuration')}
        </Button>
      </Space>
    </div>
  );
}

function ResourceHeading({
  description,
  title
}: {
  description: string;
  title: string;
}) {
  return (
    <Space direction="vertical" size={2}>
      <Typography.Text strong>{title}</Typography.Text>
      <Typography.Text type="secondary">{description}</Typography.Text>
    </Space>
  );
}

function createUniqueBindingAlias(
  operationId: string,
  usedAliases: readonly string[]
): string {
  const base = toCamelCase(operationId) || 'boundInterface';
  const used = new Set(usedAliases);
  if (!used.has(base)) {
    return base;
  }

  let suffix = 2;
  while (used.has(`${base}${suffix}`)) {
    suffix += 1;
  }
  return `${base}${suffix}`;
}

function toCamelCase(value: string): string {
  const parts = value.split(/[^A-Za-z0-9$]+/).filter(Boolean);
  return parts
    .map((part, index) =>
      index === 0
        ? `${part.charAt(0).toLowerCase()}${part.slice(1)}`
        : `${part.charAt(0).toUpperCase()}${part.slice(1)}`
    )
    .join('');
}

function readString(value: unknown): string {
  return typeof value === 'string' ? value : '';
}

function normalizePortName(value: string): string | null {
  const name = value.trim();
  return /^[A-Za-z_$][A-Za-z0-9_$]*$/.test(name) ? name : null;
}

function assignOptionalString(
  target: Record<string, unknown>,
  key: string,
  value: string
) {
  const normalized = value.trim();
  if (normalized) {
    target[key] = normalized;
  } else {
    delete target[key];
  }
}
