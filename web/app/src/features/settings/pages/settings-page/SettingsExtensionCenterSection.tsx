import { type ExtensionRow, CATEGORIES, isInstalledRow, supportsFamilyUninstall, extensionName, extensionVersion, extensionHostRequirement, extensionSource, extensionDescription, mcpTemplateWorkspaceStatusLabel, extensionKey } from './extension-center-presentation';
import { useExtensionCenterController } from './useExtensionCenterController';

import { Suspense, lazy } from 'react';

import { Alert, Button, Descriptions, Drawer, Empty, Flex, Input, Modal, Space, Tabs, Tag, Tooltip, Typography } from 'antd';

import '../../../../shared/ui/structured-list/structured-list.css';

import { i18nText } from '../../../../shared/i18n/text';
import { LoadingState } from '../../../../shared/ui/loading-state/LoadingState';
import { DataTable, DataTableColumnSettings } from '../../../../shared/ui/data-table/DataTable';

import { type SettingsExtensionCenterCategory } from '../../api/extensions/extensions';

import { SettingsSectionSurface } from '../../components/SettingsSectionSurface';
import { InstalledExtensionVersionSwitch } from '../../components/extension-center/InstalledExtensionVersionSwitch';
import { ExtensionUploadModal } from '../../components/extension-center/ExtensionUploadModal';
const PricingCatalogPanel = lazy(() =>
  import('../../components/billing/PricingCatalogPanel').then((module) => ({
    default: module.PricingCatalogPanel
  }))
);

const ApplicationTemplateCatalogPanel = lazy(() =>
  import('../../components/extension-center/ApplicationTemplateCatalogPanel').then(
    (module) => ({ default: module.ApplicationTemplateCatalogPanel })
  )
);

export function SettingsExtensionCenterSection(props: {
  category: SettingsExtensionCenterCategory;
  cursor?: string;
  q?: string;
  canManageUiComponents?: boolean;
}) {
  if (props.category === 'application-templates') {
    return (
      <Suspense fallback={<LoadingState compact />}>
        <ApplicationTemplateCatalogPanel cursor={props.cursor} q={props.q} />
      </Suspense>
    );
  }
  if (props.category === 'model-pricing') {
    return (
      <Suspense fallback={<LoadingState compact />}>
        <PricingCatalogPanel />
      </Suspense>
    );
  }
  if (props.category === 'ui-components') {
    return (
      <Suspense fallback={<LoadingState compact />}>
        <UiComponentCatalogPanel
          canManage={props.canManageUiComponents ?? false}
        />
      </Suspense>
    );
  }
  return (
    <GenericExtensionCenterSection
      category={props.category}
      cursor={props.cursor}
      q={props.q}
    />
  );
}

