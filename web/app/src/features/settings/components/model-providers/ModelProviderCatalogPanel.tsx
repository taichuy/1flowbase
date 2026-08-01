import { Button, Empty, Space, Table, Tag, Typography } from 'antd';

import { formatDateTime } from '../../../../shared/i18n/format';
import { ScrollableSurface } from '../../../../shared/ui/scrollable-surface/ScrollableSurface';
import type { SettingsPluginFamilyEntry } from '../../api/plugins';
import type { SettingsModelProviderCatalogEntry } from '../../api/model-providers';
import {
  formatPluginArtifactAvailabilityStatus,
  isPluginArtifactUnavailable
} from './plugin-installation-status';
import { ModelProviderOverviewSummary } from '../../pages/settings-page/model-providers/ModelProviderOverviewSummary';
import { i18nText } from '../../../../shared/i18n/text';

function getCatalogDescription(
  family: SettingsPluginFamilyEntry,
  currentCatalogEntry: SettingsModelProviderCatalogEntry | null | undefined
) {
  return (
    family.description?.trim() ||
    currentCatalogEntry?.description_key?.trim() ||
    i18nText('settings', 'auto.no_description_provided')
  );
}

function formatCheckedAt(value: string) {
  const timestamp = new Date(value);

  if (Number.isNaN(timestamp.getTime())) {
    return value;
  }

  return formatDateTime(timestamp);
}

