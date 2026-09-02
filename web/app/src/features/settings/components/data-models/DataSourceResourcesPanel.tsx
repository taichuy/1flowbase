import { useEffect, useState } from 'react';

import {
  Alert,
  Button,
  Empty,
  Flex,
  Popover,
  Select,
  Space,
  Table,
  Tag,
  Typography
} from 'antd';
import type { ColumnsType } from 'antd/es/table';
import { useQuery } from '@tanstack/react-query';
import CheckCircleOutlined from '@ant-design/icons/es/icons/CheckCircleOutlined';
import EyeOutlined from '@ant-design/icons/es/icons/EyeOutlined';
import LinkOutlined from '@ant-design/icons/es/icons/LinkOutlined';
import ReloadOutlined from '@ant-design/icons/es/icons/ReloadOutlined';

import type {
  SettingsCompatibleDataModelTemplate,
  SettingsRuntimeExtensionDataSource,
  SettingsDataSourceRemoteResource
} from '../../api/data-models';
import {
  fetchSettingsCompatibleDataModelTemplates,
  settingsCompatibleDataModelTemplatesQueryKey
} from '../../api/data-models';
import { i18nText } from '../../../../shared/i18n/text';
import {
  dataModelTemplateIdentity,
  dataModelTemplatePresentation
} from '../../lib/data-model-template-presentation';

const NO_COMPATIBLE_TEMPLATES: SettingsCompatibleDataModelTemplate[] = [];