function GenericExtensionCenterSection({
  category: activeTab,
  cursor,
  q
}: {
  category: Exclude<
    SettingsExtensionCenterCategory,
    'model-pricing' | 'ui-components' | 'application-templates'
  >;
  cursor?: string;
  q?: string;
}) {
  const {
    t,
    navigate,
    csrfToken,
    selected,
    setSelected,
    uploadOpen,
    setUploadOpen,
    updateStates,
    applicationTarget,
    setApplicationTarget,
    pendingI18nUpdate,
    setPendingI18nUpdate,
    searchText,
    setSearchText,
    catalogQuery,
    rows,
    updateRows,
    checkVisibleUpdates,
    invalidateExtensionApplicationState,
    closeApplicationFlow,
    deleteInstalledExtensionAsync,
    deletingInstalledExtensionId,
    toggleInstalledExtensionActivation,
    activatingInstallationId,
    submitOperation,
    columns,
    tableConfiguration,
    nextCursor,
    totalEntries,
    tableLoading
  } = useExtensionCenterController({ category: activeTab, cursor, q });

  return (
    <SettingsSectionSurface heightMode="fill">
      <Flex vertical gap={16}>
        {activeTab === 'installed' || activeTab === 'host-extensions' ? (
          <Alert
            data-testid="plugin-settings-upgrade-warning"
            type="warning"
            showIcon
            title={t('auto.plugin_settings_upgrade_warning')}
          />
        ) : null}
        <Tabs
          activeKey={activeTab}
          tabBarExtraContent={
            activeTab === 'mcp' ? (
              <Typography.Link href="/settings/mcp-management?tab=instances">
                {t('auto.go_to_mcp_management')}
              </Typography.Link>
            ) : activeTab === 'i18n' ? (
              <Typography.Link href="/settings/i18n">
                {t('auto.go_to_language_management')}
              </Typography.Link>
            ) : activeTab === 'agent-flow' ? (
              <Typography.Link href="/templates">
                {t('auto.go_to_agent_flow_templates')}
              </Typography.Link>
            ) : null
          }
          onChange={(key) => {
            void navigate({
              to: '/settings/extension-center/$category',
              params: { category: key },
              search: { q: undefined, cursor: undefined }
            });
          }}
          items={[
            { key: 'installed', label: t('auto.installed_extensions') },
            {
              key: 'application-templates',
              label: t('auto.application_templates')
            },
            ...CATEGORIES.map((category) => ({
              key: category,
              label: category
            })),
            {
              key: 'ui-components',
              label: t('auto.ui_components')
            },
            {
              key: 'model-pricing',
              label: t('auto.billing_vendor_model_pricing')
            }
          ]}
        />
        {activeTab !== 'installed' && catalogQuery.isError ? (
          <Alert
            type="error"
            showIcon
            title={t('auto.extension_catalog_load_failed')}
            description={t('auto.extension_catalog_load_failed_description')}
            action={
              <Button onClick={() => void catalogQuery.refetch()}>
                {t('auto.extension_catalog_retry')}
              </Button>
            }
          />
        ) : null}
        {activeTab !== 'installed' &&
        catalogQuery.data?.freshness === 'stale' ? (
          <Alert
            type="warning"
            showIcon
            title={t('auto.extension_catalog_stale')}
          />
        ) : null}
        <DataTable<ExtensionRow>
          rowKey={(row) => extensionKey(row)}
          columns={columns}
          configuration={tableConfiguration}
          dataSource={rows}
          emptyText={<Empty description={t('auto.no_extensions')} />}
          loading={tableLoading}
          toolbar={
            <Flex justify="flex-end" gap={8} wrap>
              {activeTab !== 'installed' ? (
                <Input.Search
                  allowClear
                  aria-label={t('auto.drop_down_search_installable_vendors')}
                  placeholder={t('auto.drop_down_search_installable_vendors')}
                  style={{ width: 240 }}
                  value={searchText}
                  onChange={(event) => setSearchText(event.target.value)}
                  onClear={() => {
                    void navigate({
                      to: '/settings/extension-center/$category',
                      params: { category: activeTab },
                      search: { q: undefined, cursor: undefined }
                    });
                  }}
                  onSearch={(value) => {
                    const normalizedQuery = value.trim();
                    void navigate({
                      to: '/settings/extension-center/$category',
                      params: { category: activeTab },
                      search: {
                        q: normalizedQuery || undefined,
                        cursor: undefined
                      }
                    });
                  }}
                />
              ) : null}
              <Button disabled={!csrfToken} onClick={() => setUploadOpen(true)}>
                {t('auto.upload_plugin')}
              </Button>
              <Button
                disabled={updateRows.length === 0}
                loading={Object.values(updateStates).some(
                  (state) => state === 'checking'
                )}
                onClick={() => void checkVisibleUpdates(updateRows)}
              >
                {t('auto.check_updates')}
              </Button>
              <DataTableColumnSettings
                columns={columns}
                configuration={tableConfiguration}
              />
            </Flex>
          }
          cursorPagination={{
            currentPage: cursor ? 2 : 1,
            hasPreviousPage: Boolean(cursor),
            hasNextPage: Boolean(nextCursor),
            previousLabel: t('auto.previous_page'),
            nextLabel: t('auto.next_page'),
            total: totalEntries,
            onPreviousPage: () => {
              void navigate({
                to: '/settings/extension-center/$category',
                params: { category: activeTab },
                search: { q, cursor: undefined }
              });
            },
            onNextPage: () => {
              if (!nextCursor) return;
              void navigate({
                to: '/settings/extension-center/$category',
                params: { category: activeTab },
                search: { q, cursor: nextCursor }
              });
            }
          }}
        />
      </Flex>

      {uploadOpen && csrfToken && (
        <ExtensionUploadModal
          csrfToken={csrfToken}
          onClose={() => setUploadOpen(false)}
          onInstalled={invalidateExtensionApplicationState}
        />
      )}
      <Drawer
        open={Boolean(selected)}
        title={selected ? extensionName(selected) : undefined}
        size={560}
        onClose={() => setSelected(null)}
      >
        {selected ? (
          <Flex vertical gap={16}>
            <Descriptions column={1} bordered size="small">
              <Descriptions.Item label={t('auto.kind')}>
                {selected.category}
              </Descriptions.Item>
              <Descriptions.Item label={t('auto.description')}>
                {extensionDescription(selected) ?? '—'}
              </Descriptions.Item>
              <Descriptions.Item label={t('auto.current_version')}>
                {extensionVersion(selected)}
              </Descriptions.Item>
              <Descriptions.Item label={t('auto.system_requirements')}>
                {extensionHostRequirement(selected) ?? '—'}
              </Descriptions.Item>
              <Descriptions.Item label={t('auto.source')}>
                {extensionSource(selected)}
              </Descriptions.Item>
              <Descriptions.Item label={t('auto.trust')}>
                {isInstalledRow(selected)
                  ? selected.trust_level
                  : selected.trust}
              </Descriptions.Item>
            </Descriptions>
            {!isInstalledRow(selected) &&
            selected.category === 'mcp' &&
            selected.mcp_instances.length > 0 ? (
              <div className="structured-list structured-list--bordered">
                <div className="structured-list__header">
                  <Typography.Text strong>
                    {t('auto.mcp_instances')}
                  </Typography.Text>
                </div>
                <ul className="structured-list__items">
                  {selected.mcp_instances.map((instance) => (
                    <li
                      className="structured-list__item"
                      key={instance.instance_id}
                    >
                      <div className="structured-list__content">
                        <Flex vertical gap={4}>
                          <Space size={8} wrap>
                            <Typography.Text strong>
                              {instance.name}
                            </Typography.Text>
                            <Tag>
                              {mcpTemplateWorkspaceStatusLabel(
                                instance.workspace_status,
                                t
                              )}
                            </Tag>
                          </Space>
                          <Typography.Text type="secondary">
                            {instance.description_short ?? instance.instance_id}
                          </Typography.Text>
                        </Flex>
                      </div>
                      <div className="structured-list__actions">
                        <Button
                          type="link"
                          disabled={!selected.builtin_template_id}
                          onClick={() => {
                            if (!selected.builtin_template_id) return;
                            setApplicationTarget({
                              builtinTemplateId: selected.builtin_template_id,
                              instanceId: instance.instance_id,
                              action: 'import_mcp'
                            });
                          }}
                        >
                          {i18nText(
                            'settingsMcpManagement',
                            'auto.restore_instance_default'
                          )}
                        </Button>
                      </div>
                    </li>
                  ))}
                </ul>
              </div>
            ) : null}
            {isInstalledRow(selected) ? (
              <div className="structured-list structured-list--bordered">
                <div className="structured-list__header">
                  <Typography.Text strong>
                    {t('auto.installed_versions')}
                  </Typography.Text>
                </div>
                <ul className="structured-list__items">
                  {selected.installed_versions.map((installedVersion) => (
                    <li
                      className="structured-list__item"
                      key={installedVersion.id}
                      data-installation-id={installedVersion.id}
                    >
                      <div className="structured-list__content">
                        <Descriptions column={1} size="small">
                          <Descriptions.Item label={t('auto.version')}>
                            <Space>
                              {installedVersion.version}
                              {installedVersion.is_current && (
                                <Tag>{t('auto.current')}</Tag>
                              )}
                            </Space>
                          </Descriptions.Item>
                          <Descriptions.Item label={t('auto.source')}>
                            {installedVersion.source_kind}
                          </Descriptions.Item>
                          <Descriptions.Item label={t('auto.trust')}>
                            {installedVersion.trust_level}
                          </Descriptions.Item>
                          <Descriptions.Item label={t('auto.signature_status')}>
                            {installedVersion.signature_status}
                          </Descriptions.Item>
                          <Descriptions.Item label={t('auto.checksum')}>
                            <Typography.Text copyable ellipsis>
                              {installedVersion.local_checksum ??
                                installedVersion.expected_checksum ??
                                '—'}
                            </Typography.Text>
                          </Descriptions.Item>
                          <Descriptions.Item label={t('auto.local_path')}>
                            <Typography.Text copyable ellipsis>
                              {installedVersion.local_path ?? '—'}
                            </Typography.Text>
                          </Descriptions.Item>
                        </Descriptions>
                      </div>
                      {supportsFamilyUninstall(selected.category) &&
                      csrfToken ? (
                        <InstalledExtensionVersionSwitch
                          installation={selected}
                          version={installedVersion}
                          csrfToken={csrfToken}
                          onChanged={async () => {
                            setSelected(null);
                            await invalidateExtensionApplicationState();
                          }}
                        />
                      ) : null}
                      {!supportsFamilyUninstall(selected.category) ? (
                        <div className="structured-list__actions">
                          {selected.category === 'host-extensions' ? (
                            <Button
                              type="link"
                              disabled={
                                installedVersion.is_current &&
                                selected.desired_state !== 'disabled'
                              }
                              loading={
                                activatingInstallationId === installedVersion.id
                              }
                              onClick={() =>
                                toggleInstalledExtensionActivation(
                                  {
                                    installationId: installedVersion.id,
                                    enabled: true
                                  },
                                  { onSuccess: () => setSelected(null) }
                                )
                              }
                            >
                              {t('auto.select_installed_plugin_version')}
                            </Button>
                          ) : null}
                          <Tooltip
                            title={
                              installedVersion.deletable
                                ? undefined
                                : installedVersion.delete_reasons.join(', ')
                            }
                          >
                            <Button
                              type="link"
                              danger
                              disabled={!installedVersion.deletable}
                              loading={
                                deletingInstalledExtensionId ===
                                installedVersion.id
                              }
                              onClick={() =>
                                Modal.confirm({
                                  title: t('auto.confirm_delete'),
                                  content: installedVersion.version,
                                  okText: t('auto.delete'),
                                  cancelText: t('auto.cancel'),
                                  okButtonProps: { danger: true },
                                  onOk: () =>
                                    deleteInstalledExtensionAsync(
                                      installedVersion.id
                                    )
                                })
                              }
                            >
                              {t('auto.delete')}
                            </Button>
                          </Tooltip>
                        </div>
                      ) : null}
                    </li>
                  ))}
                </ul>
              </div>
            ) : null}
          </Flex>
        ) : null}
      </Drawer>
      <Modal
        open={Boolean(pendingI18nUpdate)}
        title={t('auto.update_translation_catalog')}
        onCancel={() => setPendingI18nUpdate(null)}
        footer={
          <Space>
            <Button onClick={() => setPendingI18nUpdate(null)}>
              {t('auto.cancel')}
            </Button>
            <Button
              onClick={() => {
                if (!pendingI18nUpdate) return;
                const operation = pendingI18nUpdate;
                setPendingI18nUpdate(null);
                submitOperation({
                  ...operation,
                  activateI18nAfterInstall: false
                });
              }}
            >
              {t('auto.install_new_version_only')}
            </Button>
            <Button
              type="primary"
              onClick={() => {
                if (!pendingI18nUpdate) return;
                const operation = pendingI18nUpdate;
                setPendingI18nUpdate(null);
                submitOperation({
                  ...operation,
                  activateI18nAfterInstall: true
                });
              }}
            >
              {t('auto.install_and_activate')}
            </Button>
          </Space>
        }
      >
        <Typography.Paragraph>
          {t('auto.translation_catalog_update_choice_description')}
        </Typography.Paragraph>
      </Modal>
      {applicationTarget ? (
        <Suspense fallback={null}>
          <ExtensionApplicationFlow
            target={applicationTarget}
            csrfToken={csrfToken ?? ''}
            onClose={closeApplicationFlow}
            onApplied={invalidateExtensionApplicationState}
          />
        </Suspense>
      ) : null}
    </SettingsSectionSurface>
  );
}



const UiComponentCatalogPanel = lazy(() =>
  import('../../components/extension-center/UiComponentCatalogPanel').then(
    (module) => ({ default: module.UiComponentCatalogPanel })
  )
);


const ExtensionApplicationFlow = lazy(() =>
  import('../../components/extension-center/ExtensionApplicationFlow').then(
    (module) => ({ default: module.ExtensionApplicationFlow })
  )
);