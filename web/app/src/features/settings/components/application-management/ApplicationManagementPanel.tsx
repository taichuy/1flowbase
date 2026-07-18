import {
  CopyOutlined,
  DeleteOutlined,
  EditOutlined,
  ExportOutlined,
  MoreOutlined
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
  Switch,
  Tag,
  Tooltip,
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
  deleteApplication,
  exportAgentFlowTemplate,
  fetchApplicationCatalog,
  updateApplication
} from '../../../applications/api/applications';
import {
  fetchApplicationApiMapping,
  publishApplicationApiVersion,
  saveWorkflowScheduleTrigger,
  unpublishApplicationApiVersion
} from '../../../applications/api/public-api';
import { downloadTemplateFile } from '../../../applications/lib/template-download';
import { SettingsSectionSurface } from '../SettingsSectionSurface';
import {
  fetchSettingsApplicationManagement,
  fetchAllSettingsApplicationManagement,
  settingsApplicationManagementQueryKey,
  settingsApplicationManagementQueryPrefix,
  type SettingsApplicationManagementItem,
  type SettingsApplicationManagementQuery
} from '../../api/application-management';
import { fetchSettingsMembers, settingsMembersQueryKey } from '../../api/members';
import {
  ApplicationDetailsDrawer,
  type ApplicationDetailsValues
} from './ApplicationDetailsDrawer';
import {
  pushApplicationManagementRouteState,
  readApplicationManagementRouteState,
  type ApplicationManagementRouteState
} from './application-management-route-state';
import {
  buildApplicationManagementCsv,
  downloadApplicationManagementCsv
} from './application-management-export';
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
  const [detailsApplication, setDetailsApplication] =
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
  const membersQuery = useQuery({
    queryKey: settingsMembersQueryKey,
    queryFn: fetchSettingsMembers,
    retry: false
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
    mutationFn: async ({
      application,
      values
    }: {
      application: SettingsApplicationManagementItem;
      values: ApplicationDetailsValues;
    }) => {
      const updated = await updateApplication(
        application.id,
        {
          name: values.name,
          description: values.description,
          tag_ids: values.tag_ids,
          icon: values.icon.trim(),
          icon_type: values.icon_type.trim(),
          icon_background: values.icon_background.trim()
        },
        csrfToken
      );
      if (application.workflow_trigger_type === 'schedule') {
        await saveWorkflowScheduleTrigger(
          application.id,
          {
            enabled: Boolean(values.schedule_enabled),
            cron: values.schedule_cron?.trim() ?? '',
            timezone: values.schedule_timezone?.trim() ?? '',
            input_payload: values.schedule_input_payload ?? {}
          },
          csrfToken
        );
      }
      return updated;
    },
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
  const revertToDraftMutation = useMutation({
    mutationFn: (applicationId: string) =>
      unpublishApplicationApiVersion(applicationId, csrfToken),
    onSuccess: async () => {
      await invalidateApplications();
      messageApi.success(
        i18nText('applications', 'auto.revert_to_draft_success')
      );
    },
    onError: () =>
      messageApi.error(i18nText('applications', 'auto.revert_to_draft_failed'))
  });
  const publishMutation = useMutation({
    mutationFn: async (applicationId: string) => {
      const mapping = await fetchApplicationApiMapping(applicationId);
      return publishApplicationApiVersion(applicationId, mapping, csrfToken);
    },
    onSuccess: async () => {
      await invalidateApplications();
      messageApi.success(i18nText('agentFlow', 'auto.posted_successfully'));
    },
    onError: () =>
      messageApi.error(i18nText('agentFlow', 'auto.publishing_failed'))
  });
  const exportCsvMutation = useMutation({
    mutationFn: () =>
      fetchAllSettingsApplicationManagement({
        filter: managementQuery.filter,
        sort: managementQuery.sort
      }),
    onSuccess: (applications) => {
      downloadApplicationManagementCsv(
        buildApplicationManagementCsv(applications)
      );
    },
    onError: () =>
      messageApi.error(
        i18nText(
          'settingsApplicationManagement',
          'auto.application_management_export_failed'
        )
      )
  });
  const copyApplication = copyMutation.mutate;
  const exportApplication = exportMutation.mutate;
  const deleteApplicationById = deleteMutation.mutateAsync;
  const revertToDraft = revertToDraftMutation.mutateAsync;
  const publishApplication = publishMutation.mutate;
  const publishingApplicationId = publishMutation.isPending
    ? publishMutation.variables
    : null;
  const revertingApplicationId = revertToDraftMutation.isPending
    ? revertToDraftMutation.variables
    : null;

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

  const confirmRevertToDraft = useCallback(
    (application: SettingsApplicationManagementItem) => {
      modalApi.confirm({
        title: i18nText('applications', 'auto.revert_to_draft'),
        content: i18nText('applications', 'auto.revert_to_draft_confirm_content'),
        okText: i18nText('applications', 'auto.revert_to_draft'),
        cancelText: i18nText('applications', 'auto.cancel'),
        onOk: () => revertToDraft(application.id)
      });
    },
    [modalApi, revertToDraft]
  );

  const columns = useMemo<
    Array<DataTableColumn<SettingsApplicationManagementItem>>
  >(
    () => [
      {
        key: 'application',
        title: i18nText(
          'settingsApplicationManagement',
          'auto.application_management_application'
        ),
        width: 260,
        render: (_, application) => (
          <Flex vertical gap={2}>
            <Typography.Text strong>
              {application.name}
            </Typography.Text>
            <Typography.Text type="secondary" ellipsis>
              {application.description ||
                i18nText('applications', 'auto.application_description_empty')}
            </Typography.Text>
          </Flex>
        )
      },
      {
        key: 'application_type',
        title: i18nText(
          'settingsApplicationManagement',
          'auto.application_management_type'
        ),
        width: 120,
        render: (_, application) => (
          <Tag>{applicationTypeLabel(application.application_type)}</Tag>
        )
      },
      {
        key: 'workflow_trigger_type',
        title: i18nText(
          'settingsApplicationManagement',
          'auto.application_management_trigger'
        ),
        width: 120,
        render: (_, application) =>
          triggerTypeLabel(application.workflow_trigger_type)
      },
      {
        key: 'publication_status',
        title: i18nText(
          'settingsApplicationManagement',
          'auto.application_management_publication_status'
        ),
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
        title: i18nText(
          'settingsApplicationManagement',
          'auto.application_management_creator'
        ),
        dataIndex: 'created_by_display_name',
        width: 150
      },
      {
        key: 'tags',
        title: i18nText(
          'settingsApplicationManagement',
          'auto.application_management_tags'
        ),
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
        title: i18nText(
          'settingsApplicationManagement',
          'auto.application_management_actions'
        ),
        width: 210,
        render: (_, application) => {
          const editAllowed = canEdit(application);
          const deleteAllowed = canDelete(application);
          const items: MenuProps['items'] = [
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
            <Space size={8} onClick={(event) => event.stopPropagation()}>
              <Tooltip
                title={
                  editAllowed
                    ? undefined
                    : i18nText(
                        'settingsApplicationManagement',
                        'auto.application_management_permission_required'
                      )
                }
              >
                <span>
                  <Button
                    icon={<EditOutlined />}
                    disabled={!editAllowed}
                    onClick={() => setDetailsApplication(application)}
                  >
                    {i18nText('applications', 'auto.edit_information')}
                  </Button>
                </span>
              </Tooltip>
              <Tooltip
                title={i18nText(
                  'settingsApplicationManagement',
                  'auto.application_management_publication_status'
                )}
              >
                <Switch
                  checked={application.publication_status === 'published'}
                  disabled={!editAllowed}
                  loading={
                    publishingApplicationId === application.id ||
                    revertingApplicationId === application.id
                  }
                  onChange={(published) => {
                    if (published) {
                      publishApplication(application.id);
                    } else {
                      confirmRevertToDraft(application);
                    }
                  }}
                />
              </Tooltip>
              <Dropdown
                menu={{
                  items,
                  onClick: ({ key }) => {
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
            </Space>
          );
        }
      }
    ],
    [
      canCreate,
      canDelete,
      canEdit,
      confirmDelete,
      confirmRevertToDraft,
      copyApplication,
      exportApplication,
      publishApplication,
      publishingApplicationId,
      revertingApplicationId
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
              aria-label={i18nText(
                'settingsApplicationManagement',
                'auto.application_management_type'
              )}
              placeholder={i18nText(
                'settingsApplicationManagement',
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
                'settingsApplicationManagement',
                'auto.application_management_publication_status'
              )}
              placeholder={i18nText(
                'settingsApplicationManagement',
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
            <Select
              allowClear
              showSearch
              optionFilterProp="label"
              aria-label={i18nText(
                'settingsApplicationManagement',
                'auto.application_management_creator'
              )}
              placeholder={i18nText(
                'settingsApplicationManagement',
                'auto.application_management_all_creators'
              )}
              value={routeState.created_by}
              options={(membersQuery.data ?? []).map((member) => ({
                value: member.id,
                label: member.name || member.nickname || member.account
              }))}
              style={{ width: 190 }}
              onChange={(created_by) =>
                updateRouteState({
                  page: 1,
                  created_by
                })
              }
            />
            <Select
              allowClear
              aria-label={i18nText(
                'settingsApplicationManagement',
                'auto.application_management_tags'
              )}
              placeholder={i18nText(
                'settingsApplicationManagement',
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
                'settingsApplicationManagement',
                'auto.application_management_search'
              )}
              placeholder={i18nText(
                'settingsApplicationManagement',
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
              aria-label={i18nText(
                'settingsApplicationManagement',
                'auto.application_management_sort'
              )}
              value={routeState.sort}
              style={{ width: 180 }}
              options={[
                {
                  value: 'updated_at:desc',
                  label: i18nText(
                    'settingsApplicationManagement',
                    'auto.application_management_sort_updated_desc'
                  )
                },
                {
                  value: 'created_at:desc',
                  label: i18nText(
                    'settingsApplicationManagement',
                    'auto.application_management_sort_created_desc'
                  )
                },
                {
                  value: 'name:asc',
                  label: i18nText(
                    'settingsApplicationManagement',
                    'auto.application_management_sort_name_asc'
                  )
                }
              ]}
              onChange={(sort) => updateRouteState({ page: 1, sort })}
            />
          </Flex>
          <Space>
            <Button
              icon={<ExportOutlined />}
              loading={exportCsvMutation.isPending}
              onClick={() => exportCsvMutation.mutate()}
            >
              {i18nText(
                'settingsApplicationManagement',
                'auto.application_management_export_csv'
              )}
            </Button>
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
          onRow={(application) => ({
            className: 'application-management-panel__row',
            onClick: (event) => {
              const target = event.target as HTMLElement;
              if (target.closest('button, a, input, [role="switch"]')) {
                return;
              }
              setDetailsApplication(application);
            }
          })}
          onPageChange={(page) => updateRouteState({ page })}
        />
      </div>

      <ApplicationDetailsDrawer
        application={detailsApplication}
        catalogTags={catalog.tags}
        saving={updateMutation.isPending}
        onClose={() => setDetailsApplication(null)}
        onSubmit={(values) => {
          if (!detailsApplication) return;
          updateMutation.mutate(
            { application: detailsApplication, values },
            { onSuccess: () => setDetailsApplication(null) }
          );
        }}
      />
    </SettingsSectionSurface>
  );
}
