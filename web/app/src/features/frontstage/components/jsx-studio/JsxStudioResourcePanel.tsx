import type {
  ConsoleFrontstageDataCapabilities,
  ConsoleFrontstageDataCapabilityDescriptor
} from '@1flowbase/api-client';
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
import { useEffect, useMemo, useState } from 'react';

import { i18nText } from '../../../../shared/i18n/text';
import {
  readFrontstageBlockDataBindings,
  writeFrontstageBlockDataBindings,
  type FrontstageBlockDataBinding
} from '../../lib/jsx-studio/block-data-binding';
import {
  createFrontstageJsxBindingSnippet,
  type FrontstageJsxEditorProjection
} from '../../lib/jsx-studio/editor-projection';
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
  capabilities,
  capabilitiesError,
  capabilitiesLoading,
  onInsertCode,
  onSaveBlock,
  projection,
  runPanel,
  section
}: {
  block: FrontstageBlockInstance;
  capabilities: ConsoleFrontstageDataCapabilities;
  capabilitiesError: Error | null;
  capabilitiesLoading: boolean;
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
        capabilities={capabilities}
        error={capabilitiesError}
        loading={capabilitiesLoading}
        projection={projection}
        onInsertCode={onInsertCode}
        onSaveBlock={onSaveBlock}
      />
    );
  }

  if (section === 'variables') {
    return <VariablesPanel onInsertCode={onInsertCode} />;
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
    return (
      <ConfigurationPanel block={block} onSaveBlock={onSaveBlock} />
    );
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
  capabilities,
  error,
  loading,
  onInsertCode,
  onSaveBlock,
  projection
}: {
  block: FrontstageBlockInstance;
  capabilities: ConsoleFrontstageDataCapabilities;
  error: Error | null;
  loading: boolean;
  onInsertCode: (source: string) => void;
  onSaveBlock: (block: FrontstageBlockInstance) => Promise<boolean | void>;
  projection: FrontstageJsxEditorProjection;
}) {
  const [selectedModel, setSelectedModel] = useState<string | undefined>();
  const [pendingBindingId, setPendingBindingId] = useState<string | null>(null);
  const bindings = useMemo(
    () => readFrontstageBlockDataBindings(block.props),
    [block.props]
  );
  const modelCodes = capabilities.models.map((model) => model.code);

  useEffect(() => {
    if (selectedModel && modelCodes.includes(selectedModel)) {
      return;
    }
    setSelectedModel(modelCodes[0]);
  }, [modelCodes, selectedModel]);

  if (loading) {
    return <Spin />;
  }

  if (error) {
    return (
      <Alert
        type="error"
        showIcon
        message={i18nText(
          'frontstage',
          'auto.capability_catalog_load_failed'
        )}
      />
    );
  }

  if (capabilities.models.length === 0) {
    return (
      <Empty
        image={Empty.PRESENTED_IMAGE_SIMPLE}
        description={i18nText('frontstage', 'auto.no_data_capabilities')}
      />
    );
  }

  const bindCapability = async (
    descriptor: ConsoleFrontstageDataCapabilityDescriptor,
    kind: 'query' | 'action'
  ) => {
    if (!selectedModel) {
      return;
    }

    const bindingKey = createUniqueBindingKey(
      selectedModel,
      descriptor.id,
      bindings
    );
    const nextBinding: FrontstageBlockDataBinding = {
      key: bindingKey,
      id: descriptor.id,
      kind,
      params: { model: selectedModel }
    };
    setPendingBindingId(descriptor.id);
    try {
      const saved = await onSaveBlock(
        writeFrontstageBlockDataBindings(block, [...bindings, nextBinding])
      );
      if (saved !== false) {
        void message.success(
          i18nText('frontstage', 'auto.interface_binding_saved')
        );
      }
    } finally {
      setPendingBindingId(null);
    }
  };

  const removeBinding = async (bindingKey: string) => {
    setPendingBindingId(bindingKey);
    try {
      await onSaveBlock(
        writeFrontstageBlockDataBindings(
          block,
          bindings.filter((binding) => binding.key !== bindingKey)
        )
      );
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
      <Select
        aria-label={i18nText('frontstage', 'auto.select_data_model')}
        value={selectedModel}
        options={capabilities.models.map((model) => ({
          label: model.code,
          value: model.code
        }))}
        style={{ width: '100%' }}
        onChange={setSelectedModel}
      />

      {projection.bindings.length > 0 ? (
        <section className="frontstage-jsx-studio__resource-section">
          <Typography.Text strong>
            {i18nText('frontstage', 'auto.bound_interfaces')}
          </Typography.Text>
          {projection.bindings.map((binding) => (
            <div
              className="frontstage-jsx-studio__binding-row"
              key={binding.key}
            >
              <div className="frontstage-jsx-studio__binding-copy">
                <Typography.Text code>{binding.key}</Typography.Text>
                <Typography.Text type="secondary" ellipsis>
                  {binding.id}
                </Typography.Text>
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
                  loading={pendingBindingId === binding.key}
                  onClick={() => void removeBinding(binding.key)}
                >
                  {i18nText('frontstage', 'auto.unbind')}
                </Button>
              </Space>
            </div>
          ))}
        </section>
      ) : null}

      <CapabilitySection
        title={i18nText('frontstage', 'auto.queries')}
        modelCode={selectedModel ?? ''}
        descriptors={capabilities.queries}
        bindings={bindings}
        pendingBindingId={pendingBindingId}
        kind="query"
        onBind={bindCapability}
      />
      <CapabilitySection
        title={i18nText('frontstage', 'auto.actions')}
        modelCode={selectedModel ?? ''}
        descriptors={capabilities.actions}
        bindings={bindings}
        pendingBindingId={pendingBindingId}
        kind="action"
        onBind={bindCapability}
      />
    </div>
  );
}

function CapabilitySection({
  bindings,
  descriptors,
  kind,
  modelCode,
  onBind,
  pendingBindingId,
  title
}: {
  bindings: readonly FrontstageBlockDataBinding[];
  descriptors: readonly ConsoleFrontstageDataCapabilityDescriptor[];
  kind: 'query' | 'action';
  modelCode: string;
  onBind: (
    descriptor: ConsoleFrontstageDataCapabilityDescriptor,
    kind: 'query' | 'action'
  ) => Promise<void>;
  pendingBindingId: string | null;
  title: string;
}) {
  return (
    <section className="frontstage-jsx-studio__resource-section">
      <Typography.Text strong>{title}</Typography.Text>
      {descriptors.map((descriptor) => {
        const isBound = bindings.some(
          (binding) =>
            binding.id === descriptor.id && binding.params.model === modelCode
        );
        const operationLabel = getCapabilityOperationLabel(descriptor.id);
        return (
          <div
            className="frontstage-jsx-studio__capability-row"
            key={descriptor.id}
          >
            <div className="frontstage-jsx-studio__binding-copy">
              <Space size={6}>
                <Typography.Text>{operationLabel}</Typography.Text>
                <Tag>{kind === 'query' ? 'Query' : 'Action'}</Tag>
              </Space>
              <Typography.Text type="secondary" ellipsis>
                {descriptor.id}
              </Typography.Text>
            </div>
            <Button
              size="small"
              disabled={isBound || !modelCode}
              loading={pendingBindingId === descriptor.id}
              aria-label={`${i18nText('frontstage', 'auto.bind')} ${modelCode} ${operationLabel}`}
              onClick={() => void onBind(descriptor, kind)}
            >
              {isBound
                ? i18nText('frontstage', 'auto.bound')
                : i18nText('frontstage', 'auto.bind')}
            </Button>
          </div>
        );
      })}
    </section>
  );
}

function VariablesPanel({
  onInsertCode
}: {
  onInsertCode: (source: string) => void;
}) {
  const variables = [
    'ctx.currentUser',
    'ctx.workspace',
    'ctx.application',
    'ctx.page',
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
        title={i18nText('frontstage', 'auto.structured_configuration')}
        description={i18nText(
          'frontstage',
          'auto.structured_configuration_description'
        )}
      />
      <Space direction="vertical" size={12} style={{ width: '100%' }}>
        <label className="frontstage-jsx-studio__field">
          <span>{i18nText('frontstage', 'auto.title')}</span>
          <Input value={title} onChange={(event) => setTitle(event.target.value)} />
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
        <Button type="primary" loading={saving} onClick={() => void saveConfiguration()}>
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

function getCapabilityOperationLabel(id: string): string {
  const labels: Record<string, string> = {
    'frontstage.data_model.record.list': i18nText(
      'frontstage',
      'auto.query_list'
    ),
    'frontstage.data_model.record.get': i18nText(
      'frontstage',
      'auto.query_detail'
    ),
    'frontstage.data_model.record.create': i18nText(
      'frontstage',
      'auto.create_record'
    ),
    'frontstage.data_model.record.update': i18nText(
      'frontstage',
      'auto.update_record'
    ),
    'frontstage.data_model.record.delete': i18nText(
      'frontstage',
      'auto.delete_record'
    )
  };
  return labels[id] ?? id;
}

function createUniqueBindingKey(
  modelCode: string,
  capabilityId: string,
  bindings: readonly FrontstageBlockDataBinding[]
): string {
  const operation = capabilityId.split('.').at(-1) ?? 'binding';
  const base = toCamelCase(`${modelCode}-${operation}`) || 'binding';
  const usedKeys = new Set(bindings.map((binding) => binding.key));
  if (!usedKeys.has(base)) {
    return base;
  }

  let suffix = 2;
  while (usedKeys.has(`${base}${suffix}`)) {
    suffix += 1;
  }
  return `${base}${suffix}`;
}

function toCamelCase(value: string): string {
  const parts = value.split(/[^A-Za-z0-9_$]+/).filter(Boolean);
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
