import { useMemo, useState } from 'react';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate, useRouterState } from '@tanstack/react-router';
import {
  Button,
  Drawer,
  Form,
  Input,
  Modal,
  Select,
  Space,
  Table,
  Tabs,
  Tag,
  Typography,
  message
} from 'antd';
import { useTranslation } from 'react-i18next';

import { useAuthStore } from '../../../../state/auth-store';
import {
  archiveSettingsUiTemplate,
  createSettingsUiTemplate,
  fetchSettingsUiComponents,
  fetchSettingsUiTemplates,
  publishSettingsUiTemplate,
  resetSettingsUiTemplateDefault,
  setSettingsUiTemplateDefault,
  settingsUiComponentsQueryKey,
  settingsUiTemplatesQueryKey,
  updateSettingsUiComponentContract,
  updateSettingsUiComponentState,
  updateSettingsUiTemplate,
  type SettingsUiComponentCandidate,
  type SettingsUiManagedTemplate,
  type SettingsUiTemplateInput
} from '../../api/ui-management';
import { SettingsSectionSurface } from '../SettingsSectionSurface';

type TemplateForm = SettingsUiTemplateInput;

function requireToken(token: string | null): string {
  if (!token) throw new Error('missing csrf token');
  return token;
}

function CodeTemplatesTab({ canManage }: { canManage: boolean }) {
  const { t } = useTranslation('settingsUiManagement');
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const queryClient = useQueryClient();
  const [form] = Form.useForm<TemplateForm>();
  const [editing, setEditing] = useState<SettingsUiManagedTemplate | null>(
    null
  );
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [includeArchived, setIncludeArchived] = useState(false);
  const query = useQuery({
    queryKey: [...settingsUiTemplatesQueryKey, includeArchived],
    queryFn: () => fetchSettingsUiTemplates(includeArchived)
  });
  const refresh = () =>
    queryClient.invalidateQueries({ queryKey: settingsUiTemplatesQueryKey });
  const save = useMutation({
    mutationFn: async (value: TemplateForm) =>
      editing
        ? updateSettingsUiTemplate(
            editing.id,
            {
              name: value.name,
              source: value.source,
              language: value.language
            },
            requireToken(csrfToken)
          )
        : createSettingsUiTemplate(value, requireToken(csrfToken)),
    onSuccess: async () => {
      await refresh();
      setDrawerOpen(false);
      message.success(t('saved'));
    }
  });
  const action = useMutation({
    mutationFn: async (run: () => Promise<unknown>) => run(),
    onSuccess: refresh
  });
  const openCreate = () => {
    setEditing(null);
    form.resetFields();
    form.setFieldsValue({ language: 'tsx' });
    setDrawerOpen(true);
  };
  const openEdit = (row: SettingsUiManagedTemplate) => {
    setEditing(row);
    form.setFieldsValue({
      provider_code: row.provider_code,
      contribution_code: row.contribution_code,
      name: row.name,
      source: row.latest_revision.source,
      language: row.latest_revision.language
    });
    setDrawerOpen(true);
  };
  const rows = useMemo(
    () => [
      ...(query.data?.official.map((row) => ({
        key: `official:${row.provider_code}:${row.contribution_code}`,
        kind: 'official' as const,
        ...row,
        name: row.title,
        revision: row.version,
        status: 'published',
        is_archived: false
      })) ?? []),
      ...(query.data?.managed.map((row) => ({
        key: row.id,
        kind: 'managed' as const,
        ...row,
        revision: `r${row.latest_revision.revision}`,
        status: row.published_revision ? 'published' : 'draft'
      })) ?? [])
    ],
    [query.data]
  );
  return (
    <SettingsSectionSurface
      toolbar={
        <Space wrap>
          <Button type="primary" disabled={!canManage} onClick={openCreate}>
            {t('new_template')}
          </Button>
          <Button onClick={() => setIncludeArchived((v) => !v)}>
            {includeArchived ? t('hide_archived') : t('show_archived')}
          </Button>
        </Space>
      }
    >
      <Table
        loading={query.isLoading}
        rowKey="key"
        scroll={{ x: 900 }}
        dataSource={rows}
        columns={[
          { title: t('name'), dataIndex: 'name' },
          {
            title: t('contribution'),
            render: (_, r) => (
              <Typography.Text code>
                {r.provider_code}/{r.contribution_code}
              </Typography.Text>
            )
          },
          {
            title: t('source'),
            render: (_, r) => (
              <Tag>{r.kind === 'official' ? t('official') : t('managed')}</Tag>
            )
          },
          { title: t('revision'), dataIndex: 'revision' },
          {
            title: t('status'),
            render: (_, r) => (
              <Space>
                <Tag color={r.status === 'published' ? 'green' : 'default'}>
                  {t(r.status)}
                </Tag>
                {r.is_default ? <Tag color="blue">{t('default')}</Tag> : null}
                {r.is_archived ? <Tag>{t('archived')}</Tag> : null}
              </Space>
            )
          },
          {
            title: t('actions'),
            fixed: 'right',
            render: (_, row) => (
              <Space wrap>
                {row.kind === 'managed' ? (
                  <>
                    <Button
                      size="small"
                      disabled={!canManage}
                      onClick={() => openEdit(row)}
                    >
                      {t('edit')}
                    </Button>
                    <Button
                      size="small"
                      disabled={
                        !canManage ||
                        row.latest_revision.is_published ||
                        row.is_archived
                      }
                      onClick={() =>
                        action.mutate(() =>
                          publishSettingsUiTemplate(
                            row.id,
                            row.latest_revision.revision,
                            requireToken(csrfToken)
                          )
                        )
                      }
                    >
                      {t('publish')}
                    </Button>
                    <Button
                      size="small"
                      disabled={
                        !canManage || !row.published_revision || row.is_archived
                      }
                      onClick={() =>
                        action.mutate(() =>
                          setSettingsUiTemplateDefault(
                            row.id,
                            requireToken(csrfToken)
                          )
                        )
                      }
                    >
                      {t('set_default')}
                    </Button>
                    <Button
                      size="small"
                      danger={!row.is_archived}
                      disabled={!canManage}
                      onClick={() =>
                        action.mutate(() =>
                          archiveSettingsUiTemplate(
                            row.id,
                            !row.is_archived,
                            requireToken(csrfToken)
                          )
                        )
                      }
                    >
                      {row.is_archived ? t('restore') : t('archive')}
                    </Button>
                  </>
                ) : (
                  <Button
                    size="small"
                    disabled={!canManage || row.is_default}
                    onClick={() =>
                      action.mutate(() =>
                        resetSettingsUiTemplateDefault(
                          {
                            provider_code: row.provider_code,
                            contribution_code: row.contribution_code
                          },
                          requireToken(csrfToken)
                        )
                      )
                    }
                  >
                    {t('restore_official_default')}
                  </Button>
                )}
              </Space>
            )
          }
        ]}
      />
      <Drawer
        open={drawerOpen}
        width={720}
        title={editing ? t('edit_template') : t('new_template')}
        onClose={() => setDrawerOpen(false)}
        extra={
          <Button
            type="primary"
            loading={save.isPending}
            onClick={() => form.submit()}
          >
            {t('save_revision')}
          </Button>
        }
      >
        <Form form={form} layout="vertical" onFinish={(v) => save.mutate(v)}>
          <Form.Item
            name="provider_code"
            label={t('provider_code')}
            rules={[{ required: true }]}
          >
            <Input disabled={!!editing} />
          </Form.Item>
          <Form.Item
            name="contribution_code"
            label={t('contribution_code')}
            rules={[{ required: true }]}
          >
            <Input disabled={!!editing} />
          </Form.Item>
          <Form.Item name="name" label={t('name')} rules={[{ required: true }]}>
            <Input />
          </Form.Item>
          <Form.Item
            name="language"
            label={t('language')}
            rules={[{ required: true }]}
          >
            <Select
              options={[
                { value: 'tsx', label: 'TSX' },
                { value: 'jsx', label: 'JSX' }
              ]}
            />
          </Form.Item>
          <Form.Item
            name="source"
            label={t('initial_code')}
            rules={[{ required: true }]}
          >
            <Input.TextArea
              autoSize={{ minRows: 18, maxRows: 32 }}
              className="ui-management-code-editor"
            />
          </Form.Item>
        </Form>
      </Drawer>
    </SettingsSectionSurface>
  );
}

