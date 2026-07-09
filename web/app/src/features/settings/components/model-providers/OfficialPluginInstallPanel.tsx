import { useMemo, useState } from 'react';

import { QuestionCircleOutlined } from '@ant-design/icons';
import {
  Alert,
  Button,
  Empty,
  Modal,
  Select,
  Tag,
  Tooltip,
  Typography
} from 'antd';

import { ScrollableSurface } from '../../../../shared/ui/scrollable-surface/ScrollableSurface';
import type {
  SettingsPluginCompatibilityOverride,
  SettingsOfficialPluginCatalogEntry,
  SettingsPluginFamilyEntry
} from '../../api/plugins';
import { i18nText } from '../../../../shared/i18n/text';

type InstallState = 'idle' | 'installing' | 'success' | 'failed';
const BELOW_MINIMUM_HOST_VERSION = 'below_minimum_host_version';

function isBelowMinimumHostVersion(entry: SettingsOfficialPluginCatalogEntry) {
  return entry.compatibility_status === BELOW_MINIMUM_HOST_VERSION;
}

function getInstallButtonLabel(
  entry: SettingsOfficialPluginCatalogEntry,
  installState: InstallState,
  activePluginId: string | null
) {
  if (activePluginId === entry.plugin_id && installState === 'installing') {
    return i18nText('settings', 'auto.installing');
  }

  if (
    entry.install_status === 'assigned' ||
    (activePluginId === entry.plugin_id && installState === 'success')
  ) {
    return i18nText('settings', 'auto.installed_workspace');
  }

  if (activePluginId === entry.plugin_id && installState === 'failed') {
    return i18nText('settings', 'auto.retry_installation');
  }

  if (isBelowMinimumHostVersion(entry)) {
    return i18nText('settings', 'auto.still_install');
  }

  return i18nText('settings', 'auto.install_workspace');
}

function compareOfficialVersion(left: string, right: string) {
  return left.localeCompare(right, undefined, {
    numeric: true,
    sensitivity: 'base'
  });
}

function pickPreferredOfficialEntry(
  current: SettingsOfficialPluginCatalogEntry,
  candidate: SettingsOfficialPluginCatalogEntry,
  family: SettingsPluginFamilyEntry | undefined
) {
  if (family?.latest_version) {
    const currentMatchesFamilyLatest =
      current.latest_version === family.latest_version;
    const candidateMatchesFamilyLatest =
      candidate.latest_version === family.latest_version;

    if (currentMatchesFamilyLatest !== candidateMatchesFamilyLatest) {
      return candidateMatchesFamilyLatest ? candidate : current;
    }
  }

  const versionComparison = compareOfficialVersion(
    candidate.latest_version,
    current.latest_version
  );
  if (versionComparison !== 0) {
    return versionComparison > 0 ? candidate : current;
  }

  const statusScore = {
    assigned: 2,
    installed: 1,
    not_installed: 0
  } as const;
  const currentStatusScore = statusScore[current.install_status];
  const candidateStatusScore = statusScore[candidate.install_status];

  if (currentStatusScore !== candidateStatusScore) {
    return candidateStatusScore > currentStatusScore ? candidate : current;
  }

  return compareOfficialVersion(candidate.plugin_id, current.plugin_id) > 0
    ? candidate
    : current;
}

const OFFICIAL_PLUGIN_RELEASES_URL =
  'https://github.com/taichuy/1flowbase-official-plugins/releases';
const DEFAULT_PROVIDER_ICON_SRC = '/icon.svg';
const INSTALL_CONFIRM_MODAL_WIDTH = 640;

function openExternal(url: string) {
  window.open(url, '_blank', 'noopener,noreferrer');
}

function getOfficialPluginIconSrc(entry: SettingsOfficialPluginCatalogEntry) {
  return entry.icon?.trim() || DEFAULT_PROVIDER_ICON_SRC;
}

