import { Alert, Empty, Modal, Space, Switch, Tag, Typography } from 'antd';

import type {
  SettingsModelProviderCatalogEntry,
  SettingsModelProviderInstance,
  SettingsModelProviderMainInstance,
  SettingsModelProviderOptions
} from '../../api/model-providers';
import { ModelProviderTagList } from './ModelProviderTagList';
import { ModelProviderInstancesTable } from './ModelProviderInstancesTable';
import { i18nText } from '../../../../shared/i18n/text';

type ModelGroup =
  SettingsModelProviderOptions['providers'][number]['model_groups'][number];

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
  onToggleIncludedInMain: (
    instance: SettingsModelProviderInstance,
    checked: boolean
  ) => void;
}) {
  const includedCount = instances.filter(
    (instance) => instance.included_in_main
  ).length;
  const aggregatedModelCount = modelGroups.length;
  const displayName = catalogEntry?.display_name ?? providerDisplayName;
  const title = displayName
    ? i18nText('settings', 'auto.instance', { value1: displayName })
    : i18nText('settings', 'auto.supplier_instance');

  return (
    <Modal
      open={open}
      width={960}
      title={title}
      aria-label={title}
      onCancel={onClose}
      footer={null}
      destroyOnHidden
    >
      <div className="model-provider-panel__instances-modal">
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

        <section className="model-provider-panel__main-instance-card">
          <div className="model-provider-panel__main-instance-head">
            <div className="model-provider-panel__main-instance-title-row">
              <Typography.Text strong>
                {i18nText('settings', 'auto.master_instance')}
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
                checked={mainInstance?.auto_include_new_instances ?? false}
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
            <div className="model-provider-panel__main-instance-groups">
              {modelGroups.map((group) => (
                <section
                  key={group.model_id}
                  className="model-provider-panel__main-instance-group"
                >
                  <Typography.Text strong>
                    {group.model.display_name || group.model_id}
                  </Typography.Text>
                  <Typography.Text type="secondary">
                    {group.targets
                      .map((target) => target.source_instance_display_name)
                      .join(' -> ')}
                  </Typography.Text>
                  <ModelProviderTagList
                    modelIds={group.targets.map(
                      (target) => target.source_instance_display_name
                    )}
                    emptyText={i18nText('settings', 'auto.unsummarized_model')}
                  />
                </section>
              ))}
            </div>
          )}
        </section>

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
      </div>
    </Modal>
  );
}
