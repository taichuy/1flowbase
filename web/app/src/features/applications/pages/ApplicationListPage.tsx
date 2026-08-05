import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Button,
  Checkbox,
  Empty,
  Flex,
  Input,
  Menu,
  Modal,
  App,
  Result,
  Select,
  Space,
  Tag,
  Typography,
  type MenuProps
} from 'antd';
import {
  AppstoreAddOutlined,
  AppstoreOutlined,
  BlockOutlined,
  CopyOutlined,
  DeleteOutlined,
  EditOutlined,
  ExportOutlined,
  FileTextOutlined,
  ImportOutlined,
  MoreOutlined,
  RobotOutlined,
  SearchOutlined,
  TagOutlined
} from '@ant-design/icons';
import type { TFunction } from 'i18next';
import type { ChangeEvent, ReactNode } from 'react';
import { useCallback, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { useAuthStore } from '../../../state/auth-store';
import { formatDateTime } from '../../../shared/i18n/format';
import { LoadingState } from '../../../shared/ui/loading-state/LoadingState';
import {
  applicationCatalogQueryKey,
  applicationsQueryKey,
  createApplication,
  createApplicationTag,
  deleteApplication,
  exportApplicationArchive,
  fetchApplicationCatalog,
  fetchApplications,
  importApplicationArchive,
  previewApplicationArchive,
  type Application,
  type ApplicationTagCatalogEntry,
  updateApplication
} from '../api/applications';
import { ApplicationFormModal } from '../components/ApplicationFormModal';
import { ApplicationTagManagerModal } from '../components/ApplicationTagManagerModal';
import { ApplicationTemplateImportModal } from '../components/ApplicationTemplateImportModal';
import { InstalledAgentFlowImportFlow } from '../components/InstalledAgentFlowImportFlow';
import { InstalledAgentFlowPickerDrawer } from '../components/InstalledAgentFlowPickerDrawer';
import { downloadApplicationArchive } from '../lib/template-download';

type ApplicationTypeFilter = 'all' | Application['application_type'];

interface ApplicationTypeTab {
  key: ApplicationTypeFilter;
  label: string;
  icon: ReactNode;
}

function applicationTypeIcon(applicationType: Application['application_type']) {
  if (applicationType === 'workflow') {
    return <BlockOutlined />;
  }

  return <RobotOutlined />;
}

function mergeTagCatalog(
  currentTags: ApplicationTagCatalogEntry[],
  optimisticTags: ApplicationTagCatalogEntry[]
) {
  const merged = new Map<string, ApplicationTagCatalogEntry>();
  for (const tag of currentTags) {
    merged.set(tag.id, tag);
  }
  for (const tag of optimisticTags) {
    if (!merged.has(tag.id)) {
      merged.set(tag.id, tag);
    }
  }

  return Array.from(merged.values()).sort((left, right) =>
    left.name.localeCompare(right.name)
  );
}

function buildCopiedApplicationName(
  name: string,
  t: TFunction<'applications'>
) {
  return t('auto.copied_application_name', { value1: name });
}

function toApplicationTypeTabs(
  types: Array<{ value: Application['application_type']; label: string }>,
  t: TFunction<'applications'>
): ApplicationTypeTab[] {
  return [
    { key: 'all', label: t('auto.all'), icon: <AppstoreOutlined /> },
    ...types.map((type) => ({
      key: type.value,
      label: type.label,
      icon: applicationTypeIcon(type.value)
    }))
  ];
}

export function ApplicationListPage() {
  const { t } = useTranslation('applications');
  const actor = useAuthStore((state) => state.actor);
  const me = useAuthStore((state) => state.me);
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const queryClient = useQueryClient();
  const { message: messageApi } = App.useApp();
  const [modalApi, modalContextHolder] = Modal.useModal();
  const importFileInputRef = useRef<HTMLInputElement | null>(null);

  const [keyword, setKeyword] = useState('');
  const [typeFilter, setTypeFilter] = useState<ApplicationTypeFilter>('all');
  const [tagFilter, setTagFilter] = useState<string | undefined>(undefined);
  const [createOpen, setCreateOpen] = useState(false);
  const [templatePickerOpen, setTemplatePickerOpen] = useState(false);
  const [installedTemplateId, setInstalledTemplateId] = useState<string | null>(
    null
  );
  const [myCreated, setMyCreated] = useState(false);
  const [editingApplicationId, setEditingApplicationId] = useState<
    string | null
  >(null);
  const [taggingApplicationId, setTaggingApplicationId] = useState<
    string | null
  >(null);
  const [openActionApplicationId, setOpenActionApplicationId] = useState<
    string | null
  >(null);
  const [optimisticTags, setOptimisticTags] = useState<
    ApplicationTagCatalogEntry[]
  >([]);
  const [importArchive, setImportArchive] = useState<File | null>(null);
  const [importName, setImportName] = useState('');
  const [importPreview, setImportPreview] = useState<Awaited<
    ReturnType<typeof previewApplicationArchive>
  > | null>(null);

  const applicationsQuery = useQuery({
    queryKey: applicationsQueryKey,
    queryFn: fetchApplications
  });
  const applicationCatalogQuery = useQuery({
    queryKey: applicationCatalogQueryKey,
    queryFn: fetchApplicationCatalog
  });

  const duplicateApplicationMutation = useMutation({
    mutationFn: (application: Application) =>
      createApplication(
        {
          application_type: application.application_type,
          name: buildCopiedApplicationName(application.name, t),
          description: application.description,
          icon: application.icon,
          icon_type: application.icon_type,
          icon_background: application.icon_background
        },
        csrfToken ?? ''
      ),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: applicationsQueryKey });
      messageApi.success(t('auto.application_copied'));
    },
    onError: () => {
      messageApi.error(t('auto.copy_application_failed'));
    }
  });

  const exportTemplateMutation = useMutation({
    mutationFn: (applicationId: string) =>
      exportApplicationArchive([applicationId]),
    onSuccess: (archive) => {
      downloadApplicationArchive(archive);
      messageApi.success(t('auto.template_exported'));
    },
    onError: () => {
      messageApi.error(t('auto.template_export_failed'));
    }
  });

  const previewTemplateMutation = useMutation({
    mutationFn: (archive: File) => previewApplicationArchive(archive),
    onSuccess: (preview) => {
      setImportPreview(preview);
      setImportName(preview.application.name);
    },
    onError: () => {
      setImportArchive(null);
      setImportPreview(null);
      messageApi.error(t('auto.template_preview_failed'));
    }
  });

  const importTemplateMutation = useMutation({
    mutationFn: () => {
      if (!importArchive) {
        throw new Error('missing application archive');
      }

      return importApplicationArchive(
        importArchive,
        {
          name: importName.trim(),
          description: importPreview?.application.description
        },
        csrfToken ?? ''
      );
    },
    onSuccess: async (imported) => {
      await queryClient.invalidateQueries({ queryKey: applicationsQueryKey });
      messageApi.success(t('auto.template_imported'));
      window.location.assign(
        `/applications/${imported.application.id}/orchestration`
      );
    },
    onError: () => {
      messageApi.error(t('auto.template_import_failed'));
    }
  });

  const updateApplicationMutation = useMutation({
    mutationFn: ({
      applicationId,
      input
    }: {
      applicationId: string;
      input: { name: string; description: string; tag_ids: string[] };
    }) => updateApplication(applicationId, input, csrfToken ?? ''),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: applicationsQueryKey }),
        queryClient.invalidateQueries({ queryKey: applicationCatalogQueryKey })
      ]);
      setOptimisticTags([]);
    }
  });

  const deleteApplicationMutation = useMutation({
    mutationFn: (application: Application) =>
      deleteApplication(application.id, csrfToken ?? ''),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: applicationsQueryKey }),
        queryClient.invalidateQueries({ queryKey: applicationCatalogQueryKey })
      ]);
      messageApi.success(t('auto.application_deleted'));
    },
    onError: () => {
      messageApi.error(t('auto.delete_application_failed'));
    }
  });

  const createApplicationTagMutation = useMutation({
    mutationFn: (input: { name: string }) =>
      createApplicationTag(input, csrfToken ?? ''),
    onSuccess: async (createdTag) => {
      setOptimisticTags((current) =>
        current.some((tag) => tag.id === createdTag.id)
          ? current
          : [...current, createdTag]
      );
      await queryClient.invalidateQueries({
        queryKey: applicationCatalogQueryKey
      });
    }
  });

  const isRoot = actor?.effective_display_role === 'root';
  const canCreate =
    isRoot || Boolean(me?.permissions.includes('application.create.all'));
  const canEditAny =
    isRoot || Boolean(me?.permissions.includes('application.edit.all'));
  const canEditOwn = Boolean(me?.permissions.includes('application.edit.own'));
  const canDeleteAny =
    isRoot || Boolean(me?.permissions.includes('application.delete.all'));
  const canDeleteOwn = Boolean(
    me?.permissions.includes('application.delete.own')
  );
  const normalizedKeyword = keyword.trim().toLowerCase();
  const closeInstalledTemplateImport = useCallback(
    () => setInstalledTemplateId(null),
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

  if (applicationsQuery.isPending || applicationCatalogQuery.isPending) {
    return <LoadingState />;
  }

  if (applicationsQuery.isError || applicationCatalogQuery.isError) {
    return (
      <Result status="error" title={t('auto.application_list_load_failed')} />
    );
  }

  const applications = applicationsQuery.data ?? [];
  const catalog = applicationCatalogQuery.data ?? {
    types: [],
    workflow_triggers: [],
    tags: []
  };
  const availableTags = mergeTagCatalog(catalog.tags, optimisticTags);
  const typeTabs = toApplicationTypeTabs(catalog.types, t);
  const typeLabels = new Map(
    catalog.types.map((type) => [type.value, type.label] as const)
  );
  const taggingApplication =
    applications.find(
      (application) => application.id === taggingApplicationId
    ) ?? null;

  const visibleApplications = applications.filter((application) => {
    const matchesType =
      typeFilter === 'all' || application.application_type === typeFilter;
    const matchesKeyword =
      normalizedKeyword.length === 0 ||
      application.name.toLowerCase().includes(normalizedKeyword) ||
      application.description.toLowerCase().includes(normalizedKeyword);
    const matchesTag =
      !tagFilter || application.tags.some((tag) => tag.id === tagFilter);
    const matchesCreatedBy = !myCreated || application.created_by === actor?.id;

    return matchesType && matchesKeyword && matchesTag && matchesCreatedBy;
  });

  const canEditApplication = (application: Application) =>
    canEditAny || (canEditOwn && application.created_by === actor?.id);

  const canDeleteApplication = (application: Application) =>
    canDeleteAny || (canDeleteOwn && application.created_by === actor?.id);

  const confirmDeleteApplication = (application: Application) => {
    modalApi.confirm({
      title: t('auto.delete_application'),
      content:
        t('auto.delete_application_content_prefix') +
        application.name +
        t('auto.delete_application_content_suffix'),
      okText: t('auto.delete'),
      okButtonProps: { danger: true },
      cancelText: t('auto.cancel'),
      onOk: () => deleteApplicationMutation.mutateAsync(application)
    });
  };

  const handleImportTemplateFile = (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    event.target.value = '';

    if (!file) {
      return;
    }

    setImportArchive(file);
    previewTemplateMutation.mutate(file);
  };

  const handleUpdateApplication = async (
    application: Application,
    input: { name: string; description: string; tag_ids: string[] }
  ) => {
    await updateApplicationMutation.mutateAsync({
      applicationId: application.id,
      input
    });
  };
  return (
    <div
      style={{
        padding: '24px 0',
        width: '100%',
        maxWidth: 1240,
        margin: '0 auto'
      }}
    >
      {modalContextHolder}
      <input
        ref={importFileInputRef}
        type="file"
        accept="application/zip,.zip,application/json,.json"
        aria-label={t('auto.import_template_file')}
        style={{ display: 'none' }}
        onChange={handleImportTemplateFile}
      />
      <Flex
        justify="space-between"
        align="center"
        wrap="wrap"
        gap={16}
        style={{ marginBottom: 24 }}
      >
        <Space size="small" wrap>
          {typeTabs.map((tab) => (
            <Button
              key={tab.key}
              type={typeFilter === tab.key ? 'primary' : 'default'}
              icon={tab.icon}
              aria-label={tab.label}
              onClick={() => setTypeFilter(tab.key)}
            >
              {tab.label}
            </Button>
          ))}
        </Space>

        <Space size="middle" wrap>
          <Checkbox
            checked={myCreated}
            onChange={(event) => setMyCreated(event.target.checked)}
          >
            {t('auto.created_by_me')}
          </Checkbox>
          <Select
            allowClear
            value={tagFilter}
            placeholder={t('auto.all_tags')}
            options={availableTags.map((tag) => ({
              value: tag.id,
              label: `${tag.name} (${tag.application_count})`
            }))}
            style={{ width: 180 }}
            suffixIcon={<TagOutlined />}
            onChange={(value) => setTagFilter(value)}
          />
          <Input
            value={keyword}
            prefix={<SearchOutlined style={{ color: '#94a3b8' }} />}
            placeholder={t('auto.search_applications')}
            style={{ width: 220, borderRadius: 8 }}
            onChange={(event) => setKeyword(event.target.value)}
          />
        </Space>
      </Flex>

      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fill, minmax(300px, 1fr))',
          gap: 16
        }}
      >
        {canCreate && (
          <div
            style={{
              background:
                'linear-gradient(180deg, rgba(248, 250, 252, 0.5) 0%, #7affc62e 100%)',
              borderRadius: 18,
              padding: 20,
              display: 'flex',
              flexDirection: 'column',
              gap: 12,
              border: '1px solid #dbe7f3'
            }}
          >
            <Typography.Text style={{ color: '#64748b', fontSize: 13 }}>
              {t('auto.create_application')}
            </Typography.Text>
            <Button
              type="text"
              icon={<AppstoreAddOutlined />}
              style={{ justifyContent: 'flex-start' }}
              onClick={() => setCreateOpen(true)}
            >
              {t('auto.create_blank_application')}
            </Button>
            <Button
              type="text"
              icon={<FileTextOutlined />}
              aria-label={t('auto.create_from_application_template')}
              style={{ justifyContent: 'flex-start' }}
              onClick={() => setTemplatePickerOpen(true)}
            >
              {t('auto.create_from_application_template')}
            </Button>
            <Button
              type="text"
              icon={<ImportOutlined />}
              style={{ justifyContent: 'flex-start' }}
              loading={previewTemplateMutation.isPending}
              onClick={() => importFileInputRef.current?.click()}
            >
              {t('auto.import_template_file')}
            </Button>
          </div>
        )}

        {visibleApplications.map((application) => {
          const canEdit = canEditApplication(application);
          const canDelete = canDeleteApplication(application);
          const typeLabel =
            typeLabels.get(application.application_type) ??
            application.application_type;
          const applicationHref = `/applications/${application.id}/orchestration`;
          const actionItems: MenuProps['items'] = [
            {
              key: 'export_template',
              icon: <ExportOutlined />,
              label: t('auto.export_template'),
              disabled: exportTemplateMutation.isPending
            },
            {
              key: 'copy',
              icon: <CopyOutlined />,
              label: t('auto.copy'),
              disabled: !canCreate || duplicateApplicationMutation.isPending
            },
            {
              key: 'edit',
              icon: <EditOutlined />,
              label: t('auto.edit_information'),
              disabled: !canEdit
            },
            {
              key: 'delete',
              icon: <DeleteOutlined />,
              label: t('auto.delete'),
              danger: true,
              disabled: !canDelete || deleteApplicationMutation.isPending
            }
          ];

          return (
            <div
              key={application.id}
              style={{
                position: 'relative',
                background: '#ffffff',
                borderRadius: 18,
                padding: 18,
                border: '1px solid #e2e8f0',
                display: 'flex',
                flexDirection: 'column',
                minHeight: 250,
                boxShadow: '0 12px 32px rgba(15, 23, 42, 0.06)'
              }}
            >
              <a
                href={applicationHref}
                aria-label={t('auto.enter_application_named', {
                  value1: application.name
                })}
                style={{
                  position: 'absolute',
                  inset: 0,
                  zIndex: 1,
                  borderRadius: 18
                }}
              />
              <div
                style={{
                  position: 'relative',
                  zIndex: 2,
                  pointerEvents: 'none'
                }}
              >
                <Flex align="flex-start" gap={12} style={{ marginBottom: 16 }}>
                  <div
                    style={{
                      width: 44,
                      height: 44,
                      borderRadius: 12,
                      background: '#eef6ff',
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                      fontSize: 20,
                      color: '#2563eb'
                    }}
                  >
                    {applicationTypeIcon(application.application_type)}
                  </div>
                  <div style={{ flex: 1 }}>
                    <Typography.Title
                      level={5}
                      style={{ margin: 0, color: '#0f172a' }}
                    >
                      {application.name}
                    </Typography.Title>
                    <Typography.Text type="secondary">
                      {typeLabel} {t('auto.edited_at_prefix')}{' '}
                      {formatDateTime(application.updated_at, {
                        year: 'numeric',
                        month: '2-digit',
                        day: '2-digit',
                        hour: '2-digit',
                        minute: '2-digit'
                      })}
                    </Typography.Text>
                  </div>
                </Flex>

                <Typography.Paragraph
                  style={{ color: '#334155', minHeight: 44 }}
                >
                  {application.description ||
                    t('auto.application_description_empty')}
                </Typography.Paragraph>

                <Flex wrap gap={8} style={{ minHeight: 32, marginBottom: 16 }}>
                  {application.tags.length === 0 ? (
                    <Tag variant="filled" color="default">
                      {t('auto.no_tags')}
                    </Tag>
                  ) : (
                    application.tags.map((tag) => (
                      <Tag key={tag.id} variant="filled" color="blue">
                        {tag.name}
                      </Tag>
                    ))
                  )}
                </Flex>
              </div>

              <Flex
                justify="space-between"
                align="center"
                style={{ position: 'relative', zIndex: 3, marginTop: 'auto' }}
              >
                <Space size="small" wrap>
                  <Button
                    size="small"
                    icon={<TagOutlined />}
                    aria-label={t('auto.manage_tags_named', {
                      value1: application.name
                    })}
                    onClick={() => setTaggingApplicationId(application.id)}
                    disabled={!canEdit}
                  >
                    {t('auto.manage_tags')}
                  </Button>
                </Space>
                <div style={{ position: 'relative' }}>
                  <Button
                    type="text"
                    icon={<MoreOutlined />}
                    aria-label={t('auto.more_actions_named', {
                      value1: application.name
                    })}
                    aria-expanded={openActionApplicationId === application.id}
                    onMouseDown={(event) => {
                      event.preventDefault();
                      setOpenActionApplicationId((current) =>
                        current === application.id ? null : application.id
                      );
                    }}
                    onKeyDown={(event) => {
                      if (event.key !== 'Enter' && event.key !== ' ') {
                        return;
                      }
                      event.preventDefault();
                      setOpenActionApplicationId((current) =>
                        current === application.id ? null : application.id
                      );
                    }}
                    style={{
                      width: 40,
                      height: 40,
                      borderRadius: 8,
                      background: 'transparent'
                    }}
                  />
                  {openActionApplicationId === application.id ? (
                    <div
                      style={{
                        position: 'absolute',
                        top: 44,
                        right: 0,
                        zIndex: 10,
                        width: 180,
                        overflow: 'hidden',
                        background: '#ffffff',
                        border: '1px solid #e2e8f0',
                        borderRadius: 8,
                        boxShadow: '0 16px 36px rgba(15, 23, 42, 0.12)'
                      }}
                    >
                      <Menu
                        selectable={false}
                        items={actionItems}
                        onClick={({ key }) => {
                          if (key === 'export_template') {
                            exportTemplateMutation.mutate(application.id);
                          }
                          if (key === 'copy' && canCreate) {
                            duplicateApplicationMutation.mutate(application);
                          }
                          if (key === 'edit' && canEdit) {
                            setEditingApplicationId(application.id);
                          }
                          if (key === 'delete' && canDelete) {
                            confirmDeleteApplication(application);
                          }
                          setOpenActionApplicationId(null);
                        }}
                      />
                    </div>
                  ) : null}
                </div>
              </Flex>
            </div>
          );
        })}
      </div>

      {visibleApplications.length === 0 ? (
        <div style={{ marginTop: 24 }}>
          <Empty description={t('auto.no_applications_for_filter')} />
        </div>
      ) : null}

      <ApplicationFormModal
        open={Boolean(editingApplicationId)}
        csrfToken={csrfToken ?? ''}
        intent={{
          kind: 'edit',
          applicationId: editingApplicationId ?? ''
        }}
        onClose={() => setEditingApplicationId(null)}
      />

      <ApplicationTagManagerModal
        open={Boolean(taggingApplication)}
        application={taggingApplication}
        catalogTags={availableTags}
        saving={updateApplicationMutation.isPending}
        creating={createApplicationTagMutation.isPending}
        onCancel={() => setTaggingApplicationId(null)}
        onCreateTag={async (name) => {
          const createdTag = await createApplicationTagMutation.mutateAsync({
            name
          });
          return { id: createdTag.id, name: createdTag.name };
        }}
        onSubmit={(tagIds) => {
          if (!taggingApplication) {
            return;
          }

          void handleUpdateApplication(taggingApplication, {
            name: taggingApplication.name,
            description: taggingApplication.description,
            tag_ids: tagIds
          }).then(() => setTaggingApplicationId(null));
        }}
      />

      <ApplicationFormModal
        open={createOpen}
        csrfToken={csrfToken ?? ''}
        onClose={() => setCreateOpen(false)}
        intent={{
          kind: 'create',
          onCreated: (applicationId) => {
            window.location.assign(
              `/applications/${applicationId}/orchestration`
            );
          }
        }}
      />

      <ApplicationTemplateImportModal
        open={Boolean(importPreview)}
        preview={importPreview}
        name={importName}
        importing={importTemplateMutation.isPending}
        onNameChange={setImportName}
        onCancel={() => {
          setImportArchive(null);
          setImportPreview(null);
        }}
        onImport={() => importTemplateMutation.mutate()}
      />
      <InstalledAgentFlowPickerDrawer
        open={templatePickerOpen}
        onClose={() => setTemplatePickerOpen(false)}
        onSelect={(installationId) => {
          setTemplatePickerOpen(false);
          setInstalledTemplateId(installationId);
        }}
      />
      <InstalledAgentFlowImportFlow
        installationId={installedTemplateId}
        csrfToken={csrfToken ?? ''}
        onClose={closeInstalledTemplateImport}
        onImported={finishInstalledTemplateImport}
      />
    </div>
  );
}