function getTagColor(tag: string) {
  switch (tag) {
    case 'latest':
      return 'gold';
    case 'active':
      return 'green';
    case 'installed':
      return 'blue';
    case 'installing':
      return 'processing';
    case 'failed':
      return 'red';
    case BELOW_MINIMUM_HOST_VERSION:
      return 'orange';
    case 'hybrid':
      return 'purple';
    case 'dynamic':
      return 'cyan';
    case 'static':
      return 'default';
    default:
      return 'default';
  }
}

function OfficialPluginTagLabel({ tag }: { tag: string }) {
  if (tag === 'latest') {
    return (
      <span className="model-provider-panel__tag-label">
        latest
        <Tooltip
          title={i18nText(
            'settings',
            'auto.indicates_version_already_latest_official_version'
          )}
        >
          <QuestionCircleOutlined className="model-provider-panel__tag-help" />
        </Tooltip>
      </span>
    );
  }

  if (tag === BELOW_MINIMUM_HOST_VERSION) {
    return i18nText('settings', 'auto.host_version_risk');
  }

  if (tag === 'hybrid' || tag === 'dynamic' || tag === 'static') {
    return (
      <span className="model-provider-panel__tag-label">
        {tag}
        <Tooltip
          title={
            tag === 'hybrid'
              ? i18nText(
                  'settings',
                  'auto.discovery_mode_preset_model_list_plus_dynamically_pull_model_list'
                )
              : tag === 'dynamic'
                ? i18nText(
                    'settings',
                    'auto.discovery_mode_dynamically_pull_list_models_runtime'
                  )
                : i18nText(
                    'settings',
                    'auto.discovery_mode_use_preset_model_list_plug'
                  )
          }
        >
          <QuestionCircleOutlined className="model-provider-panel__tag-help" />
        </Tooltip>
      </span>
    );
  }

  return tag;
}

function getStatusTags(
  entry: SettingsOfficialPluginCatalogEntry,
  family: SettingsPluginFamilyEntry | undefined,
  installState: InstallState,
  activePluginId: string | null
) {
  const tags: string[] = [];

  if (family) {
    tags.push(family.current_version);
    tags.push(
      family.has_update && family.latest_version
        ? family.latest_version
        : 'latest'
    );
    if (isBelowMinimumHostVersion(entry)) {
      tags.push(BELOW_MINIMUM_HOST_VERSION);
    }
    tags.push(entry.model_discovery_mode);
    return tags;
  }

  tags.push(entry.latest_version);

  if (activePluginId === entry.plugin_id && installState === 'installing') {
    tags.push('installing');
  } else if (entry.install_status === 'assigned') {
    tags.push('active');
  } else if (entry.install_status === 'installed') {
    tags.push('installed');
  } else if (activePluginId === entry.plugin_id && installState === 'failed') {
    tags.push('failed');
  } else {
    tags.push('latest');
  }

  if (isBelowMinimumHostVersion(entry)) {
    tags.push(BELOW_MINIMUM_HOST_VERSION);
  }

  tags.push(entry.model_discovery_mode);
  return tags;
}

