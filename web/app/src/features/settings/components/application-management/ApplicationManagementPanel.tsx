import {
  CopyOutlined,
  DeleteOutlined,
  DownOutlined,
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
  fetchApplicationCatalog
} from '../../../applications/api/applications';
import {
  fetchApplicationApiMapping,
  publishApplicationApiVersion,
  unpublishApplicationApiVersion
} from '../../../applications/api/public-api';
import { ApplicationFormModal } from '../../../applications/components/ApplicationFormModal';
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
import {
  fetchSettingsMembers,
  settingsMembersQueryKey
} from '../../api/members';
import {
  APPLICATION_MANAGEMENT_DEFAULT_SORT,
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
  const [filterDraft, setFilterDraft] = useState(routeState);
  const [selectedApplicationIds, setSelectedApplicationIds] = useState<
    string[]
  >([]);
  const [detailsApplication, setDetailsApplication] =
    useState<SettingsApplicationManagementItem | null>(null);

  useEffect(() => {
    const handlePopState = () => {
      const nextState = readApplicationManagementRouteState();
      setRouteState(nextState);
      setFilterDraft(nextState);
      setSelectedApplicationIds([]);
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
  const applyFilters = useCallback(() => {
    const nextState = {
      ...filterDraft,
      page: 1,
      keyword: filterDraft.keyword?.trim() || undefined
    };
    pushApplicationManagementRouteState(nextState);
    setRouteState(nextState);
    setFilterDraft(nextState);
    setSelectedApplicationIds([]);
  }, [filterDraft]);
  const resetFilters = useCallback(() => {
    const nextState: ApplicationManagementRouteState = {
      page: 1,
      sort: APPLICATION_MANAGEMENT_DEFAULT_SORT
    };
    pushApplicationManagementRouteState(nextState);
    setRouteState(nextState);
    setFilterDraft(nextState);
    setSelectedApplicationIds([]);
  }, []);
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
  const deleteMutation = useMutation({
    mutationFn: (applicationId: string) =>
      deleteApplication(applicationId, csrfToken),
    onSuccess: async () => {
      await invalidateApplications();
      messageApi.success(i18nText('applications', 'auto.application_deleted'));
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
      messageApi.error(i18nText('applications', 'auto.copy_application_failed'))
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
  const exportFilteredCsvMutation = useMutation({
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
  const selectedApplications = useMemo(() => {
    const selectedIds = new Set(selectedApplicationIds);
    return (applicationsQuery.data?.items ?? []).filter((application) =>
      selectedIds.has(application.id)
    );
  }, [applicationsQuery.data?.items, selectedApplicationIds]);
  const exportSelectedApplications = useCallback(() => {
    if (selectedApplications.length === 0) {
      return;
    }
    downloadApplicationManagementCsv(
      buildApplicationManagementCsv(selectedApplications)
    );
  }, [selectedApplications]);
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
        content: i18nText(
          'applications',
          'auto.revert_to_draft_confirm_content'
        ),
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
            <Typography.Text strong>{application.name}</Typography.Text>
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
        key: 'publication_control',
        title: i18nText(
          'settingsApplicationManagement',
          'auto.application_management_publication_control'
        ),
        width: 100,
        render: (_, application) => {
          const editAllowed = canEdit(application);
          return (
            <Tooltip
              title={i18nText(
                'settingsApplicationManagement',
                'auto.application_management_publication_control'
              )}
            >
              <Switch
                checked={application.publication_status === 'published'}
                disabled={!editAllowed}
                loading={
                  publishingApplicationId === application.id ||
                  revertingApplicationId === application.id
                }
                onClick={(_, event) => event.stopPropagation()}
                onChange={(published) => {
                  if (published) {
                    publishApplication(application.id);
                  } else {
                    confirmRevertToDraft(application);
                  }
                }}
              />
            </Tooltip>
          );
        }
      },
      {
        key: 'actions',
        title: i18nText(
          'settingsApplicationManagement',
          'auto.application_management_actions'
        ),
        width: 140,
        align: 'right',
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
                    disabled={!editAllowed}
                    onClick={() => setDetailsApplication(application)}
                  >
                    {i18nText('settings', 'auto.edit')}
                  </Button>
                </span>
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
        <form
          className="application-management-panel__filter-form"
          onSubmit={(event) => {
            event.preventDefault();
            applyFilters();
          }}
        >
          <label className="application-management-panel__filter-field">
            <span className="application-management-panel__filter-label">
              {i18nText(
                'settingsApplicationManagement',
                'auto.application_management_type'
              )}
            </span>
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
              value={filterDraft.application_type}
              options={catalog.types}
              onChange={(application_type) =>
                setFilterDraft((current) => ({
                  ...current,
                  application_type
                }))
              }
            />
          </label>
          <label className="application-management-panel__filter-field">
            <span className="application-management-panel__filter-label">
              {i18nText(
                'settingsApplicationManagement',
                'auto.application_management_publication_status'
              )}
            </span>
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
              value={filterDraft.publication_status}
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
              onChange={(publication_status) =>
                setFilterDraft((current) => ({
                  ...current,
                  publication_status
                }))
              }
            />
          </label>
          <label className="application-management-panel__filter-field">
            <span className="application-management-panel__filter-label">
              {i18nText(
                'settingsApplicationManagement',
                'auto.application_management_creator'
              )}
            </span>
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
              value={filterDraft.created_by}
              options={(membersQuery.data ?? []).map((member) => ({
                value: member.id,
                label: member.name || member.nickname || member.account
              }))}
              onChange={(created_by) =>
                setFilterDraft((current) => ({ ...current, created_by }))
              }
            />
          </label>
          <label className="application-management-panel__filter-field">
            <span className="application-management-panel__filter-label">
              {i18nText(
                'settingsApplicationManagement',
                'auto.application_management_tags'
              )}
            </span>
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
              value={filterDraft.tag_id}
              options={catalog.tags.map((tag) => ({
                value: tag.id,
                label: tag.name
              }))}
              onChange={(tag_id) =>
                setFilterDraft((current) => ({ ...current, tag_id }))
              }
            />
          </label>
          <label className="application-management-panel__filter-field">
            <span className="application-management-panel__filter-label">
              {i18nText(
                'settingsApplicationManagement',
                'auto.application_management_keyword'
              )}
            </span>
            <Input
              aria-label={i18nText(
                'settingsApplicationManagement',
                'auto.application_management_search'
              )}
              placeholder={i18nText(
                'settingsApplicationManagement',
                'auto.application_management_search'
              )}
              type="search"
              value={filterDraft.keyword ?? ''}
              onChange={(event) =>
                setFilterDraft((current) => ({
                  ...current,
                  keyword: event.target.value
                }))
              }
            />
          </label>
          <label className="application-management-panel__filter-field">
            <span className="application-management-panel__filter-label">
              {i18nText(
                'settingsApplicationManagement',
                'auto.application_management_sort'
              )}
            </span>
            <Select
              aria-label={i18nText(
                'settingsApplicationManagement',
                'auto.application_management_sort'
              )}
              value={filterDraft.sort}
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
              onChange={(sort) =>
                setFilterDraft((current) => ({ ...current, sort }))
              }
            />
          </label>
          <div className="application-management-panel__filter-actions">
            <Button htmlType="button" onClick={resetFilters}>
              {i18nText(
                'settingsApplicationManagement',
                'auto.application_management_reset_filters'
              )}
            </Button>
            <Button htmlType="submit" type="primary">
              {i18nText(
                'settingsApplicationManagement',
                'auto.application_management_apply_filters'
              )}
            </Button>
          </div>
        </form>
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
          rowSelection={{
            selectedRowKeys: selectedApplicationIds,
            onChange: (keys) =>
              setSelectedApplicationIds(keys.map((key) => String(key)))
          }}
          toolbar={
            <Flex justify="flex-end" gap={8} wrap>
              <Dropdown
                trigger={['click']}
                menu={{
                  items: [
                    {
                      key: 'selected',
                      disabled: selectedApplications.length === 0,
                      label: i18nText(
                        'settingsApplicationManagement',
                        'auto.application_management_export_selected',
                        { value1: selectedApplications.length }
                      ),
                      onClick: exportSelectedApplications
                    },
                    {
                      key: 'filtered',
                      label: i18nText(
                        'settingsApplicationManagement',
                        'auto.application_management_export_filtered'
                      ),
                      onClick: () => exportFilteredCsvMutation.mutate()
                    }
                  ]
                }}
              >
                <Button
                  icon={<ExportOutlined />}
                  loading={exportFilteredCsvMutation.isPending}
                >
                  <Space size={4}>
                    {i18nText(
                      'settingsApplicationManagement',
                      'auto.application_management_export_csv'
                    )}
                    <DownOutlined aria-hidden="true" />
                  </Space>
                </Button>
              </Dropdown>
              <Button onClick={() => applicationsQuery.refetch()}>
                {i18nText('settings', 'auto.refresh')}
              </Button>
              <DataTableColumnSettings
                columns={columns}
                configuration={tableConfiguration}
              />
            </Flex>
          }
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
          onPageChange={(page) => {
            setSelectedApplicationIds([]);
            updateRouteState({ page });
          }}
        />
      </div>

      <ApplicationFormModal
        open={Boolean(detailsApplication)}
        csrfToken={csrfToken}
        intent={{
          kind: 'edit',
          applicationId: detailsApplication?.id ?? '',
          onSaved: () => void invalidateApplications()
        }}
        onClose={() => setDetailsApplication(null)}
      />
    </SettingsSectionSurface>
  );
}
