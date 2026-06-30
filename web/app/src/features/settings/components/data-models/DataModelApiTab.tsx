import { Alert, Descriptions, Empty, Flex, List, Tag, Typography } from 'antd';
import { useQuery } from '@tanstack/react-query';

import {
  fetchSettingsDataModelOpenApiDocument,
  settingsDataModelOpenApiQueryKey,
  type SettingsDataModel,
  type SettingsDataModelOpenApiDocument
} from '../../api/data-models';
import { i18nText } from '../../../../shared/i18n/text';

type OpenApiSchema = {
  $ref?: string;
  title?: unknown;
  required?: unknown;
  properties?: unknown;
};

type FieldValidationGroup = {
  key: string;
  title: string;
  fields: string[];
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

function schemaRequired(schema: OpenApiSchema | null) {
  if (!schema || !Array.isArray(schema.required)) {
    return new Set<string>();
  }

  return new Set(
    schema.required.filter(
      (fieldCode): fieldCode is string => typeof fieldCode === 'string'
    )
  );
}

function fieldLabel(fieldCode: string, schema: unknown) {
  if (isRecord(schema) && typeof schema.title === 'string') {
    return schema.title;
  }

  return fieldCode;
}

function getFieldValidationGroups(
  document: SettingsDataModelOpenApiDocument | undefined,
  model: SettingsDataModel
): FieldValidationGroup[] {
  if (!document) {
    return [];
  }

  const schemas = getSchemas(document);
  const baseName = toSchemaBaseName(model);
  const recordSchema = findSchemaByName(
    schemas,
    [`${baseName}Record`],
    model,
    'Record'
  );
  const createInputSchema = findSchemaByName(
    schemas,
    [`${baseName}RecordCreateInput`, `${baseName}CreateInput`],
    model,
    'RecordCreateInput'
  );

  const recordProperties = schemaProperties(recordSchema);
  const createProperties = schemaProperties(createInputSchema);
  const createRequired = schemaRequired(createInputSchema);
  const requiredFields = Object.entries(createProperties)
    .filter(([fieldCode]) => createRequired.has(fieldCode))
    .map(([fieldCode, schema]) => fieldLabel(fieldCode, schema));
  const optionalFields = Object.entries(createProperties)
    .filter(([fieldCode]) => !createRequired.has(fieldCode))
    .map(([fieldCode, schema]) => fieldLabel(fieldCode, schema));
  const readOnlyFields = Object.entries(recordProperties)
    .filter(([fieldCode]) => !(fieldCode in createProperties))
    .map(([fieldCode, schema]) => fieldLabel(fieldCode, schema));

  return [
    {
      key: 'required',
      title: i18nText('settings', 'auto.create_request_required_fields'),
      fields: requiredFields
    },
    {
      key: 'optional',
      title: i18nText('settings', 'auto.optional_input_fields'),
      fields: optionalFields
    },
    {
      key: 'read_only',
      title: i18nText('settings', 'auto.system_generated_read_only_fields'),
      fields: readOnlyFields
    }
  ];
}

export function DataModelApiTab({ model }: { model: SettingsDataModel }) {
  const openApiQuery = useQuery({
    queryKey: settingsDataModelOpenApiQueryKey(model.id),
    queryFn: () => fetchSettingsDataModelOpenApiDocument(model.id),
    enabled: Boolean(model.id)
  });
  const fieldValidationGroups = getFieldValidationGroups(
    openApiQuery.data,
    model
  );
  const exposureStatus = model.api_exposure_status;
  const exposureLabel =
    exposureStatus === 'api_exposed_ready'
      ? i18nText('settings', 'auto.api_exposed_ready')
      : exposureStatus;
  const exposureColor =
    exposureStatus === 'api_exposed_ready' ? 'green' : 'default';

  return (
    <Flex vertical gap={16}>
      <Descriptions
        size="small"
        column={1}
        items={[
          {
            key: 'status',
            label: i18nText('settings', 'auto.api_exposure_status'),
            children: (
              <Tag
                color={exposureColor}
                data-testid="data-model-api-exposure-status"
              >
                {exposureLabel}
              </Tag>
            )
          },
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
      <Flex vertical gap={8}>
        <Typography.Text strong>
          {i18nText('settings', 'auto.api_field_validation_groups')}
        </Typography.Text>
        <Typography.Text type="secondary">
          {i18nText('settings', 'auto.api_field_validation_groups_help')}
        </Typography.Text>
        {openApiQuery.isError ? (
          <Alert
            type="warning"
            showIcon
            message={i18nText(
              'settings',
              'auto.openapi_contract_load_failed'
            )}
          />
        ) : null}
        {!openApiQuery.isLoading &&
        !openApiQuery.isError &&
        fieldValidationGroups.every((group) => group.fields.length === 0) ? (
          <Empty
            image={Empty.PRESENTED_IMAGE_SIMPLE}
            description={i18nText(
              'settings',
              'auto.openapi_contract_fields_empty'
            )}
          />
        ) : (
          <List
            data-testid="data-model-api-field-validation-groups"
            loading={openApiQuery.isLoading}
            dataSource={fieldValidationGroups}
            renderItem={(group) => (
              <List.Item>
                <Flex vertical gap={8}>
                  <Typography.Text strong>{group.title}</Typography.Text>
                  <Flex wrap gap={8}>
                    {group.fields.length === 0 ? (
                      <Typography.Text type="secondary">
                        {i18nText('settings', 'auto.no_fields')}
                      </Typography.Text>
                    ) : (
                      group.fields.map((field) => (
                        <Tag key={field}>{field}</Tag>
                      ))
                    )}
                  </Flex>
                </Flex>
              </List.Item>
            )}
          />
        )}
      </Flex>
    </Flex>
  );
}
