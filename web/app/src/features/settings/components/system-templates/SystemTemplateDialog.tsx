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
import { useSystemTemplates } from '../../api/system-templates/useSystemTemplates';

const emptySelection = (): PortableTemplateSelection => ({
  page_ids: [],
  application_ids: [],
  data_model_ids: []
});

export function SystemTemplateDialog({
  mode,
  file,
  onClose
}: {
  mode: 'export' | 'import';
  file?: File;
  onClose: () => void;
}) {
  const { t } = useTranslation('settingsSystemTemplates');
  const exportOpen = mode === 'export';
  const [selection, setSelection] = useState(emptySelection);
  const [importFile, setImportFile] = useState<{
    name: string;
    body: PortableTemplatePackage;
  }>();
  const [reading, setReading] = useState(false);
  const [fileError, setFileError] = useState(false);
  const { catalog, exportMutation, previewMutation, installMutation } =
    useSystemTemplates(exportOpen);
  const preview = previewMutation.data;
  const result = installMutation.data;
  const busy =
    reading || previewMutation.isPending || installMutation.isPending;
  const { mutate: previewFile } = previewMutation;
  useEffect(() => {
    if (!file) return;
    let cancelled = false;
    setReading(true);
    void file
      .text()
      .then((text) => {
        if (cancelled) return;
        const body: unknown = JSON.parse(text);
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
  }, [file, previewFile]);
  const download = async () => {
    try {
      const body = await exportMutation.mutateAsync(selection);
      const url = URL.createObjectURL(
        new Blob([JSON.stringify(body, null, 2)], { type: 'application/json' })
      );
      const anchor = document.createElement('a');
      anchor.href = url;
      anchor.download = '1flowbase-template.json';
      anchor.click();
      window.setTimeout(() => URL.revokeObjectURL(url), 1000);
      onClose();
      setSelection(emptySelection());
    } catch {
      /* The mutation error is shown in the export dialog. */
    }
  };
  return (
    <>
      <Modal
        open={mode === 'import'}
        title={t('import')}
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
        <Typography.Paragraph type="secondary">
          {t('plugin_notice')}
        </Typography.Paragraph>
        {reading && <Typography.Text>{file?.name}</Typography.Text>}
        {fileError && <Alert type="error" showIcon title={t('invalid_json')} />}
        {importFile && (
          <Space orientation="vertical" size="middle" style={{ width: '100%' }}>
            <Typography.Text>{importFile.name}</Typography.Text>
            {previewMutation.isError && (
              <Alert type="error" showIcon title={t('preview_failed')} />
            )}
            {preview && (
              <>
                <Descriptions
                  title={t('preview')}
                  column={{ xs: 1, sm: 3 }}
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
                {result.failures.map((failure, i) => (
                  <Alert key={i} type="error" title={failure} />
                ))}
                <Table
                  size="small"
                  pagination={{ pageSize: 10 }}
                  scroll={{ x: 640 }}
                  dataSource={result.created}
                  rowKey={(item) => `${item.kind}:${item.source_id}`}
                  columns={[
                    { title: t('kind'), dataIndex: 'kind' },
                    { title: t('source_id'), dataIndex: 'source_id' },
                    { title: t('target_id'), dataIndex: 'target_id' }
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
                  !preview?.valid ||
                  Boolean(preview.failures.length) ||
                  Boolean(result) ||
                  installMutation.isError
                }
                onClick={() => installMutation.mutate(importFile.body)}
              >
                {t('install')}
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
        {exportMutation.isError && (
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
        </Form>
      </Modal>
    </>
  );
}