export function ModelProviderCatalogPanel({
  overviewRows,
  entries,
  currentCatalogEntries,
  loading,
  canManage,
  deletingProviderCode,
  refreshingArtifactInstallationId,
  installingArtifactInstallationId,
  onCreate,
  onViewInstances,
  onRefreshCurrentNodeArtifact,
  onInstallCurrentNodeArtifact,
  onDelete
}: {
  overviewRows: { key: string; label: string; value: string }[];
  entries: SettingsPluginFamilyEntry[];
  currentCatalogEntries: Record<
    string,
    SettingsModelProviderCatalogEntry | null
  >;
  loading?: boolean;
  canManage: boolean;
  deletingProviderCode?: string | null;
  refreshingArtifactInstallationId?: string | null;
  installingArtifactInstallationId?: string | null;
  onCreate: (entry: SettingsPluginFamilyEntry) => void;
  onViewInstances: (entry: SettingsPluginFamilyEntry) => void;
  onRefreshCurrentNodeArtifact: (entry: SettingsPluginFamilyEntry) => void;
  onInstallCurrentNodeArtifact: (entry: SettingsPluginFamilyEntry) => void;
  onDelete: (entry: SettingsPluginFamilyEntry) => void;
}) {
  return (
    <ScrollableSurface className="model-provider-panel__catalog">
      <div className="model-provider-panel__section-head">
        <ModelProviderOverviewSummary rows={overviewRows} />
      </div>

      <Table<SettingsPluginFamilyEntry>
        className="model-provider-panel__catalog-table"
        rowKey="provider_code"
        size="small"
        loading={loading}
        pagination={false}
        dataSource={entries}
        scroll={{ x: 980 }}
        locale={{
          emptyText: (
            <Empty
              image={Empty.PRESENTED_IMAGE_SIMPLE}
              description={
                loading
                  ? i18nText('settings', 'auto.loading_supplier_catalog')
                  : i18nText('settings', 'auto.suppliers_available_yet')
              }
            />
          )
        }}
        columns={[
          ...(canManage
            ? [
                {
                  title: i18nText('settings', 'auto.operation'),
                  key: 'actions',
                  width: 190,
                  render: (_: unknown, entry: SettingsPluginFamilyEntry) => {
                    const localArtifact = entry.current_local_artifact;
                    const artifactUnavailable = isPluginArtifactUnavailable(
                      localArtifact.artifact_status
                    );

                    return (
                      <Space
                        size={4}
                        className="model-provider-panel__catalog-actions"
                      >
                        <Button
                          type="link"
                          onClick={() => onViewInstances(entry)}
                        >
                          {i18nText(
                            'settings',
                            'auto.model_provider_manage_action'
                          )}
                        </Button>
                        <Button
                          type="link"
                          disabled={artifactUnavailable}
                          onClick={() => onCreate(entry)}
                        >
                          {i18nText('settings', 'auto.new')}
                        </Button>
                        <Button
                          type="link"
                          loading={
                            refreshingArtifactInstallationId ===
                            entry.current_installation_id
                          }
                          onClick={() => onRefreshCurrentNodeArtifact(entry)}
                        >
                          {i18nText('settings', 'auto.refresh')}
                        </Button>
                        {artifactUnavailable ? (
                          <Button
                            type="link"
                            loading={
                              installingArtifactInstallationId ===
                              entry.current_installation_id
                            }
                            onClick={() => onInstallCurrentNodeArtifact(entry)}
                          >
                            {i18nText('settings', 'auto.repair')}
                          </Button>
                        ) : null}
                        <Button
                          danger
                          type="link"
                          loading={deletingProviderCode === entry.provider_code}
                          onClick={() => onDelete(entry)}
                        >
                          {i18nText('settings', 'auto.delete')}
                        </Button>
                      </Space>
                    );
                  }
                }
              ]
            : []),
          {
            title: i18nText('settings', 'auto.name'),
            key: 'provider',
            width: 180,
            render: (_, entry) => (
              <div className="model-provider-panel__catalog-name">
                <Typography.Text strong>{entry.display_name}</Typography.Text>
              </div>
            )
          },
          {
            title: i18nText('settings', 'auto.status'),
            key: 'status',
            width: 190,
            render: (_, entry) => {
              const artifactStatus = formatPluginArtifactAvailabilityStatus(
                entry.current_local_artifact.artifact_status
              );

              return (
                <Space
                  wrap
                  size={[6, 6]}
                  className="model-provider-panel__catalog-status"
                >
                  <Tag color={artifactStatus.color}>{artifactStatus.label}</Tag>
                  <Tag>{entry.model_discovery_mode}</Tag>
                  {entry.has_update ? (
                    <Tag color="gold">
                      {i18nText('settings', 'auto.updates_available')}
                    </Tag>
                  ) : null}
                </Space>
              );
            }
          },
          {
            title: i18nText('settings', 'auto.version'),
            key: 'version',
            width: 220,
            render: (_, entry) => {
              const localArtifactVersion =
                entry.current_local_artifact.local_version;
              const shouldShowLocalArtifactVersion =
                Boolean(localArtifactVersion) &&
                localArtifactVersion !== entry.current_version;
              return (
                <div className="model-provider-panel__catalog-version">
                  <Typography.Text strong>
                    {entry.current_version}
                  </Typography.Text>
                  {shouldShowLocalArtifactVersion ? (
                    <Typography.Text
                      type="secondary"
                      className="model-provider-panel__version-detail"
                    >
                      {i18nText('settings', 'auto.current_node_version', {
                        value1: localArtifactVersion
                      })}
                    </Typography.Text>
                  ) : null}
                  <Typography.Text
                    type="secondary"
                    className="model-provider-panel__version-detail"
                  >
                    {i18nText('settings', 'auto.checked_at', {
                      value1: formatCheckedAt(
                        entry.current_local_artifact.checked_at
                      )
                    })}
                  </Typography.Text>
                  {entry.current_local_artifact.last_error ? (
                    <Typography.Text
                      type="danger"
                      className="model-provider-panel__version-detail"
                    >
                      {i18nText('settings', 'auto.current_node_error', {
                        value1: entry.current_local_artifact.last_error
                      })}
                    </Typography.Text>
                  ) : null}
                </div>
              );
            }
          },
          {
            title: i18nText('settings', 'auto.description'),
            key: 'summary',
            width: 200,
            render: (_, entry) => {
              const currentCatalogEntry =
                currentCatalogEntries[entry.provider_code];
              const description = getCatalogDescription(
                entry,
                currentCatalogEntry
              );

              return (
                <div className="model-provider-panel__catalog-description">
                  <Typography.Paragraph
                    className="model-provider-panel__catalog-description-text"
                    ellipsis={{ rows: 2, tooltip: description }}
                  >
                    {description}
                  </Typography.Paragraph>
                </div>
              );
            }
          }
        ]}
      />
    </ScrollableSurface>
  );
}
