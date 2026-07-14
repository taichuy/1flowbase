import {
  CopyOutlined,
  DeleteOutlined,
  EditOutlined,
  ExportOutlined,
  MoreOutlined,
  TagOutlined
} from '@ant-design/icons';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Button,
  Dropdown,
  Flex,
  Input,
  message,
  Modal,
  Select,
  Space,
  Tag,
  Typography,
  type MenuProps
} from 'antd';
import { useCallback, useEffect, useMemo, useState } from 'react';

import { useAuthStore } from '../../../../state/auth-store';
import { formatDateTime } from '../../../../shared/i18n/format';
import { i18nText } from '../../../../shared/i18n/text';
import {
  DataTable,
  DataTableColumnSettings,
  type DataTableColumn
} from '../../../../shared/ui/data-table/DataTable';
import { usePersistedDataTableConfiguration } from '../../../../shared/ui/data-table/data-table-state';
import {
  applicationCatalogQueryKey,
  applicationsQueryKey,
  createApplication,
  createApplicationTag,
  deleteApplication,
  exportAgentFlowTemplate,
  fetchApplicationCatalog,
  updateApplication
} from '../../../applications/api/applications';
import { ApplicationEditModal } from '../../../applications/components/ApplicationEditModal';
import { ApplicationTagManagerModal } from '../../../applications/components/ApplicationTagManagerModal';
import { downloadTemplateFile } from '../../../applications/lib/template-download';
import { SettingsSectionSurface } from '../SettingsSectionSurface';
import {
  fetchSettingsApplicationManagement,
  settingsApplicationManagementQueryKey,
  settingsApplicationManagementQueryPrefix,
  type SettingsApplicationManagementItem,
  type SettingsApplicationManagementQuery
} from '../../api/application-management';
import {
  pushApplicationManagementRouteState,
  readApplicationManagementRouteState,
  type ApplicationManagementRouteState
} from './application-management-route-state';
import './application-management-panel.css';

const PAGE_SIZE = 20;

function applicationTypeLabel(
  applicationType: SettingsApplicationManagementItem['application_type']
) {
  return applicationType === 'agent_flow'
    ? i18nText('applications', 'auto.application_type_agent_flow')
    : i18nText('applications', 'auto.application_type_workflow');
}

function triggerTypeLabel(
  triggerType: SettingsApplicationManagementItem['workflow_trigger_type']
) {
  if (triggerType === 'schedule') {
    return i18nText('settings', 'auto.workflow_trigger_schedule');
  }
  if (triggerType === 'extension') {
    return i18nText('settings', 'auto.workflow_trigger_extension');
  }
  return '—';
}

function managementFilter(
  state: ApplicationManagementRouteState
): Record<string, unknown> | undefined {
  const clauses: Array<Record<string, unknown>> = [];
  if (state.application_type) {
    clauses.push({ application_type: state.application_type });
  }
  if (state.publication_status) {
    clauses.push({ publication_status: state.publication_status });
  }
  if (state.created_by) {
    clauses.push({ created_by: state.created_by });
  }
  if (state.tag_id) {
    clauses.push({ 'tags.id': state.tag_id });
  }
  if (state.keyword) {
    clauses.push({
      $or: [
        { name: { $includes: state.keyword } },
        { id: { $includes: state.keyword } }
      ]
    });
  }

  return clauses.length > 0 ? { $and: clauses } : undefined;
}

