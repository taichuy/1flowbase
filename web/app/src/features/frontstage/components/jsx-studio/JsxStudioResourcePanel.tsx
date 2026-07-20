import type { ConsoleFrontstageCallableInterface } from '@1flowbase/api-client';
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
import { bindFrontstageCallableInterface } from '../../lib/jsx-studio/interface-binding';
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
  callableInterfaces,
  callableInterfacesError,
  callableInterfacesLoading,
  onInsertCode,
  onSaveBlock,
  projection,
  runPanel,
  section
}: {
  block: FrontstageBlockInstance;
  callableInterfaces: readonly ConsoleFrontstageCallableInterface[];
  callableInterfacesError: Error | null;
  callableInterfacesLoading: boolean;
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
        callableInterfaces={callableInterfaces}
        error={callableInterfacesError}
        loading={callableInterfacesLoading}
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
  callableInterfaces,
  error,
  loading,
  onInsertCode,
  onSaveBlock,
  projection
}: {
  block: FrontstageBlockInstance;
  callableInterfaces: readonly ConsoleFrontstageCallableInterface[];
  error: Error | null;
  loading: boolean;
  onInsertCode: (source: string) => void;
  onSaveBlock: (block: FrontstageBlockInstance) => Promise<boolean | void>;
  projection: FrontstageJsxEditorProjection;
}) {
  const [pendingBindingId, setPendingBindingId] = useState<string | null>(null);
  const bindings = block.interfaces ?? [];

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

  if (callableInterfaces.length === 0) {
    return (
      <Empty
        image={Empty.PRESENTED_IMAGE_SIMPLE}
        description={i18nText('frontstage', 'auto.no_data_capabilities')}
      />
    );
  }

  const bindCapability = async (
    operation: ConsoleFrontstageCallableInterface
  ) => {
    const alias = createUniqueBindingAlias(
      operation.operation_id,
      bindings.map((item) => item.alias)
    );
    setPendingBindingId(operation.operation_id);
    try {
      const saved = await onSaveBlock(
        bindFrontstageCallableInterface(block, alias, operation)
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
                  {binding.binding.operation_id}
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
        {callableInterfaces.map((operation) => {
          const isBound = bindings.some(
            (binding) => binding.operation_id === operation.operation_id
          );
          return (
            <div
              className="frontstage-jsx-studio__capability-row"
              key={operation.operation_id}
            >
              <div className="frontstage-jsx-studio__binding-copy">
                <Space size={6}>
                  <Typography.Text>{operation.name}</Typography.Text>
                  <Tag>{operation.method.toUpperCase()}</Tag>
                  <Tag>{operation.risk_level}</Tag>
                </Space>
                <Typography.Text type="secondary" ellipsis>
                  {operation.operation_id}
                </Typography.Text>
                {!operation.bindable && operation.disabled_reason ? (
                  <Typography.Text type="secondary">
                    {operation.disabled_reason}
                  </Typography.Text>
                ) : null}
              </div>
              <Button
                size="small"
                disabled={isBound || !operation.bindable}
                loading={pendingBindingId === operation.operation_id}
                onClick={() => void bindCapability(operation)}
              >
                {isBound
                  ? i18nText('frontstage', 'auto.bound')
                  : i18nText('frontstage', 'auto.bind')}
              </Button>
            </div>
          );
        })}
      </section>
    </div>
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
