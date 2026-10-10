import type {
  PortableTemplatePackage,
  PortableTemplateSelection
} from '@1flowbase/api-client';
import {
  Alert,
  Button,
  Descriptions,
  Form,
  Modal,
  Select,
  Space,
  Table,
  TreeSelect,
  Typography
} from 'antd';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useAuthStore } from '../../../../state/auth-store';
import { useSystemTemplates } from '../../api/system-templates/useSystemTemplates';
import { readTemplateFile, templateArchiveBlob } from './archive';

const emptySelection = (): PortableTemplateSelection => ({
  page_ids: [],
  application_ids: [],
  data_model_ids: [],
  mcp_instance_ids: [],
  i18n_keys: []
});

export function SystemTemplateDialog({
  mode,
  file,
  template,
  canInstall = true,
  onClose
}: {
  mode: 'export' | 'import';
  file?: File;
  template?: { name: string; body: PortableTemplatePackage };
  canInstall?: boolean;
  onClose: () => void;
}) {
  const { t } = useTranslation('settingsSystemTemplates');
  const effectLabel = (action: string) => {
    switch (action) {
      case 'skip':
        return t('effect_skip');
      case 'unchanged':
        return t('effect_unchanged');
      case 'update':
        return t('effect_update');
      case 'create':
        return t('effect_create');
      default:
        return t('effect_unknown');
    }
  };
  const reasonLabel = (reason?: string | null) => {
    switch (reason) {
      case 'user_modified':
        return t('skip_local_modified');
      case 'unknown_baseline':
        return t('skip_missing_baseline');
      case 'user_deleted':
        return t('skip_user_deleted');
      case 'pending_write':
        return t('skip_pending_write');
      case undefined:
      case null:
        return null;
      default:
        return t('skip_preserved');
    }
  };
  const exportOpen = mode === 'export';
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const [selection, setSelection] = useState(emptySelection);
  const [importFile, setImportFile] = useState<{
    name: string;
    body: PortableTemplatePackage;
  }>();
  const [reading, setReading] = useState(false);
  const [fileError, setFileError] = useState(false);
  const [downloadError, setDownloadError] = useState(false);
  const { catalog, exportMutation, previewMutation, installMutation } =
    useSystemTemplates(exportOpen);
  const preview = previewMutation.data;
  const result = installMutation.data;
  const busy =
    reading || previewMutation.isPending || installMutation.isPending;
  const { mutate: previewFile } = previewMutation;
  useEffect(() => {
    if (template) {
      setImportFile(template);
      previewFile(template.body);
      return;
    }
    if (!file) return;
    let cancelled = false;
    setReading(true);
    void readTemplateFile(file)
      .then((body) => {
        if (cancelled) return;
        setImportFile({ name: file.name, body });
        previewFile(body);
      })
      .catch(() => {
        if (!cancelled) setFileError(true);
      })
      .finally(() => {
        if (!cancelled) setReading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [file, template, previewFile]);
  const download = async () => {
    setDownloadError(false);
    try {
      const body = await exportMutation.mutateAsync(selection);
      const url = URL.createObjectURL(templateArchiveBlob(body.archive_base64));
      const anchor = document.createElement('a');
      anchor.href = url;
      anchor.download = body.file_name;
      anchor.click();
      window.setTimeout(() => URL.revokeObjectURL(url), 1000);
      onClose();
      setSelection(emptySelection());
    } catch {
      setDownloadError(true);
    }
  };
  return (
    <>
      <Modal
        open={mode === 'import'}
        title={template ? t('preview') : t('import')}
        footer={null}
        width={800}
        closable={!busy}
        maskClosable={!busy}
        keyboard={!busy}
        onCancel={() => {
          if (!busy) onClose();
        }}
      >
        <Typography.Paragraph>{t('structure_notice')}</Typography.Paragraph>
        <Alert type="info" showIcon title={t('merge_notice')} />
        <Typography.Paragraph type="secondary">
          {t('plugin_notice')}
        </Typography.Paragraph>
        {reading && <Typography.Text>{file?.name}</Typography.Text>}
        {fileError && <Alert type="error" showIcon title={t('invalid_file')} />}
        {importFile && (
          <Space orientation="vertical" size="middle" style={{ width: '100%' }}>
            <Typography.Text>{importFile.name}</Typography.Text>
            {previewMutation.isError && (
              <Alert
                type="error"
                showIcon
                title={
                  template ? t('catalog_preview_failed') : t('preview_failed')
                }
              />
            )}
            {preview && (
              <>
                <Descriptions
                  title={t('preview')}
                  column={{ xs: 1, sm: 4 }}
                  items={[
                    {
                      key: 'pages',
                      label: t('pages'),
                      children: preview.counts.pages
                    },
                    {
                      key: 'applications',
                      label: t('applications'),
                      children: preview.counts.applications
                    },
                    {
                      key: 'data_models',
                      label: t('data_models'),
                      children: preview.counts.data_models
                    },
                    {
                      key: 'mcp_instances',
                      label: t('mcp_instances'),
                      children: preview.counts.mcp_instances
                    },
                    {
                      key: 'i18n_entries',
                      label: t('i18n_entries'),
                      children: preview.counts.i18n_entries
                    }
                  ]}
                />
                {preview.failures.map((failure, i) => (
                  <Alert
                    key={`failure-${i}`}
                    type="error"
                    showIcon
                    title={failure}
                  />
                ))}
                {preview.warnings.map((warning, i) => (
                  <Alert
                    key={`warning-${i}`}
                    type="warning"
                    showIcon
                    title={warning}
                  />
                ))}
                {preview.effects.length > 0 && (
                  <Table
                    size="small"
                    pagination={{ pageSize: 8 }}
                    dataSource={preview.effects}
                    rowKey={(item) => `${item.kind}:${item.source_id}`}
                    columns={[
                      { title: t('kind'), dataIndex: 'kind' },
                      { title: t('source_id'), dataIndex: 'source_id' },
                      {
                        title: t('action'),
                        dataIndex: 'action',
                        render: effectLabel
                      },
                      {
                        title: t('reason'),
                        dataIndex: 'reason',
                        render: reasonLabel
                      }
                    ]}
                  />
                )}
                {preview.mcp_shared_tool_impacts.length > 0 && (
                  <Alert
                    type="warning"
                    showIcon
                    title={t('shared_tool_impact')}
                    description={preview.mcp_shared_tool_impacts.map(
                      (impact) => (
                        <div key={impact.tool_id}>
                          {impact.tool_id}: {impact.instance_ids.join(', ')}
                        </div>
                      )
                    )}
                  />
                )}
                {preview.dependencies.length > 0 && (
                  <Table
                    size="small"
                    pagination={false}
                    scroll={{ x: 520 }}
                    dataSource={preview.dependencies}
                    rowKey={(item) =>
                      `${item.plugin_id}:${item.plugin_version}:${item.contribution_code}`
                    }
                    columns={[
                      { title: t('plugin'), dataIndex: 'plugin_id' },
                      { title: t('version'), dataIndex: 'plugin_version' },
                      {
                        title: t('contribution'),
                        dataIndex: 'contribution_code'
                      }
                    ]}
                  />
                )}
              </>
            )}
            {installMutation.isError && (
              <Alert type="error" showIcon title={t('install_unknown')} />
            )}
            {result && (
              <>
                <Alert
                  type={result.complete ? 'success' : 'warning'}
                  showIcon
                  title={result.complete ? t('installed') : t('partial')}
                  description={
                    !result.complete ? t('partial_notice') : undefined
                  }
                />
                {result.skipped.length > 0 && (
                  <Alert
                    type="info"
                    showIcon
                    title={t('skipped_notice', {
                      count: result.skipped.length
                    })}
                  />
                )}
                {result.failures.map((failure, i) => (
                  <Alert key={i} type="error" title={failure} />
                ))}
                <Table
                  size="small"
                  pagination={{ pageSize: 10 }}
                  scroll={{ x: 640 }}
                  dataSource={[
                    ...result.created.map((item) => ({
                      ...item,
                      action: 'create'
                    })),
                    ...result.updated.map((item) => ({
                      ...item,
                      action: 'update'
                    })),
                    ...result.skipped.map((item) => ({
                      ...item,
                      action: 'skip'
                    }))
                  ]}
                  rowKey={(item) => `${item.kind}:${item.source_id}`}
                  columns={[
                    { title: t('kind'), dataIndex: 'kind' },
                    { title: t('source_id'), dataIndex: 'source_id' },
                    { title: t('target_id'), dataIndex: 'target_id' },
                    {
                      title: t('action'),
                      dataIndex: 'action',
                      render: effectLabel
                    },
                    {
                      title: t('reason'),
                      dataIndex: 'reason',
                      render: reasonLabel
                    }
                  ]}
                />
                <Descriptions
                  title={t('reference_map')}
                  column={1}
                  size="small"
                  items={Object.entries(result.id_map).map(
                    ([source_id, target_id]) => ({
                      key: source_id,
                      label: source_id,
                      children: target_id
                    })
                  )}
                />
              </>
            )}
            <Space wrap>
              <Button
                type="primary"
                loading={installMutation.isPending}
                disabled={
                  busy ||
                  !canInstall ||
                  !csrfToken ||
                  !preview?.valid ||
                  Boolean(preview.failures.length) ||
                  Boolean(result) ||
                  installMutation.isError
                }
                onClick={() => installMutation.mutate(importFile.body)}
              >
                {template ? t('confirm_install') : t('install')}
              </Button>
              <Button disabled={busy} onClick={onClose}>
                {t('close')}
              </Button>
            </Space>
          </Space>
        )}
      </Modal>
      <Modal
        open={exportOpen}
        title={t('export')}
        onCancel={() => {
          if (!exportMutation.isPending) onClose();
        }}
        onOk={() => void download()}
        okText={t('download')}
        cancelText={t('cancel')}
        confirmLoading={exportMutation.isPending}
        cancelButtonProps={{ disabled: exportMutation.isPending }}
        okButtonProps={{
          disabled:
            catalog.isLoading ||
            catalog.isError ||
            !Object.values(selection).some((ids) => ids.length)
        }}
      >
        <Typography.Paragraph>{t('selection_notice')}</Typography.Paragraph>
        {catalog.isError && (
          <Alert
            type="error"
            showIcon
            title={t('catalog_failed')}
            action={
              <Button onClick={() => void catalog.refetch()}>
                {t('retry')}
              </Button>
            }
          />
        )}
        {(exportMutation.isError || downloadError) && (
          <Alert type="error" showIcon title={t('export_failed')} />
        )}
        <Form layout="vertical" disabled={exportMutation.isPending}>
          <Form.Item label={t('pages')}>
            <TreeSelect
              aria-label={t('pages')}
              style={{ width: '100%' }}
              multiple
              treeCheckable
              treeCheckStrictly
              treeDataSimpleMode={{ id: 'id', pId: 'parent_id' }}
              treeData={catalog.data?.pages.map((item) => ({
                ...item,
                value: item.id,
                title: item.name
              }))}
              value={selection.page_ids.map((value) => ({ value }))}
              loading={catalog.isLoading}
              onChange={(values: { value: string }[]) =>
                setSelection((current) => ({
                  ...current,
                  page_ids: values.map((item) => item.value)
                }))
              }
            />
          </Form.Item>
          <Form.Item label={t('applications')}>
            <Select
              aria-label={t('applications')}
              mode="multiple"
              optionFilterProp="label"
              loading={catalog.isLoading}
              options={catalog.data?.applications.map((item) => ({
                value: item.id,
                label: item.name
              }))}
              value={selection.application_ids}
              onChange={(application_ids: string[]) =>
                setSelection((current) => ({ ...current, application_ids }))
              }
            />
          </Form.Item>
          <Form.Item label={t('data_models')}>
            <Select
              aria-label={t('data_models')}
              mode="multiple"
              optionFilterProp="label"
              loading={catalog.isLoading}
              options={catalog.data?.data_models.map((item) => ({
                value: item.id,
                label: `${item.name}${item.code ? ` (${item.code})` : ''}`
              }))}
              value={selection.data_model_ids}
              onChange={(data_model_ids: string[]) =>
                setSelection((current) => ({ ...current, data_model_ids }))
              }
            />
          </Form.Item>
          <Form.Item label={t('mcp_instances')}>
            <Select
              aria-label={t('mcp_instances')}
              mode="multiple"
              optionFilterProp="label"
              loading={catalog.isLoading}
              options={catalog.data?.mcp_instances.map((item) => ({
                value: item.id,
                label: item.name
              }))}
              value={selection.mcp_instance_ids}
              onChange={(mcp_instance_ids: string[]) =>
                setSelection((current) => ({ ...current, mcp_instance_ids }))
              }
            />
          </Form.Item>
          <Form.Item
            label={t('i18n_entries')}
            extra={t('i18n_selection_notice')}
          >
            <Select
              aria-label={t('i18n_entries')}
              mode="multiple"
              optionFilterProp="label"
              loading={catalog.isLoading}
              options={catalog.data?.i18n_entries.map((item) => ({
                value: item.key,
                label: item.key
              }))}
              value={selection.i18n_keys}
              onChange={(i18n_keys: string[]) =>
                setSelection((current) => ({ ...current, i18n_keys }))
              }
            />
          </Form.Item>
        </Form>
      </Modal>
    </>
  );
}