function OfficialPluginInstallConfirmContent({
  entry,
  family,
  belowMinimumHostVersion
}: {
  entry: SettingsOfficialPluginCatalogEntry;
  family: SettingsPluginFamilyEntry | undefined;
  belowMinimumHostVersion: boolean;
}) {
  return (
    <div className="model-provider-panel__install-confirm">
      <div className="model-provider-panel__install-confirm-card">
        <Typography.Title level={5}>{entry.display_name}</Typography.Title>
        <Typography.Paragraph type="secondary">
          {family
            ? i18nText(
                'settings',
                'auto.workspace_s_upgraded_latest_official_version_completion_all_instances_supplier',
                {
                  value1: entry.display_name,
                  value2: entry.latest_version
                }
              )
            : i18nText(
                'settings',
                'auto.latest_official_version_about_installed_completion_automatically_enabled_workspace',
                { value1: entry.latest_version }
              )}
        </Typography.Paragraph>
        <div className="model-provider-panel__catalog-item-meta">
          <span>
            {i18nText('settings', 'auto.agreement')}
            {entry.protocol}
          </span>
          <span>
            {i18nText('settings', 'auto.discovery_mode')}
            {entry.model_discovery_mode}
          </span>
        </div>
        {belowMinimumHostVersion ? (
          <Alert
            type="warning"
            showIcon
            message={i18nText(
              'settings',
              'auto.host_version_below_minimum_warning'
            )}
            description={
              <div className="model-provider-panel__install-warning-detail">
                <Typography.Text>
                  {i18nText('settings', 'auto.current_host_version_value', {
                    value1: entry.current_host_version
                  })}
                </Typography.Text>
                <Typography.Text>
                  {i18nText('settings', 'auto.minimum_host_version_value', {
                    value1: entry.minimum_host_version
                  })}
                </Typography.Text>
                <Typography.Text>
                  {i18nText('settings', 'auto.plugin_version_value', {
                    value1: entry.latest_version
                  })}
                </Typography.Text>
                <Typography.Text>
                  {i18nText('settings', 'auto.possible_risk_value', {
                    value1: i18nText(
                      'settings',
                      'auto.host_version_below_minimum_risk'
                    )
                  })}
                </Typography.Text>
                <Typography.Text>
                  {i18nText(
                    'settings',
                    'auto.upgrade_one_flowbase_before_continuing'
                  )}
                </Typography.Text>
              </div>
            }
          />
        ) : null}
      </div>
    </div>
  );
}