export function DataSourceResourcesPanel({
  dataSource,
  resources,
  loading,
  validating,
  discovering,
  previewingResourceKey,
  mappingResourceKey,
  canManage,
  onValidate,
  onDiscover,
  onPreview,
  onMap
}: {
  dataSource: SettingsRuntimeExtensionDataSource;
  resources: SettingsDataSourceRemoteResource[];
  loading: boolean;
  validating: boolean;
  discovering: boolean;
  previewingResourceKey: string | null;
  mappingResourceKey: string | null;
  canManage: boolean;
  onValidate: () => void;
  onDiscover: () => void;
  onPreview: (resource: SettingsDataSourceRemoteResource) => void;
  onMap: (
    resource: SettingsDataSourceRemoteResource,
    template: SettingsCompatibleDataModelTemplate
  ) => void;
}) {
  const [mappingTarget, setMappingTarget] =
    useState<SettingsDataSourceRemoteResource | null>(null);
  const [selectedTemplate, setSelectedTemplate] =
    useState<SettingsCompatibleDataModelTemplate | null>(null);
  const compatibleTemplatesQuery = useQuery({
    queryKey: settingsCompatibleDataModelTemplatesQueryKey(
      dataSource.id,
      mappingTarget?.resource_key
    ),
    queryFn: () =>
      fetchSettingsCompatibleDataModelTemplates(
        dataSource.id,
        mappingTarget?.resource_key
      ),
    enabled: Boolean(mappingTarget)
  });
  const templatesReady =
    compatibleTemplatesQuery.isSuccess && !compatibleTemplatesQuery.isFetching;
  const compatibleTemplates = templatesReady
    ? (compatibleTemplatesQuery.data ?? [])
    : NO_COMPATIBLE_TEMPLATES;

  useEffect(() => {
    setSelectedTemplate(null);
  }, [mappingTarget]);

  useEffect(() => {
    if (!templatesReady) {
      setSelectedTemplate(null);
      return;
    }
    setSelectedTemplate((current) =>
      current &&
      compatibleTemplates.some(
        (template) =>
          dataModelTemplateIdentity(template) ===
          dataModelTemplateIdentity(current)
      )
        ? current
        : null
    );
  }, [compatibleTemplates, templatesReady]);

  const selectedTemplateIsCompatible =
    templatesReady &&
    selectedTemplate !== null &&
    compatibleTemplates.some(
      (template) =>
        dataModelTemplateIdentity(template) ===
        dataModelTemplateIdentity(selectedTemplate)
    );

  const closeTemplateSelection = () => {
    setMappingTarget(null);
    setSelectedTemplate(null);
  };

  const templateSelection = (
    <Space orientation="vertical" size={12} style={{ width: 320 }}>
      <Typography.Text strong>
        {i18nText('settings', 'auto.select_data_model_template')}
      </Typography.Text>
      {compatibleTemplatesQuery.error instanceof Error ? (
        <Alert
          type="error"
          showIcon
          title={compatibleTemplatesQuery.error.message}
        />
      ) : null}
      {templatesReady &&
      !compatibleTemplatesQuery.error &&
      compatibleTemplates.length === 0 ? (
        <Alert
          type="warning"
          showIcon
          title={i18nText('settings', 'auto.no_compatible_data_model_template')}
        />
      ) : null}
      <Select
        aria-label={i18nText('settings', 'auto.data_model_template')}
        aria-busy={compatibleTemplatesQuery.isFetching}
        loading={compatibleTemplatesQuery.isFetching}
        disabled={
          !templatesReady ||
          Boolean(compatibleTemplatesQuery.error) ||
          compatibleTemplates.length === 0
        }
        value={
          selectedTemplate
            ? dataModelTemplateIdentity(selectedTemplate)
            : undefined
        }
        placeholder={i18nText('settings', 'auto.select_data_model_template')}
        options={compatibleTemplates.map((template) => ({
          value: dataModelTemplateIdentity(template),
          label: dataModelTemplatePresentation(template).title
        }))}
        optionRender={(option) => <span>{option.data.label}</span>}
        onChange={(identity) =>
          setSelectedTemplate(
            compatibleTemplates.find(
              (template) => dataModelTemplateIdentity(template) === identity
            ) ?? null
          )
        }
      />
      <Flex justify="flex-end" gap={8}>
        <Button onClick={closeTemplateSelection}>
          {i18nText('settings', 'auto.cancel')}
        </Button>
        <Button
          type="primary"
          disabled={!selectedTemplateIsCompatible || !mappingTarget}
          onClick={() => {
            if (
              mappingTarget &&
              selectedTemplate &&
              selectedTemplateIsCompatible
            ) {
              onMap(mappingTarget, selectedTemplate);
              closeTemplateSelection();
            }
          }}
        >
          {i18nText('settings', 'auto.map_to_data_model')}
        </Button>
      </Flex>
    </Space>
  );

  const columns: ColumnsType<SettingsDataSourceRemoteResource> = [
    {
      title: i18nText('settings', 'auto.remote_resources'),
      key: 'display_name',
      render: (_, resource) => (
        <Space orientation="vertical" size={2}>
          <Typography.Text strong>{resource.display_name}</Typography.Text>
          <Typography.Text type="secondary">
            <code className="data-model-panel__code-badge">
              {resource.resource_key}
            </code>
          </Typography.Text>
        </Space>
      )
    },
    {
      title: i18nText('settings', 'auto.kind'),
      dataIndex: 'resource_kind',
      key: 'resource_kind',
      width: 140,
      render: (kind: string) => <Tag>{kind}</Tag>
    },
    {
      title: i18nText('settings', 'auto.operation'),
      key: 'actions',
      width: 220,
      render: (_, resource) => (
        <Space>
          <Button
            type="link"
            icon={<EyeOutlined aria-hidden="true" />}
            disabled={
              !canManage || !dataSource.capabilities.can_preview_resources
            }
            loading={previewingResourceKey === resource.resource_key}
            onClick={() => onPreview(resource)}
          >
            {i18nText('settings', 'auto.preview')}
          </Button>
          <Popover
            trigger="click"
            placement="bottomRight"
            open={mappingTarget?.resource_key === resource.resource_key}
            content={templateSelection}
            onOpenChange={(open) => {
              if (!open) {
                closeTemplateSelection();
              }
            }}
          >
            <Button
              type="link"
              icon={<LinkOutlined aria-hidden="true" />}
              disabled={
                !canManage || !dataSource.capabilities.can_map_resources
              }
              loading={mappingResourceKey === resource.resource_key}
              onClick={() => setMappingTarget(resource)}
            >
              {i18nText('settings', 'auto.map_to_data_model')}
            </Button>
          </Popover>
        </Space>
      )
    }
  ];

  return (
    <section aria-labelledby="data-source-remote-resources-title">
      <Flex align="center" justify="space-between" gap={12} wrap="wrap">
        <Typography.Title
          id="data-source-remote-resources-title"
          level={5}
          style={{ margin: 0 }}
        >
          {i18nText('settings', 'auto.remote_resources')}
        </Typography.Title>
        {dataSource.capabilities.can_discover_resources ? (
          <Button
            icon={<ReloadOutlined aria-hidden="true" />}
            disabled={!canManage}
            loading={discovering}
            onClick={onDiscover}
          >
            {i18nText('settings', 'auto.discover_resources')}
          </Button>
        ) : dataSource.capabilities.can_validate ? (
          <Button
            type="primary"
            icon={<CheckCircleOutlined aria-hidden="true" />}
            disabled={!canManage}
            loading={validating}
            onClick={onValidate}
          >
            {i18nText('settings', 'auto.validate_data_source')}
          </Button>
        ) : null}
      </Flex>
      {dataSource.capabilities.can_discover_resources ? (
        <Table
          rowKey="resource_key"
          size="small"
          loading={loading}
          columns={columns}
          dataSource={resources}
          pagination={false}
          locale={{
            emptyText: (
              <Empty
                image={Empty.PRESENTED_IMAGE_SIMPLE}
                description={i18nText(
                  'settings',
                  'auto.discover_resources_empty'
                )}
              />
            )
          }}
        />
      ) : null}
    </section>
  );
}
