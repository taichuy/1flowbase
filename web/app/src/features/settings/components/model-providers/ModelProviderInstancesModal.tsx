import { useState } from 'react';

import {
  Alert,
  Button,
  Empty,
  Space,
  Switch,
  Table,
  Tabs,
  Tag,
  Typography
} from 'antd';
import type { ColumnsType } from 'antd/es/table';

import { FixedHeightModal } from '../../../../shared/ui/fixed-height-modal/FixedHeightModal';
import type {
  SettingsModelProviderCatalogEntry,
  SettingsModelProviderInstance,
  SettingsModelProviderMainInstance,
  SettingsModelProviderOptions
} from '../../api/model-providers';
import { ModelProviderInstancesTable } from './ModelProviderInstancesTable';
import { ModelProviderRoutingPolicyModal } from './ModelProviderRoutingPolicyModal';
import { i18nText } from '../../../../shared/i18n/text';

type ModelGroup =
  SettingsModelProviderOptions['providers'][number]['model_groups'][number];
type ModelGroupTarget = ModelGroup['targets'][number];
type DistributionRule =
  SettingsModelProviderMainInstance['model_routing_policies'][number]['distribution_rule'];

const SOURCE_INSTANCE_TAG_COLORS = [
  'blue',
  'cyan',
  'green',
  'geekblue',
  'purple',
  'magenta',
  'volcano',
  'gold'
] as const;

function sourceInstanceTagColor(sourceInstanceDisplayName: string) {
  const colorIndex =
    Array.from(sourceInstanceDisplayName).reduce(
      (sum, character) => sum + character.charCodeAt(0),
      0
    ) % SOURCE_INSTANCE_TAG_COLORS.length;

  return SOURCE_INSTANCE_TAG_COLORS[colorIndex];
}

function distributionRuleOptions() {
  return [
    {
      value: 'none',
      label: i18nText('settings', 'auto.distribution_rule_none')
    },
    {
      value: 'round_robin',
      label: i18nText('settings', 'auto.distribution_rule_round_robin')
    },
    {
      value: 'retry_round_robin',
      label: i18nText('settings', 'auto.distribution_rule_retry_round_robin')
    }
  ] satisfies Array<{ value: DistributionRule; label: string }>;
}