function OfficialPluginCard({
  entry,
  family,
  canManage,
  installState,
  activePluginId,
  upgradingProviderCode,
  modal,
  onInstall,
  onUpgradeLatest
}: {
  entry: SettingsOfficialPluginCatalogEntry;
  family: SettingsPluginFamilyEntry | undefined;
  canManage: boolean;
  installState: InstallState;
  activePluginId: string | null;
  upgradingProviderCode: string | null;
  modal: ReturnType<typeof Modal.useModal>[0];
  onInstall: (
    entry: SettingsOfficialPluginCatalogEntry,
    compatibilityOverride?: SettingsPluginCompatibilityOverride
  ) => void;
  onUpgradeLatest: (
    entry: SettingsOfficialPluginCatalogEntry,
    compatibilityOverride?: SettingsPluginCompatibilityOverride
  ) => void;
}) {
  const installing =
    activePluginId === entry.plugin_id && installState === 'installing';
  const installed =
    entry.install_status === 'assigned' ||
    (activePluginId === entry.plugin_id && installState === 'success');
  const upgrading = upgradingProviderCode === entry.provider_code;
  const belowMinimumHostVersion = isBelowMinimumHostVersion(entry);
  const buttonLabel = family
    ? family.has_update
      ? upgrading
        ? i18nText('settings', 'auto.upgrading')
        : belowMinimumHostVersion
          ? i18nText('settings', 'auto.still_update')
          : i18nText('settings', 'auto.upgrade_latest_version')
      : i18nText('settings', 'auto.currently_latest_version')
    : getInstallButtonLabel(entry, installState, activePluginId);
  const buttonDisabled = family ? !family.has_update : installed;
  const compatibilityOverride = belowMinimumHostVersion
    ? ({
        reason: BELOW_MINIMUM_HOST_VERSION,
        acknowledged_current_host_version: entry.current_host_version,
        acknowledged_minimum_host_version: entry.minimum_host_version
      } satisfies SettingsPluginCompatibilityOverride)
    : undefined;

  function confirmInstall() {
    void modal.confirm({
      title: family
        ? i18nText('settings', 'auto.upgrade_plugin')
        : i18nText('settings', 'auto.install_plugin'),
      icon: null,
      centered: true,
      width: INSTALL_CONFIRM_MODAL_WIDTH,
      okText: buttonLabel,
      cancelText: i18nText('settings', 'auto.cancel'),
      okButtonProps: {
        loading: installing || upgrading,
        disabled: buttonDisabled
      },
      content: (
        <OfficialPluginInstallConfirmContent
          entry={entry}
          family={family}
          belowMinimumHostVersion={belowMinimumHostVersion}
        />
      ),
      onOk: async () => {
        if (family) {
          onUpgradeLatest(entry, compatibilityOverride);
          return;
        }

        onInstall(entry, compatibilityOverride);
      }
    });
  }

  return (
    <article className="model-provider-panel__official-card">
      <div className="model-provider-panel__catalog-item-main">
        <div className="model-provider-panel__catalog-item-title-row">
          <img
            className="model-provider-panel__provider-icon"
            src={getOfficialPluginIconSrc(entry)}
            alt=""
            aria-hidden="true"
            loading="lazy"
            onError={(event) => {
              const image = event.currentTarget;
              if (image.src.endsWith(DEFAULT_PROVIDER_ICON_SRC)) {
                return;
              }
              image.src = DEFAULT_PROVIDER_ICON_SRC;
            }}
          />
          <Typography.Title level={5}>{entry.display_name}</Typography.Title>
        </div>
        <div className="model-provider-panel__catalog-item-tag-row">
          {getStatusTags(entry, family, installState, activePluginId).map(
            (tag) => (
              <Tag key={`${entry.plugin_id}-${tag}`} color={getTagColor(tag)}>
                <OfficialPluginTagLabel tag={tag} />
              </Tag>
            )
          )}
        </div>
        {entry.description ? (
          <Typography.Paragraph className="model-provider-panel__official-card-description">
            {entry.description}
          </Typography.Paragraph>
        ) : null}
      </div>

      {canManage ? (
        <div className="model-provider-panel__catalog-item-actions">
          {entry.help_url ? (
            <Button onClick={() => openExternal(entry.help_url!)}>
              {i18nText('settings', 'auto.documentation')}
            </Button>
          ) : null}
          <Button
            type={buttonDisabled ? 'default' : 'primary'}
            loading={installing || upgrading}
            disabled={buttonDisabled}
            onClick={confirmInstall}
          >
            {buttonLabel}
          </Button>
        </div>
      ) : entry.help_url ? (
        <div className="model-provider-panel__catalog-item-actions">
          <Button onClick={() => openExternal(entry.help_url!)}>
            {i18nText('settings', 'auto.documentation')}
          </Button>
        </div>
      ) : null}
    </article>
  );
}

