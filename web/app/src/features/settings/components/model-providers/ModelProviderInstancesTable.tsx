import { Descriptions, Empty, Switch, Tag, Tooltip, Typography } from 'antd';

import { CollapseShell } from '../../../../shared/ui/collapse-shell/CollapseShell';
import type { SettingsModelProviderInstance } from '../../api/model-providers';
import { ModelProviderTagList } from './ModelProviderTagList';
import { i18nText } from '../../../../shared/i18n/text';

function renderStatusTag(status: string) {
  switch (status) {
    case 'ready':
      return (
        <Tag
          className="model-provider-panel__instance-status-tag"
          color="green"
          variant="filled"
        >
          ready
        </Tag>
      );
    case 'invalid':
      return (
        <Tag
          className="model-provider-panel__instance-status-tag"
          color="red"
          variant="filled"
        >
          invalid
        </Tag>
      );
    case 'disabled':
      return (
        <Tag className="model-provider-panel__instance-status-tag" variant="filled">
          disabled
        </Tag>
      );
    default:
      return (
        <Tag
          className="model-provider-panel__instance-status-tag"
          color="gold"
          variant="filled"
        >
          {status}
        </Tag>
      );
  }
}

function formatCatalogRefreshedAt(value: string | null) {
  if (!value) {
    return i18nText("settings", "auto.not_refreshed");
  }

  const matched = value.match(
    /^(\d{4}-\d{2}-\d{2})[T\s](\d{2}:\d{2}:\d{2})/
  );

  if (!matched) {
    return value;
  }

  return `${matched[1]} ${matched[2]}`;
}