export function ModelProviderInstancesModal({
  open,
  catalogEntry,
  providerDisplayName,
  mainInstance,
  modelGroups,
  instances,
  updatingMainInstance,
  updatingInstanceId,
  refreshingCandidates,
  refreshing,
  deleting,
  canManage,
  versionSwitchNotice,
  onClose,
  onEdit,
  onRefreshCandidates,
  onRefreshModels,
  onDelete,
  onToggleAutoIncludeNewInstances,
  onChangeDistributionRule,
  onSaveRoutingPolicy,
  onToggleIncludedInMain
}: {
  open: boolean;
  catalogEntry: SettingsModelProviderCatalogEntry | null;
  providerDisplayName: string | null;
  mainInstance: SettingsModelProviderMainInstance | null;
  modelGroups: ModelGroup[];
  instances: SettingsModelProviderInstance[];
  updatingMainInstance: boolean;
  updatingInstanceId?: string | null;
  refreshingCandidates: boolean;
  refreshing: boolean;
  deleting: boolean;
  canManage: boolean;
  versionSwitchNotice: {
    targetVersion: string | null;
    migratedInstanceCount: number | null;
  } | null;
  onClose: () => void;
  onEdit: (instance: SettingsModelProviderInstance) => void;
  onRefreshCandidates: (instance: SettingsModelProviderInstance) => void;
  onRefreshModels: (instance: SettingsModelProviderInstance) => void;
  onDelete: (instance: SettingsModelProviderInstance) => void;
  onToggleAutoIncludeNewInstances: (checked: boolean) => void;
  onChangeDistributionRule: (
    modelId: string,
    distributionRule: DistributionRule
  ) => void;
  onSaveRoutingPolicy: (
    modelId: string,
    distributionRule: DistributionRule,
    providerInstanceIds: string[],
    onSuccess: () => void
  ) => void;
  onToggleIncludedInMain: (
    instance: SettingsModelProviderInstance,
    checked: boolean
  ) => void;
}) {
  const [editingGroup, setEditingGroup] = useState<ModelGroup | null>(null);
  const includedCount = instances.filter(
    (instance) => instance.included_in_main
  ).length;
  const aggregatedModelCount = modelGroups.length;
  const displayName = catalogEntry?.display_name ?? providerDisplayName;
  const title = displayName
    ? i18nText('settings', 'auto.instance', { value1: displayName })
    : i18nText('settings', 'auto.supplier_instance');
  const mainInstanceLabel = i18nText('settings', 'auto.master_instance');
  const renderModelGroupTargetTag = (target: ModelGroupTarget) => {
    const sourceInstance = instances.find(
      (instance) => instance.id === target.source_instance_id
    );
    const tagColor = sourceInstanceTagColor(
      target.source_instance_display_name
    );

    if (!canManage || !sourceInstance) {
      return (
        <Tag key={target.source_instance_id} bordered={false} color={tagColor}>
          {target.source_instance_display_name}
        </Tag>
      );
    }

    return (
      <Tag
        key={target.source_instance_id}
        bordered={false}
        className="model-provider-panel__main-instance-target-tag-action"
        color={tagColor}
        role="button"
        tabIndex={0}
        onClick={() => onEdit(sourceInstance)}
        onKeyDown={(event) => {
          if (event.key === 'Enter' || event.key === ' ') {
            event.preventDefault();
            onEdit(sourceInstance);
          }
        }}
      >
        {target.source_instance_display_name}
      </Tag>
    );
  };
  const modelGroupColumns: ColumnsType<ModelGroup> = [
    {
      key: 'model_id',
      dataIndex: 'model_id',
      title: i18nText('settings', 'auto.model_id_alt'),
      width: '42%',
      render: (modelId: string) => <Typography.Text>{modelId}</Typography.Text>
    },
    {
      key: 'group',
      title: i18nText('settings', 'auto.group'),
      render: (_, group) => (
        <div className="model-provider-panel__main-instance-targets">
          {group.targets.length === 0 ? (
            <Typography.Text type="secondary">
              {i18nText('settings', 'auto.unsummarized_model')}
            </Typography.Text>
          ) : (
            group.targets.map(renderModelGroupTargetTag)
          )}
        </div>
      )
    },
    {
      key: 'distribution_rule',
      title: i18nText('settings', 'auto.distribution_rule'),
      width: 160,
      render: (_, group) => (
        <select
          aria-label={i18nText('settings', 'auto.distribution_rule')}
          className="model-provider-panel__distribution-rule-select"
          disabled={!canManage || updatingMainInstance || !mainInstance}
          value={group.distribution_rule}
          onChange={(event) =>
            onChangeDistributionRule(
              group.model_id,
              event.currentTarget.value as DistributionRule
            )
          }
        >
          <option value="" disabled hidden />
          {distributionRuleOptions().map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
      )
    },
    {
      key: 'operation',
      title: i18nText('settings', 'auto.operation'),
      width: 88,
      render: (_, group) => (
        <Button
          type="link"
          size="small"
          disabled={!canManage || updatingMainInstance || !mainInstance}
          onClick={(event) => {
            event.stopPropagation();
            setEditingGroup(group);
          }}
        >
          {i18nText('settings', 'auto.edit')}
        </Button>
      )
    }
  ];

  const editingPolicy = mainInstance?.model_routing_policies.find(
    (policy) => policy.model_id === editingGroup?.model_id
  );

  return (
    <>
      <FixedHeightModal
        open={open}
        width={960}
        height="min(860px, calc(100vh - 96px))"
        title={title}
        onCancel={onClose}
        footer={null}
        destroyOnHidden
        scrollBodyClassName="model-provider-panel__instances-modal"
      >
        <>
          {versionSwitchNotice ? (
            <Alert
              type="warning"
              showIcon
              message={i18nText('settings', 'auto.text')}
              description={
                versionSwitchNotice.targetVersion
                  ? i18nText(
                      'settings',
                      'auto.target_version_instances_migrated',
                      {
                        value1: versionSwitchNotice.targetVersion,
                        value2: versionSwitchNotice.migratedInstanceCount ?? 0
                      }
                    )
                  : undefined
              }
            />
          ) : null}

          <Tabs
            className="model-provider-panel__instances-tabs"
            defaultActiveKey="models"
            items={[
              {
                key: 'models',
                label: i18nText('settings', 'auto.model_management'),
                children: (
                  <section className="model-provider-panel__main-instance-card">
                    <div className="model-provider-panel__main-instance-head">
                      <div className="model-provider-panel__main-instance-title-row">
                        <Typography.Text strong>
                          {mainInstanceLabel}
                        </Typography.Text>
                        <div className="model-provider-panel__main-instance-summary">
                          <Tag bordered={false} color="blue">
                            {i18nText('settings', 'auto.aggregate_view')}
                          </Tag>
                          <Typography.Text type="secondary">
                            {i18nText('settings', 'auto.example')}
                            {includedCount}
                          </Typography.Text>
                          <Typography.Text type="secondary">
                            {i18nText('settings', 'auto.model')}
                            {aggregatedModelCount}
                          </Typography.Text>
                        </div>
                      </div>
                      <Space
                        direction="horizontal"
                        size={8}
                        className="model-provider-panel__main-instance-toggle"
                      >
                        <Typography.Text type="secondary">
                          {i18nText(
                            'settings',
                            'auto.new_instances_automatically_injected_main_instance'
                          )}
                        </Typography.Text>
                        <Switch
                          aria-label={i18nText(
                            'settings',
                            'auto.new_instances_automatically_injected_main_instance'
                          )}
                          checked={
                            mainInstance?.auto_include_new_instances ?? false
                          }
                          disabled={!canManage || updatingMainInstance}
                          onChange={onToggleAutoIncludeNewInstances}
                        />
                      </Space>
                    </div>

                    {modelGroups.length === 0 ? (
                      <Empty
                        image={Empty.PRESENTED_IMAGE_SIMPLE}
                        description={i18nText('settings', 'auto.text_alt')}
                      />
                    ) : (
                      <Table<ModelGroup>
                        className="model-provider-panel__main-instance-table"
                        columns={modelGroupColumns}
                        dataSource={modelGroups}
                        pagination={false}
                        rowKey="model_id"
                        size="small"
                        components={{
                          table: (props) => (
                            <table {...props} aria-label={mainInstanceLabel} />
                          )
                        }}
                      />
                    )}
                  </section>
                )
              },
              {
                key: 'sources',
                label: i18nText('settings', 'auto.source_management'),
                children: (
                  <ModelProviderInstancesTable
                    instances={instances}
                    canManage={canManage}
                    loading={false}
                    updatingInstanceId={updatingInstanceId}
                    onToggleIncludedInMain={onToggleIncludedInMain}
                    onEdit={onEdit}
                    onRefreshCandidates={(instance) => {
                      if (!refreshingCandidates) {
                        onRefreshCandidates(instance);
                      }
                    }}
                    onRefreshModels={(instance) => {
                      if (!refreshing) {
                        onRefreshModels(instance);
                      }
                    }}
                    onDelete={(instance) => {
                      if (!deleting) {
                        onDelete(instance);
                      }
                    }}
                  />
                )
              }
            ]}
          />
        </>
      </FixedHeightModal>
      {editingGroup ? (
        <ModelProviderRoutingPolicyModal
          open
          modelId={editingGroup.model_id}
          policy={editingPolicy}
          targets={editingGroup.targets}
          saving={updatingMainInstance}
          onCancel={() => setEditingGroup(null)}
          onSave={({ distribution_rule, provider_instance_ids }) => {
            onSaveRoutingPolicy(
              editingGroup.model_id,
              distribution_rule,
              provider_instance_ids,
              () => setEditingGroup(null)
            );
          }}
        />
      ) : null}
    </>
  );
}
