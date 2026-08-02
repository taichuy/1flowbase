import { useCallback, useState } from 'react';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Button,
  Descriptions,
  Drawer,
  Empty,
  List,
  Modal,
  Space,
  Table,
  Tag,
  Typography,
  message,
  type TableProps
} from 'antd';
import { useTranslation } from 'react-i18next';

import { useAuthStore } from '../../../state/auth-store';
import { applicationsQueryKey } from '../../applications/api/applications';
import { InstalledAgentFlowImportFlow } from '../../applications/components/InstalledAgentFlowImportFlow';
import {
  deleteInstalledAgentFlowVersion,
  fetchInstalledAgentFlowTemplates,
  installedAgentFlowTemplatesQueryKey,
  selectInstalledAgentFlowVersion,
  type InstalledAgentFlowFamily
} from '../api/templates';
import './agent-flow-template-library.css';

type InstalledVersion = InstalledAgentFlowFamily['installed_versions'][number];

export function AgentFlowTemplateLibrary() {
  const { t } = useTranslation('templates');
  const csrfToken = useAuthStore((state) => state.csrfToken) ?? '';
  const actor = useAuthStore((state) => state.actor);
  const me = useAuthStore((state) => state.me);
  const queryClient = useQueryClient();
  const [messageApi, messageContextHolder] = message.useMessage();
  const [modalApi, modalContextHolder] = Modal.useModal();
  const [selectedFamily, setSelectedFamily] =
    useState<InstalledAgentFlowFamily | null>(null);
  const [importInstallationId, setImportInstallationId] = useState<
    string | null
  >(null);
  const [pendingInstallationId, setPendingInstallationId] = useState<
    string | null
  >(null);
  const canCreate =
    actor?.effective_display_role === 'root' ||
    Boolean(me?.permissions.includes('application.create.all'));
  const closeInstalledTemplateImport = useCallback(
    () => setImportInstallationId(null),
    []
  );
  const finishInstalledTemplateImport = useCallback(
    async (applicationId: string) => {
      await queryClient.invalidateQueries({ queryKey: applicationsQueryKey });
      messageApi.success(t('auto.template_imported'));
      window.location.assign(`/applications/${applicationId}/orchestration`);
    },
    [messageApi, queryClient, t]
  );

  const installedQuery = useQuery({
    queryKey: installedAgentFlowTemplatesQueryKey,
    queryFn: fetchInstalledAgentFlowTemplates,
    retry: false
  });

  const refreshInstalled = async () => {
    await queryClient.invalidateQueries({
      queryKey: installedAgentFlowTemplatesQueryKey
    });
  };

  const selectMutation = useMutation({
    mutationFn: (installationId: string) =>
      selectInstalledAgentFlowVersion(installationId, csrfToken),
    onSuccess: async () => {
      await refreshInstalled();
      setSelectedFamily(null);
      messageApi.success(t('auto.current_version_changed'));
    },
    onError: () => messageApi.error(t('auto.current_version_change_failed')),
    onSettled: () => setPendingInstallationId(null)
  });

  const deleteMutation = useMutation({
    mutationFn: (installationId: string) =>
      deleteInstalledAgentFlowVersion(installationId, csrfToken),
    onSuccess: async () => {
      await refreshInstalled();
      setSelectedFamily(null);
      messageApi.success(t('auto.template_release_deleted'));
    },
    onError: () => messageApi.error(t('auto.template_release_delete_failed')),
    onSettled: () => setPendingInstallationId(null)
  });

  const families = installedQuery.data?.entries ?? [];
  const columns: TableProps<InstalledAgentFlowFamily>['columns'] = [
    {
      title: t('auto.template_info'),
      key: 'template',
      render: (_, family) => (
        <Space direction="vertical" size={0}>
          <Typography.Text strong>{family.artifact_id}</Typography.Text>
          <Typography.Text type="secondary">
            {family.catalog_id}
          </Typography.Text>
        </Space>
      )
    },
    {
      title: t('auto.current_version'),
      key: 'current',
      width: 160,
      render: (_, family) =>
        family.installed_versions.find((version) => version.is_current)
          ?.version ?? '—'
    },
    {
      title: t('auto.installed_versions'),
      key: 'history',
      width: 160,
      render: (_, family) => family.installed_versions.length
    },
    {
      title: t('auto.actions'),
      key: 'actions',
      width: 180,
      render: (_, family) => {
        const current = family.installed_versions.find(
          (version) => version.is_current
        );
        return (
          <Space size="small">
            <Button
              type="link"
              disabled={!current || !canCreate}
              onClick={() => current && setImportInstallationId(current.id)}
            >
              {t('auto.import_template')}
            </Button>
            <Button type="link" onClick={() => setSelectedFamily(family)}>
              {t('auto.view')}
            </Button>
          </Space>
        );
      }
    }
  ];

  const confirmDelete = (version: InstalledVersion) => {
    modalApi.confirm({
      title: t('auto.confirm_delete_release'),
      okText: t('auto.delete'),
      okButtonProps: { danger: true },
      cancelText: t('auto.cancel'),
      onOk: () => {
        setPendingInstallationId(version.id);
        return deleteMutation.mutateAsync(version.id);
      }
    });
  };

  return (
    <div className="agent-flow-template-library">
      {messageContextHolder}
      {modalContextHolder}
      {installedQuery.isError ? (
        <Empty description={t('auto.catalog_load_failed')}>
          <Button onClick={() => void installedQuery.refetch()}>
            {t('auto.retry')}
          </Button>
        </Empty>
      ) : (
        <Table
          rowKey="catalog_id"
          columns={columns}
          dataSource={families}
          loading={installedQuery.isPending}
          pagination={false}
          locale={{
            emptyText: (
              <Empty description={t('auto.empty_catalog')}>
                <Typography.Link href="/settings/extension-center/agent-flow">
                  {t('auto.go_to_agent_flow_extension_center')}
                </Typography.Link>
              </Empty>
            )
          }}
        />
      )}

      <Drawer
        open={Boolean(selectedFamily)}
        title={selectedFamily?.artifact_id}
        width={640}
        onClose={() => setSelectedFamily(null)}
      >
        {selectedFamily ? (
          <Space direction="vertical" size={16} style={{ width: '100%' }}>
            <Descriptions bordered column={1} size="small">
              <Descriptions.Item label={t('auto.template_id')}>
                {selectedFamily.catalog_id}
              </Descriptions.Item>
              <Descriptions.Item label={t('auto.current_version')}>
                {selectedFamily.installed_versions.find(
                  (version) => version.is_current
                )?.version ?? '—'}
              </Descriptions.Item>
            </Descriptions>
            <List
              bordered
              dataSource={selectedFamily.installed_versions}
              renderItem={(version) => (
                <List.Item
                  aria-label={`${version.version}${
                    version.is_current ? ` ${t('auto.current')}` : ''
                  }`}
                  actions={[
                    <Button
                      key="import"
                      type="link"
                      disabled={!canCreate || pendingInstallationId !== null}
                      onClick={() => setImportInstallationId(version.id)}
                    >
                      {t('auto.import_this_version')}
                    </Button>,
                    <Button
                      key="current"
                      type="link"
                      disabled={
                        version.is_current ||
                        (pendingInstallationId !== null &&
                          pendingInstallationId !== version.id)
                      }
                      loading={pendingInstallationId === version.id}
                      onClick={() => {
                        setPendingInstallationId(version.id);
                        selectMutation.mutate(version.id);
                      }}
                    >
                      {version.is_current
                        ? t('auto.current')
                        : t('auto.set_current')}
                    </Button>,
                    <Button
                      key="delete"
                      type="link"
                      danger
                      disabled={
                        pendingInstallationId !== null &&
                        pendingInstallationId !== version.id
                      }
                      loading={pendingInstallationId === version.id}
                      onClick={() => confirmDelete(version)}
                    >
                      {t('auto.delete')}
                    </Button>
                  ]}
                >
                  <List.Item.Meta
                    title={
                      <Space>
                        <Typography.Text>{version.version}</Typography.Text>
                        {version.is_current ? (
                          <Tag color="success">{t('auto.current')}</Tag>
                        ) : null}
                      </Space>
                    }
                    description={version.source}
                  />
                </List.Item>
              )}
            />
          </Space>
        ) : null}
      </Drawer>

      <InstalledAgentFlowImportFlow
        installationId={importInstallationId}
        csrfToken={csrfToken}
        onClose={closeInstalledTemplateImport}
        onImported={finishInstalledTemplateImport}
      />
    </div>
  );
}