function ComponentsTab({ canManage }: { canManage: boolean }) {
  const { t } = useTranslation('settingsUiManagement');
  const csrfToken = useAuthStore((s) => s.csrfToken);
  const client = useQueryClient();
  const query = useQuery({
    queryKey: settingsUiComponentsQueryKey,
    queryFn: () => fetchSettingsUiComponents()
  });
  const [selected, setSelected] = useState<SettingsUiComponentCandidate | null>(
    null
  );
  const [form] = Form.useForm<{ contract: string }>();
  const refresh = () =>
    client.invalidateQueries({ queryKey: settingsUiComponentsQueryKey });
  const save = useMutation({
    mutationFn: async (value: { contract: string }) => {
      if (!selected) throw new Error('missing component');
      const contract = JSON.parse(value.contract) as Record<string, unknown>;
      return updateSettingsUiComponentContract(
        selected,
        contract,
        requireToken(csrfToken)
      );
    },
    onSuccess: async () => {
      await refresh();
      setSelected(null);
      message.success(t('saved'));
    }
  });
  const state = useMutation({
    mutationFn: async (input: {
      row: SettingsUiComponentCandidate;
      state: 'inherit' | 'published' | 'hidden';
    }) =>
      updateSettingsUiComponentState(
        input.row,
        input.state,
        requireToken(csrfToken)
      ),
    onSuccess: refresh
  });
  const edit = (row: SettingsUiComponentCandidate) => {
    setSelected(row);
    form.setFieldsValue({
      contract: JSON.stringify(
        row.latest_contract ??
          row.published_contract ??
          row.official_contract ?? {
            component_code: row.export_name,
            export_name: row.export_name,
            upstream: null,
            description: '',
            props: [],
            limitations: [''],
            examples: [{ title: '', code: '' }],
            insert_snippet: `<${row.export_name} />`
          },
        null,
        2
      )
    });
  };
  return (
    <SettingsSectionSurface>
      <Table
        loading={query.isLoading}
        rowKey={(r) =>
          `${r.provider_code}:${r.contribution_code}:${r.module_source}:${r.export_name}`
        }
        scroll={{ x: 1000 }}
        dataSource={query.data ?? []}
        columns={[
          { title: t('component'), dataIndex: 'export_name' },
          {
            title: t('module'),
            render: (_, r) => (
              <>
                <Typography.Text code>{r.module_source}</Typography.Text>
                <br />
                <Typography.Text type="secondary">
                  {r.module_version}
                </Typography.Text>
              </>
            )
          },
          {
            title: t('contribution'),
            render: (_, r) => `${r.provider_code}/${r.contribution_code}`
          },
          {
            title: t('state'),
            render: (_, r) => (
              <Tag
                color={
                  r.state === 'published'
                    ? 'green'
                    : r.state === 'hidden'
                      ? 'red'
                      : 'default'
                }
              >
                {t(r.state)}
              </Tag>
            )
          },
          {
            title: t('revision'),
            render: (_, r) =>
              r.latest_revision ? `r${r.latest_revision}` : '—'
          },
          {
            title: t('actions'),
            fixed: 'right',
            render: (_, r) => (
              <Space wrap>
                <Button
                  size="small"
                  disabled={!canManage}
                  onClick={() => edit(r)}
                >
                  {t('edit_contract')}
                </Button>
                <Button
                  size="small"
                  disabled={!canManage || !r.latest_contract}
                  onClick={() => state.mutate({ row: r, state: 'published' })}
                >
                  {t('publish')}
                </Button>
                <Button
                  size="small"
                  danger
                  disabled={!canManage || r.state === 'hidden'}
                  onClick={() => state.mutate({ row: r, state: 'hidden' })}
                >
                  {t('hide')}
                </Button>
                <Button
                  size="small"
                  disabled={!canManage || r.state === 'inherit'}
                  onClick={() => state.mutate({ row: r, state: 'inherit' })}
                >
                  {t('restore_official')}
                </Button>
              </Space>
            )
          }
        ]}
      />
      <Drawer
        open={!!selected}
        width={720}
        title={selected ? `${t('edit_contract')}: ${selected.export_name}` : ''}
        onClose={() => setSelected(null)}
        extra={
          <Button
            type="primary"
            loading={save.isPending}
            onClick={() => form.submit()}
          >
            {t('save_revision')}
          </Button>
        }
      >
        <Typography.Paragraph type="secondary">
          {t('contract_help')}
        </Typography.Paragraph>
        <Form
          form={form}
          layout="vertical"
          onFinish={(v) => {
            try {
              JSON.parse(v.contract);
              save.mutate(v);
            } catch {
              Modal.error({ title: t('invalid_json') });
            }
          }}
        >
          <Form.Item
            name="contract"
            label={t('contract_json')}
            rules={[{ required: true }]}
          >
            <Input.TextArea
              autoSize={{ minRows: 22, maxRows: 36 }}
              className="ui-management-code-editor"
            />
          </Form.Item>
        </Form>
      </Drawer>
    </SettingsSectionSurface>
  );
}

export function UiManagementPanel({ canManage }: { canManage: boolean }) {
  const { t } = useTranslation('settingsUiManagement');
  const navigate = useNavigate();
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  const active = pathname.endsWith('/components')
    ? 'components'
    : 'code-templates';
  return (
    <>
      <Tabs
        activeKey={active}
        onChange={(key) =>
          navigate({
            to:
              key === 'components'
                ? '/settings/ui-management/components'
                : '/settings/ui-management/code-templates'
          })
        }
        items={[
          {
            key: 'code-templates',
            label: t('code_templates'),
            children: <CodeTemplatesTab canManage={canManage} />
          },
          {
            key: 'components',
            label: t('components'),
            children: <ComponentsTab canManage={canManage} />
          }
        ]}
      />
    </>
  );
}