export function ModelProviderInstancesTable({
  instances,
  loading,
  canManage,
  updatingInstanceId,
  onToggleIncludedInMain,
  onEdit,
  onRefreshCandidates,
  onRefreshModels,
  onDelete
}: {
  instances: SettingsModelProviderInstance[];
  loading?: boolean;
  canManage: boolean;
  updatingInstanceId?: string | null;
  onToggleIncludedInMain: (
    instance: SettingsModelProviderInstance,
    checked: boolean
  ) => void;
  onEdit: (instance: SettingsModelProviderInstance) => void;
  onRefreshCandidates: (instance: SettingsModelProviderInstance) => void;
  onRefreshModels: (instance: SettingsModelProviderInstance) => void;
  onDelete: (instance: SettingsModelProviderInstance) => void;
}) {
  if (instances.length === 0) {
    return (
      <section className="model-provider-panel__instances">
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={loading ? i18nText("settings", "auto.loading_instances") : i18nText("settings", "auto.currently_model_supplier_instance")}
        />
      </section>
    );
  }

  return (
    <section className="model-provider-panel__instances">
      <CollapseShell
        accordion
        className="model-provider-panel__instances-collapse"
        defaultActiveKey={instances[0] ? [instances[0].id] : undefined}
        items={instances.map((instance) => ({
          key: instance.id,
          header: (
            <div className="model-provider-panel__instance-header">
              <div className="model-provider-panel__instance-header-main">
                <div className="model-provider-panel__instance-title-row">
                  <span className="model-provider-panel__instance-title">
                    {instance.display_name}
                  </span>
                  {renderStatusTag(instance.status)}
                </div>
              </div>

              <div className="model-provider-panel__instance-header-side">
                <div className="model-provider-panel__instance-inclusion-card">
                  <span className="model-provider-panel__instance-stat-label">
                    {i18nText("settings", "auto.inject_main_instance_alt")}</span>
                  <div className="model-provider-panel__instance-inclusion-row">
                    <Switch
                      aria-label={i18nText("settings", "auto.inject_main_instance", { value1: instance.display_name })}
                      checked={instance.included_in_main}
                      disabled={!canManage || updatingInstanceId === instance.id}
                      onClick={(_, event) => {
                        event?.stopPropagation();
                      }}
                      onChange={(checked) => {
                        onToggleIncludedInMain(instance, checked);
                      }}
                    />
                    <Typography.Text type="secondary">
                      {instance.included_in_main ? i18nText("settings", "auto.already_connected") : i18nText("settings", "auto.not_connected")}
                    </Typography.Text>
                  </div>
                </div>

                <div className="model-provider-panel__instance-stats">
                  <div className="model-provider-panel__instance-stat">
                    <span className="model-provider-panel__instance-stat-label">
                      {i18nText("settings", "auto.effective_model")}</span>
                    <span className="model-provider-panel__instance-stat-value">
                      {instance.enabled_model_ids.length}
                    </span>
                  </div>
                  <div className="model-provider-panel__instance-stat">
                    <span className="model-provider-panel__instance-stat-label">
                      {i18nText("settings", "auto.cache_model")}</span>
                    <span className="model-provider-panel__instance-stat-value">
                      {instance.model_count}
                    </span>
                  </div>
                </div>
              </div>
            </div>
          ),
          children: (
            <>
              <Descriptions
                className="model-provider-panel__instance-descriptions"
                size="small"
                column={1}
                items={[
                  {
                    key: 'base-url',
                    label: 'Base URL',
                    children: (
                      <Typography.Paragraph
                        className="model-provider-panel__instance-baseurl-value"
                        ellipsis={{ rows: 2, expandable: true, symbol: i18nText("settings", "auto.expand") }}
                        style={{ marginBottom: 0 }}
                      >
                        {String(instance.config_json.base_url ?? i18nText("settings", "auto.not_configured"))}
                      </Typography.Paragraph>
                    )
                  },
                  {
                    key: 'enabled-models',
                    label: i18nText("settings", "auto.effective_model"),
                    children: (
                      <ModelProviderTagList
                        modelIds={instance.enabled_model_ids}
                        emptyText={i18nText("settings", "auto.not_set")}
                      />
                    )
                  },
                  {
                    key: 'refreshed-at',
                    label: i18nText("settings", "auto.recently_refreshed"),
                    children: (
                      <Typography.Text type="secondary">
                        {formatCatalogRefreshedAt(instance.catalog_refreshed_at)}
                      </Typography.Text>
                    )
                  }
                ]}
              />

              {canManage ? (
                <div className="model-provider-panel__instance-actions">
                  <Tooltip
                    title={i18nText(
                      "settings",
                      "auto.edit_model_provider_instance_tip"
                    )}
                  >
                    <button
                      type="button"
                      className="model-provider-panel__instance-action-btn"
                      onClick={() => onEdit(instance)}
                      aria-label={i18nText("settings", "auto.edit_api_key", { value1: instance.display_name })}
                    >
                      {i18nText("settings", "auto.edit")}</button>
                  </Tooltip>
                  <Tooltip
                    title={i18nText(
                      "settings",
                      "auto.detect_model_provider_instance_tip"
                    )}
                  >
                    <button
                      type="button"
                      className="model-provider-panel__instance-action-btn"
                      onClick={() => onRefreshCandidates(instance)}
                      aria-label={i18nText("settings", "auto.refresh_candidate_model", { value1: instance.display_name })}
                    >
                      {i18nText("settings", "auto.detection")}</button>
                  </Tooltip>
                  <Tooltip
                    title={i18nText(
                      "settings",
                      "auto.refresh_model_provider_instance_tip"
                    )}
                  >
                    <button
                      type="button"
                      className="model-provider-panel__instance-action-btn"
                      onClick={() => onRefreshModels(instance)}
                      aria-label={i18nText("settings", "auto.refresh_model_alt", { value1: instance.display_name })}
                    >
                      {i18nText("settings", "auto.refresh")}</button>
                  </Tooltip>
                  <Tooltip
                    title={i18nText(
                      "settings",
                      "auto.delete_model_provider_instance_tip"
                    )}
                  >
                    <button
                      type="button"
                      className="model-provider-panel__instance-action-btn model-provider-panel__instance-action-btn--danger"
                      onClick={() => onDelete(instance)}
                      aria-label={i18nText("settings", "auto.delete_instance_alt", { value1: instance.display_name })}
                    >
                      {i18nText("settings", "auto.delete")}</button>
                  </Tooltip>
                </div>
              ) : null}
            </>
          )
        }))}
      />
    </section>
  );
}