export function ApplicationManagementPanel() {
  const actor = useAuthStore((state) => state.actor);
  const me = useAuthStore((state) => state.me);
  const csrfToken = useAuthStore((state) => state.csrfToken) ?? '';
  const queryClient = useQueryClient();
  const [messageApi, messageContextHolder] = message.useMessage();
  const [modalApi, modalContextHolder] = Modal.useModal();
  const [routeState, setRouteState] = useState(
    readApplicationManagementRouteState
  );
  const [keywordDraft, setKeywordDraft] = useState(routeState.keyword ?? '');
  const [editingApplication, setEditingApplication] =
    useState<SettingsApplicationManagementItem | null>(null);
  const [taggingApplication, setTaggingApplication] =
    useState<SettingsApplicationManagementItem | null>(null);

  useEffect(() => {
    const handlePopState = () => {
      const nextState = readApplicationManagementRouteState();
      setRouteState(nextState);
      setKeywordDraft(nextState.keyword ?? '');
    };
    window.addEventListener('popstate', handlePopState);
    return () => window.removeEventListener('popstate', handlePopState);
  }, []);

  const updateRouteState = useCallback(
    (patch: Partial<ApplicationManagementRouteState>) => {
      const nextState = { ...routeState, ...patch };
      pushApplicationManagementRouteState(nextState);
      setRouteState(nextState);
    },
    [routeState]
  );
  const managementQuery = useMemo<SettingsApplicationManagementQuery>(
    () => ({
      page: routeState.page,
      page_size: PAGE_SIZE,
      filter: managementFilter(routeState),
      sort: routeState.sort
    }),
    [routeState]
  );
  const applicationsQuery = useQuery({
    queryKey: settingsApplicationManagementQueryKey(managementQuery),
    queryFn: () => fetchSettingsApplicationManagement(managementQuery)
  });
  const catalogQuery = useQuery({
    queryKey: applicationCatalogQueryKey,
    queryFn: fetchApplicationCatalog
  });

  const invalidateApplications = useCallback(async () => {
    await Promise.all([
      queryClient.invalidateQueries({
        queryKey: settingsApplicationManagementQueryPrefix
      }),
      queryClient.invalidateQueries({ queryKey: applicationsQueryKey }),
      queryClient.invalidateQueries({ queryKey: applicationCatalogQueryKey })
    ]);
  }, [queryClient]);
  const updateMutation = useMutation({
    mutationFn: ({
      application,
      name,
      description,
      tagIds
    }: {
      application: SettingsApplicationManagementItem;
      name: string;
      description: string;
      tagIds: string[];
    }) =>
      updateApplication(
        application.id,
        { name, description, tag_ids: tagIds },
        csrfToken
      ),
    onSuccess: invalidateApplications,
    onError: () => messageApi.error(i18nText('applications', 'auto.save_failed'))
  });
  const deleteMutation = useMutation({
    mutationFn: (applicationId: string) =>
      deleteApplication(applicationId, csrfToken),
    onSuccess: async () => {
      await invalidateApplications();
      messageApi.success(
        i18nText('applications', 'auto.application_deleted')
      );
    },
    onError: () =>
      messageApi.error(
        i18nText('applications', 'auto.delete_application_failed')
      )
  });
  const copyMutation = useMutation({
    mutationFn: (application: SettingsApplicationManagementItem) =>
      createApplication(
        {
          application_type: application.application_type,
          name: i18nText('applications', 'auto.copied_application_name', {
            value1: application.name
          }),
          description: application.description,
          icon: application.icon,
          icon_type: application.icon_type,
          icon_background: application.icon_background
        },
        csrfToken
      ),
    onSuccess: async () => {
      await invalidateApplications();
      messageApi.success(i18nText('applications', 'auto.application_copied'));
    },
    onError: () =>
      messageApi.error(
        i18nText('applications', 'auto.copy_application_failed')
      )
  });
  const exportMutation = useMutation({
    mutationFn: exportAgentFlowTemplate,
    onSuccess: (template) => {
      downloadTemplateFile(template);
      messageApi.success(i18nText('applications', 'auto.template_exported'));
    },
    onError: () =>
      messageApi.error(i18nText('applications', 'auto.template_export_failed'))
  });
  const createTagMutation = useMutation({
    mutationFn: (name: string) => createApplicationTag({ name }, csrfToken),
    onSuccess: invalidateApplications
  });
  const copyApplication = copyMutation.mutate;
  const exportApplication = exportMutation.mutate;
  const deleteApplicationById = deleteMutation.mutateAsync;

  const permissions = me?.permissions ?? [];
  const isRoot = actor?.effective_display_role === 'root';
  const canCreate = isRoot || permissions.includes('application.create.all');
  const canEditAny = isRoot || permissions.includes('application.edit.all');
  const canEditOwn = permissions.includes('application.edit.own');
  const canDeleteAny = isRoot || permissions.includes('application.delete.all');
  const canDeleteOwn = permissions.includes('application.delete.own');
  const canEdit = useCallback(
    (application: SettingsApplicationManagementItem) =>
      canEditAny || (canEditOwn && application.created_by === actor?.id),
    [actor?.id, canEditAny, canEditOwn]
  );
  const canDelete = useCallback(
    (application: SettingsApplicationManagementItem) =>
      canDeleteAny || (canDeleteOwn && application.created_by === actor?.id),
    [actor?.id, canDeleteAny, canDeleteOwn]
  );

  const confirmDelete = useCallback(
    (application: SettingsApplicationManagementItem) => {
      modalApi.confirm({
        title: i18nText('applications', 'auto.delete_application'),
        content: `${i18nText('applications', 'auto.delete_application_content_prefix')}${application.name}${i18nText('applications', 'auto.delete_application_content_suffix')}`,
        okText: i18nText('applications', 'auto.delete'),
        okButtonProps: { danger: true },
        cancelText: i18nText('applications', 'auto.cancel'),
        onOk: () => deleteApplicationById(application.id)
      });
    },
    [deleteApplicationById, modalApi]
  );

  const columns = useMemo<
    Array<DataTableColumn<SettingsApplicationManagementItem>>
  >(
    () => [
      {
        key: 'application',
        title: i18nText('settings', 'auto.application_management_application'),
        width: 260,
        render: (_, application) => (
          <Flex vertical gap={2}>
            <Typography.Link
              href={`/applications/${encodeURIComponent(application.id)}/orchestration`}
            >
              {application.name}
            </Typography.Link>
            <Typography.Text type="secondary" ellipsis>
              {application.description ||
                i18nText('applications', 'auto.application_description_empty')}
            </Typography.Text>
          </Flex>
        )
      },
      {
        key: 'application_type',
        title: i18nText('settings', 'auto.application_management_type'),
        width: 120,
        render: (_, application) => (
          <Tag>{applicationTypeLabel(application.application_type)}</Tag>
        )
      },
      {
        key: 'workflow_trigger_type',
        title: i18nText('settings', 'auto.application_management_trigger'),
        width: 120,
        render: (_, application) =>
          triggerTypeLabel(application.workflow_trigger_type)
      },
      {
        key: 'publication_status',
        title: i18nText('settings', 'auto.application_management_publication_status'),
        width: 120,
        render: (_, application) => (
          <Tag
            color={
              application.publication_status === 'published'
                ? 'success'
                : 'default'
            }
          >
            {application.publication_status === 'published'
              ? i18nText('settings', 'auto.published')
              : i18nText('settings', 'auto.unpublished')}
          </Tag>
        )
      },
      {
        key: 'created_by_display_name',
        title: i18nText('settings', 'auto.application_management_creator'),
        dataIndex: 'created_by_display_name',
        width: 150
      },
      {
        key: 'tags',
        title: i18nText('settings', 'auto.application_management_tags'),
        width: 180,
        render: (_, application) =>
          application.tags.length > 0 ? (
            <Space size={[4, 4]} wrap>
              {application.tags.map((tag) => (
                <Tag key={tag.id}>{tag.name}</Tag>
              ))}
            </Space>
          ) : (
            '—'
          )
      },
      {
        key: 'created_at',
        title: i18nText('applications', 'auto.created_at'),
        dataIndex: 'created_at',
        width: 180,
        render: (value) => formatDateTime(value as string)
      },
      {
        key: 'updated_at',
        title: i18nText('applications', 'auto.updated_at'),
        dataIndex: 'updated_at',
        width: 180,
        render: (value) => formatDateTime(value as string)
      },
      {
        key: 'actions',
        title: i18nText('settings', 'auto.application_management_actions'),
        width: 90,
        render: (_, application) => {
          const editAllowed = canEdit(application);
          const deleteAllowed = canDelete(application);
          const items: MenuProps['items'] = [
            {
              key: 'edit',
              icon: <EditOutlined />,
              label: i18nText('applications', 'auto.edit_information'),
              disabled: !editAllowed
            },
            {
              key: 'tags',
              icon: <TagOutlined />,
              label: i18nText('applications', 'auto.manage_tags'),
              disabled: !editAllowed
            },
            {
              key: 'copy',
              icon: <CopyOutlined />,
              label: i18nText('applications', 'auto.copy'),
              disabled: !canCreate
            },
            {
              key: 'export',
              icon: <ExportOutlined />,
              label: i18nText('applications', 'auto.export_template'),
              disabled: application.application_type !== 'agent_flow'
            },
            { type: 'divider' },
            {
              key: 'delete',
              icon: <DeleteOutlined />,
              label: i18nText('applications', 'auto.delete'),
              danger: true,
              disabled: !deleteAllowed
            }
          ];
          return (
            <Dropdown
              menu={{
                items,
                onClick: ({ key }) => {
                  if (key === 'edit') setEditingApplication(application);
                  if (key === 'tags') setTaggingApplication(application);
                  if (key === 'copy') copyApplication(application);
                  if (key === 'export') exportApplication(application.id);
                  if (key === 'delete') confirmDelete(application);
                }
              }}
              trigger={['click']}
            >
              <Button
                type="text"
                icon={<MoreOutlined />}
                aria-label={i18nText(
                  'applications',
                  'auto.more_actions_named',
                  { value1: application.name }
                )}
              />
            </Dropdown>
          );
        }
      }
    ],
    [
      canCreate,
      canDelete,
      canEdit,
      confirmDelete,
      copyApplication,
      exportApplication
    ]
  );
  const tableConfiguration = usePersistedDataTableConfiguration({
    columns,
    storageKey: 'settings.application_management'
  });
  const catalog = catalogQuery.data ?? { types: [], tags: [] };

  return (
    <SettingsSectionSurface
      heightMode="fill"
      toolbar={
        <Flex justify="space-between" gap={12} wrap>
          <Flex gap={12} wrap>
            <Select
              allowClear
              aria-label={i18nText('settings', 'auto.application_management_type')}
              placeholder={i18nText(
                'settings',
                'auto.application_management_all_types'
              )}
              value={routeState.application_type}
              options={catalog.types}
              style={{ width: 150 }}
              onChange={(application_type) =>
                updateRouteState({ page: 1, application_type })
              }
            />
            <Select
              allowClear
              aria-label={i18nText(
                'settings',
                'auto.application_management_publication_status'
              )}
              placeholder={i18nText(
                'settings',
                'auto.application_management_all_publication_statuses'
              )}
              value={routeState.publication_status}
              options={[
                {
                  value: 'published',
                  label: i18nText('settings', 'auto.published')
                },
                {
                  value: 'unpublished',
                  label: i18nText('settings', 'auto.unpublished')
                }
              ]}
              style={{ width: 160 }}
              onChange={(publication_status) =>
                updateRouteState({ page: 1, publication_status })
              }
            />
            <Input
              aria-label={i18nText(
                'settings',
                'auto.application_management_creator_id'
              )}
              placeholder={i18nText(
                'settings',
                'auto.application_management_creator_id'
              )}
              value={routeState.created_by ?? ''}
              style={{ width: 190 }}
              onChange={(event) =>
                updateRouteState({
                  page: 1,
                  created_by: event.target.value.trim() || undefined
                })
              }
            />
            <Select
              allowClear
              aria-label={i18nText('settings', 'auto.application_management_tags')}
              placeholder={i18nText(
                'settings',
                'auto.application_management_all_tags'
              )}
              value={routeState.tag_id}
              options={catalog.tags.map((tag) => ({
                value: tag.id,
                label: tag.name
              }))}
              style={{ width: 160 }}
              onChange={(tag_id) => updateRouteState({ page: 1, tag_id })}
            />
            <Input.Search
              aria-label={i18nText(
                'settings',
                'auto.application_management_search'
              )}
              placeholder={i18nText(
                'settings',
                'auto.application_management_search'
              )}
              value={keywordDraft}
              style={{ width: 240 }}
              onChange={(event) => setKeywordDraft(event.target.value)}
              onSearch={(keyword) =>
                updateRouteState({
                  page: 1,
                  keyword: keyword.trim() || undefined
                })
              }
            />
            <Select
              aria-label={i18nText('settings', 'auto.application_management_sort')}
              value={routeState.sort}
              style={{ width: 180 }}
              options={[
                {
                  value: 'updated_at:desc',
                  label: i18nText(
                    'settings',
                    'auto.application_management_sort_updated_desc'
                  )
                },
                {
                  value: 'created_at:desc',
                  label: i18nText(
                    'settings',
                    'auto.application_management_sort_created_desc'
                  )
                },
                {
                  value: 'name:asc',
                  label: i18nText(
                    'settings',
                    'auto.application_management_sort_name_asc'
                  )
                }
              ]}
              onChange={(sort) => updateRouteState({ page: 1, sort })}
            />
          </Flex>
          <Space>
            <Button onClick={() => applicationsQuery.refetch()}>
              {i18nText('settings', 'auto.refresh')}
            </Button>
            <DataTableColumnSettings
              columns={columns}
              configuration={tableConfiguration}
            />
          </Space>
        </Flex>
      }
    >
      {messageContextHolder}
      {modalContextHolder}
      <div className="application-management-panel__table-region">
        <DataTable<SettingsApplicationManagementItem>
          columns={columns}
          configuration={tableConfiguration}
          dataSource={applicationsQuery.data?.items ?? []}
          loading={applicationsQuery.isLoading || applicationsQuery.isFetching}
          page={routeState.page}
          pageSize={PAGE_SIZE}
          rowKey="id"
          total={applicationsQuery.data?.total ?? 0}
          onPageChange={(page) => updateRouteState({ page })}
        />
      </div>

      {editingApplication ? (
        <ApplicationEditModal
          open
          application={editingApplication}
          saving={updateMutation.isPending}
          onCancel={() => setEditingApplication(null)}
          onSubmit={(values) => {
            updateMutation.mutate(
              {
                application: editingApplication,
                name: values.name,
                description: values.description,
                tagIds: editingApplication.tags.map((tag) => tag.id)
              },
              { onSuccess: () => setEditingApplication(null) }
            );
          }}
        />
      ) : null}
      {taggingApplication ? (
        <ApplicationTagManagerModal
          open
          application={taggingApplication}
          catalogTags={catalog.tags}
          saving={updateMutation.isPending}
          creating={createTagMutation.isPending}
          onCancel={() => setTaggingApplication(null)}
          onSubmit={(tagIds) => {
            updateMutation.mutate(
              {
                application: taggingApplication,
                name: taggingApplication.name,
                description: taggingApplication.description,
                tagIds
              },
              { onSuccess: () => setTaggingApplication(null) }
            );
          }}
          onCreateTag={(name) => createTagMutation.mutateAsync(name)}
        />
      ) : null}
    </SettingsSectionSurface>
  );
}
