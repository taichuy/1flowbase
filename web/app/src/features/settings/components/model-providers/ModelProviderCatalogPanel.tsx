import {
  Badge,
  Button,
  Empty,
  Select,
  Space,
  Table,
  Tag,
  Tooltip,
  Typography
} from 'antd';

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

function compareVersions(left: string, right: string) {
  return right.localeCompare(left, undefined, {
    numeric: true,
    sensitivity: 'base'
  });
}

function sortInstalledVersions(
  versions: SettingsPluginFamilyEntry['installed_versions']
) {
  return Array.from(versions).sort((left, right) =>
    compareVersions(left.plugin_version, right.plugin_version)
  );
}

export function ModelProviderCatalogPanel({
  overviewRows,
  entries,
  currentCatalogEntries,
  loading,
  canManage,
  switchingProviderCode,
  upgradingProviderCode,
  onCreate,
  onViewInstances,
  onUpgradeLatest,
  onSwitchVersion
}: {
  overviewRows: { key: string; label: string; value: string }[];
  entries: SettingsPluginFamilyEntry[];
  currentCatalogEntries: Record<
    string,
    SettingsModelProviderCatalogEntry | null
  >;
  loading?: boolean;
  canManage: boolean;
  switchingProviderCode?: string | null;
  upgradingProviderCode?: string | null;
  onCreate: (entry: SettingsPluginFamilyEntry) => void;
  onViewInstances: (entry: SettingsPluginFamilyEntry) => void;
  onUpgradeLatest: (entry: SettingsPluginFamilyEntry) => void;
  onSwitchVersion: (
    entry: SettingsPluginFamilyEntry,
    installationId: string
  ) => void;
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
                  width: 220,
                  render: (_: unknown, entry: SettingsPluginFamilyEntry) => {
                    const localArtifact = entry.current_local_artifact;
                    const artifactUnavailable = isPluginArtifactUnavailable(
                      localArtifact.artifact_status
                    );
                    const upgrading =
                      upgradingProviderCode === entry.provider_code;

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
                        <Tooltip
                          title={
                            entry.has_update
                              ? undefined
                              : i18nText(
                                  'settings',
                                  'auto.currently_latest_version'
                                )
                          }
                        >
                          <span>
                            <Badge
                              dot={entry.has_update && !upgrading}
                              color="gold"
                              title={
                                entry.has_update
                                  ? i18nText(
                                      'settings',
                                      'auto.updates_available'
                                    )
                                  : undefined
                              }
                            >
                              <Button
                                type="link"
                                loading={upgrading}
                                disabled={!entry.has_update}
                                onClick={() => onUpgradeLatest(entry)}
                              >
                                {i18nText('settings', 'auto.update')}
                              </Button>
                            </Badge>
                          </span>
                        </Tooltip>
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
              const versionOptions = sortInstalledVersions(
                entry.installed_versions
              ).map((version) => ({
                value: version.installation_id,
                label: version.plugin_version
              }));

              return (
                <div className="model-provider-panel__catalog-version">
                  {canManage ? (
                    <Space
                      size={8}
                      wrap
                      className="model-provider-panel__version-inline"
                    >
                      <Select
                        size="small"
                        value={entry.current_installation_id}
                        className="model-provider-panel__version-select"
                        classNames={{
                          popup: {
                            root: 'model-provider-panel__version-dropdown'
                          }
                        }}
                        aria-label={i18nText(
                          'settings',
                          'auto.switch_version',
                          { value1: entry.display_name }
                        )}
                        loading={switchingProviderCode === entry.provider_code}
                        options={versionOptions}
                        onChange={(installationId) => {
                          if (
                            installationId === entry.current_installation_id
                          ) {
                            return;
                          }

                          onSwitchVersion(entry, installationId);
                        }}
                      />
                    </Space>
                  ) : (
                    <Typography.Text strong>
                      {entry.current_version}
                    </Typography.Text>
                  )}
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
