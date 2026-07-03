import {
  Alert,
  Descriptions,
  Empty,
  Flex,
  Switch,
  Table,
  Tag,
  Typography
} from 'antd';
import type { ColumnsType } from 'antd/es/table';
import { useQuery } from '@tanstack/react-query';

import {
  fetchSettingsDataModelOpenApiDocument,
  settingsDataModelOpenApiQueryKey,
  type SettingsDataModel,
  type SettingsDataModelField,
  type SettingsDataModelOpenApiDocument
} from '../../api/data-models';
import { i18nText } from '../../../../shared/i18n/text';

type OpenApiSchema = {
  $ref?: string;
  title?: unknown;
  properties?: unknown;
};

type ApiFieldSets = {
  createInput: Set<string>;
  updateInput: Set<string>;
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function getSchemas(document: SettingsDataModelOpenApiDocument) {
  const components = document.components;
  if (!isRecord(components) || !isRecord(components.schemas)) {
    return {};
  }

  return components.schemas;
}

function normalizeSchemaName(value: string) {
  return value.replace(/[^a-zA-Z0-9]/g, '').toLowerCase();
}

function toSchemaBaseName(model: SettingsDataModel) {
  const source = model.title || model.code;

  return source
    .split(/[^a-zA-Z0-9]+/)
    .filter(Boolean)
    .map((segment) => segment.charAt(0).toUpperCase() + segment.slice(1))
    .join('');
}

function resolveSchemaRef(
  schema: unknown,
  schemas: Record<string, unknown>
): OpenApiSchema | null {
  if (!isRecord(schema)) {
    return null;
  }

  if (typeof schema.$ref === 'string') {
    const refName = schema.$ref.match(/^#\/components\/schemas\/(.+)$/)?.[1];

    return refName ? resolveSchemaRef(schemas[refName], schemas) : null;
  }

  return schema as OpenApiSchema;
}

function findSchemaByName(
  schemas: Record<string, unknown>,
  candidates: string[],
  model: SettingsDataModel,
  suffix: string
) {
  for (const candidate of candidates) {
    const schema = resolveSchemaRef(schemas[candidate], schemas);
    if (schema) {
      return schema;
    }
  }

  const normalizedModelNames = [model.code, model.title]
    .filter(Boolean)
    .map(normalizeSchemaName);
  const matchedEntry = Object.entries(schemas).find(([schemaName]) => {
    const normalizedSchemaName = normalizeSchemaName(schemaName);

    return (
      normalizedSchemaName.endsWith(normalizeSchemaName(suffix)) &&
      normalizedModelNames.some((modelName) =>
        normalizedSchemaName.startsWith(modelName)
      )
    );
  });

  return matchedEntry ? resolveSchemaRef(matchedEntry[1], schemas) : null;
}

function schemaProperties(schema: OpenApiSchema | null) {
  if (!schema || !isRecord(schema.properties)) {
    return {};
  }

  return schema.properties;
}

function getApiFieldSets(
  document: SettingsDataModelOpenApiDocument | undefined,
  model: SettingsDataModel
): ApiFieldSets {
  if (!document) {
    return {
      createInput: new Set(),
      updateInput: new Set()
    };
  }

  const schemas = getSchemas(document);
  const baseName = toSchemaBaseName(model);
  const createInputSchema = findSchemaByName(
    schemas,
    [`${baseName}RecordCreateInput`, `${baseName}CreateInput`],
    model,
    'RecordCreateInput'
  );
  const updateInputSchema = findSchemaByName(
    schemas,
    [`${baseName}RecordUpdateInput`, `${baseName}UpdateInput`],
    model,
    'RecordUpdateInput'
  );

  return {
    createInput: new Set(Object.keys(schemaProperties(createInputSchema))),
    updateInput: new Set(Object.keys(schemaProperties(updateInputSchema)))
  };
}

function fieldKindTag(kind: string) {
  return (
    <Tag style={{ borderRadius: 4, margin: 0 }} color="blue">
      {kind}
    </Tag>
  );
}

function yesNoTag(value: boolean) {
  return value ? (
    <Tag color="success" style={{ borderRadius: 4, margin: 0 }}>
      {i18nText('settings', 'auto.yes')}
    </Tag>
  ) : (
    <Tag style={{ borderRadius: 4, margin: 0 }}>
      {i18nText('settings', 'auto.no')}
    </Tag>
  );
}

function contractUnavailableTag() {
  return (
    <Tag style={{ borderRadius: 4, margin: 0 }}>
      {i18nText('settings', 'auto.api_contract_unavailable')}
    </Tag>
  );
}

function fieldAttribute(field: SettingsDataModelField) {
  if (field.is_system || !field.is_writable) {
    return i18nText('settings', 'auto.system_generated_read_only');
  }

  return i18nText('settings', 'auto.writable');
}

export function DataModelApiTab({
  model,
  canManage,
  saving,
  onUpdateApiRequired
}: {
  model: SettingsDataModel;
  canManage: boolean;
  saving: boolean;
  onUpdateApiRequired: (
    field: SettingsDataModelField,
    apiRequired: boolean
  ) => void;
}) {
  const openApiQuery = useQuery({
    queryKey: settingsDataModelOpenApiQueryKey(model.id),
    queryFn: () => fetchSettingsDataModelOpenApiDocument(model.id),
    enabled: Boolean(model.id) && model.status === 'published'
  });
  const apiFieldSets = getApiFieldSets(openApiQuery.data, model);
  const apiContractAvailable =
    Boolean(openApiQuery.data) && !openApiQuery.isError;
  const columns: ColumnsType<SettingsDataModelField> = [
    {
      title: i18nText('settings', 'auto.field_title'),
      dataIndex: 'title',
      key: 'title',
      render: (value: string) => (
        <Typography.Text strong>{value}</Typography.Text>
      )
    },
    {
      title: 'Code',
      dataIndex: 'code',
      key: 'code',
      render: (value: string) => (
        <code className="data-model-panel__code-badge">{value}</code>
      )
    },
    {
      title: i18nText('settings', 'auto.field_type'),
      dataIndex: 'field_kind',
      key: 'field_kind',
      render: fieldKindTag
    },
    {
      title: i18nText('settings', 'auto.create_input'),
      key: 'create_input',
      render: (_, field) =>
        apiContractAvailable
          ? yesNoTag(apiFieldSets.createInput.has(field.code))
          : contractUnavailableTag()
    },
    {
      title: i18nText('settings', 'auto.create_required'),
      key: 'api_required',
      render: (_, field) => {
        if (!apiContractAvailable) {
          return contractUnavailableTag();
        }

        const editable =
          field.is_writable &&
          !field.is_system &&
          apiFieldSets.createInput.has(field.code);

        if (!editable) {
          return (
            <Tag style={{ borderRadius: 4, margin: 0 }}>
              {i18nText('settings', 'auto.not_configurable')}
            </Tag>
          );
        }

        return (
          <Switch
            size="small"
            aria-label={i18nText(
              'settings',
              'auto.api_create_required_toggle',
              {
                value1: field.title || field.code
              }
            )}
            checked={field.api_required}
            disabled={!canManage || saving || openApiQuery.isLoading}
            onChange={(checked) => onUpdateApiRequired(field, checked)}
          />
        );
      }
    },
    {
      title: i18nText('settings', 'auto.update_input'),
      key: 'update_input',
      render: (_, field) =>
        apiContractAvailable
          ? yesNoTag(apiFieldSets.updateInput.has(field.code))
          : contractUnavailableTag()
    },
    {
      title: i18nText('settings', 'auto.field_attribute'),
      key: 'attribute',
      render: (_, field) => (
        <Tag style={{ borderRadius: 4, margin: 0 }}>
          {fieldAttribute(field)}
        </Tag>
      )
    }
  ];

  return (
    <Flex vertical gap={16}>
      <Descriptions
        size="small"
        column={1}
        items={[
          {
            key: 'runtime',
            label: i18nText('settings', 'auto.runtime'),
            children: model.runtime_availability
          },
          {
            key: 'namespace',
            label: i18nText('settings', 'auto.acl_namespace'),
            children: model.acl_namespace
          }
        ]}
      />
      <Flex vertical gap={8} data-testid="data-model-api-fields-table">
        <Typography.Text strong>
          {i18nText('settings', 'auto.api_exposed_fields')}
        </Typography.Text>
        {openApiQuery.isError ? (
          <Alert
            type="warning"
            showIcon
            message={i18nText('settings', 'auto.openapi_contract_load_failed')}
          />
        ) : null}
        {model.fields.length === 0 ? (
          <Empty
            image={Empty.PRESENTED_IMAGE_SIMPLE}
            description={i18nText('settings', 'auto.no_fields')}
          />
        ) : (
          <Table
            rowKey="id"
            size="small"
            loading={openApiQuery.isLoading}
            pagination={false}
            columns={columns}
            dataSource={model.fields}
          />
        )}
      </Flex>
    </Flex>
  );
}