export function OfficialPluginInstallPanel({
  sourceMeta,
  entries,
  catalogErrorMessage,
  familiesByProviderCode,
  loading,
  canManage,
  searchQuery,
  activePluginId,
  installState,
  upgradingProviderCode,
  onInstall,
  onOpenUpload,
  onSearchQueryChange,
  onUpgradeLatest
}: {
  sourceMeta: {
    sourceKind: string;
    sourceLabel: string;
    registryUrl: string;
  } | null;
  entries: SettingsOfficialPluginCatalogEntry[];
  catalogErrorMessage?: string | null;
  familiesByProviderCode: Record<string, SettingsPluginFamilyEntry | undefined>;
  loading?: boolean;
  canManage: boolean;
  searchQuery: string;
  activePluginId: string | null;
  installState: InstallState;
  upgradingProviderCode: string | null;
  onInstall: (
    entry: SettingsOfficialPluginCatalogEntry,
    compatibilityOverride?: SettingsPluginCompatibilityOverride
  ) => void;
  onOpenUpload: () => void;
  onSearchQueryChange: (query: string) => void;
  onUpgradeLatest: (
    entry: SettingsOfficialPluginCatalogEntry,
    compatibilityOverride?: SettingsPluginCompatibilityOverride
  ) => void;
}) {
  const [modal, contextHolder] = Modal.useModal();
  const [selectedPluginId, setSelectedPluginId] = useState<string | null>(null);
  const normalizedEntries = useMemo(() => {
    const grouped = new Map<string, SettingsOfficialPluginCatalogEntry>();

    for (const entry of entries) {
      const existing = grouped.get(entry.provider_code);
      if (!existing) {
        grouped.set(entry.provider_code, entry);
        continue;
      }

      grouped.set(
        entry.provider_code,
        pickPreferredOfficialEntry(
          existing,
          entry,
          familiesByProviderCode[entry.provider_code]
        )
      );
    }

    return Array.from(grouped.values());
  }, [entries, familiesByProviderCode]);
  const visibleEntries = useMemo(() => {
    if (!selectedPluginId) {
      return normalizedEntries;
    }

    return normalizedEntries.filter(
      (entry) => entry.plugin_id === selectedPluginId
    );
  }, [normalizedEntries, selectedPluginId]);

  return (
    <ScrollableSurface className="model-provider-panel__official">
      {contextHolder}
      <div className="model-provider-panel__section-head">
        <div>
          <Typography.Title level={5}>
            {i18nText('settings', 'auto.model_providers')}
          </Typography.Title>
          {sourceMeta ? (
            <Typography.Text type="secondary">
              {i18nText('settings', 'auto.currently_from')}
              {sourceMeta.sourceLabel}
              {i18nText(
                'settings',
                'auto.read_installable_supplier_directory_directly_view_instructions_install_workspace'
              )}
            </Typography.Text>
          ) : null}
          <div className="model-provider-panel__official-toolbar">
            {canManage ? (
              <Button onClick={onOpenUpload}>
                {i18nText('settings', 'auto.upload_plugin')}
              </Button>
            ) : null}
            {sourceMeta ? (
              <Button onClick={() => openExternal(sourceMeta.registryUrl)}>
                {i18nText('settings', 'auto.source')}
              </Button>
            ) : null}
            <Button onClick={() => openExternal(OFFICIAL_PLUGIN_RELEASES_URL)}>
              {i18nText('settings', 'auto.go_warehouse_download')}
            </Button>
          </div>
        </div>
      </div>
      <Select
        allowClear
        showSearch
        className="model-provider-panel__official-select"
        placeholder={i18nText(
          'settings',
          'auto.drop_down_search_installable_vendors'
        )}
        optionFilterProp="label"
        filterOption={false}
        searchValue={searchQuery}
        value={selectedPluginId}
        onChange={(value) => setSelectedPluginId(value ?? null)}
        onSearch={(value) => {
          setSelectedPluginId(null);
          onSearchQueryChange(value);
        }}
        options={normalizedEntries.map((entry) => ({
          value: entry.plugin_id,
          label: `${entry.display_name} / ${entry.protocol}`
        }))}
      />

      {catalogErrorMessage ? (
        <Alert type="error" showIcon message={catalogErrorMessage} />
      ) : null}

      {normalizedEntries.length === 0 ? (
        catalogErrorMessage ? null : (
          <div className="model-provider-panel__empty">
            <Empty
              image={Empty.PRESENTED_IMAGE_SIMPLE}
              description={
                loading
                  ? i18nText(
                      'settings',
                      'auto.loading_official_supplier_catalog'
                    )
                  : i18nText(
                      'settings',
                      'auto.currently_official_supplier_install'
                    )
              }
            />
          </div>
        )
      ) : (
        <div className="model-provider-panel__official-grid">
          {visibleEntries.map((entry) => (
            <OfficialPluginCard
              key={entry.plugin_id}
              entry={entry}
              family={familiesByProviderCode[entry.provider_code]}
              canManage={canManage}
              installState={installState}
              activePluginId={activePluginId}
              upgradingProviderCode={upgradingProviderCode}
              modal={modal}
              onInstall={onInstall}
              onUpgradeLatest={onUpgradeLatest}
            />
          ))}
        </div>
      )}
    </ScrollableSurface>
  );
}
