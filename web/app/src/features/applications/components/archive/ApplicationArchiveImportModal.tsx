import { Alert, Input, Modal, Space, Table, Tag, Typography } from 'antd';
import { useTranslation } from 'react-i18next';
import type {
  ConsoleApplicationArchivePreview,
  ImportConsoleApplicationArchiveResponse
} from '@1flowbase/api-client';

export function ApplicationArchiveImportModal({
  open,
  preview,
  names,
  importing,
  results,
  onNameChange,
  onCancel,
  onImport
}: {
  open: boolean;
  preview: ConsoleApplicationArchivePreview | null;
  names: Record<number, string>;
  importing: boolean;
  results?: ImportConsoleApplicationArchiveResponse | null;
  onNameChange: (entry_index: number, name: string) => void;
  onCancel: () => void;
  onImport: () => void;
}) {
  const { t } = useTranslation('applications');
  const applications = preview?.applications ?? [];
  return (
    <Modal
      open={open}
      title={t('auto.import_template')}
      width={960}
      okText={t('auto.import_template')}
      cancelText={results ? t('archive_import.close') : t('auto.cancel')}
      confirmLoading={importing}
      closable={!importing}
      mask={{ closable: !importing }}
      keyboard={!importing}
      cancelButtonProps={{ disabled: importing }}
      okButtonProps={{
        style: results ? { display: 'none' } : undefined,
        disabled:
          Boolean(results) ||
          applications.length === 0 ||
          applications.some(({ entry_index }) => !names[entry_index]?.trim())
      }}
      onCancel={onCancel}
      onOk={onImport}
    >
      <Space orientation="vertical" size={16} style={{ width: '100%' }}>
        <Alert
          showIcon
          type={
            results?.failed_count || results?.partial_count ? 'warning' : 'info'
          }
          title={
            results
              ? t('archive_import.result_summary', {
                  succeeded_count: results.succeeded_count,
                  failed_count: results.failed_count,
                  partial_count: results.partial_count
                })
              : t('archive_import.preview_summary', {
                  count: applications.length
                })
          }
          description={
            results?.partial_count
              ? t('archive_import.partial_help')
              : undefined
          }
        />
        <Table
          size="small"
          pagination={false}
          rowKey="entry_index"
          dataSource={applications}
          scroll={{ x: 660 }}
          columns={[
            {
              title: t('auto.application_name'),
              key: 'name',
              render: (_, entry) => (
                <Space orientation="vertical" size={4}>
                  <Input
                    aria-label={
                      applications.length === 1
                        ? t('auto.application_name')
                        : `${t('auto.application_name')} ${entry.entry_index + 1}`
                    }
                    value={names[entry.entry_index] ?? ''}
                    maxLength={80}
                    disabled={importing || Boolean(results)}
                    onChange={(event) =>
                      onNameChange(entry.entry_index, event.target.value)
                    }
                  />
                  <Typography.Text type="secondary">
                    {entry.preview.application.description}
                  </Typography.Text>
                </Space>
              )
            },
            {
              title: t('archive_import.application_type'),
              key: 'type',
              width: 120,
              render: (_, entry) =>
                entry.preview.application.application_type === 'workflow'
                  ? 'Workflow'
                  : 'AgentFlow'
            },
            {
              title: t('auto.template_dependency_summary'),
              key: 'dependencies',
              width: 190,
              render: (_, entry) => (
                <Space wrap>
                  <Tag>
                    {t('auto.missing_dependency_count', {
                      value1: entry.preview.dependencies.filter(
                        (d) => d.status !== 'ready'
                      ).length
                    })}
                  </Tag>
                  <Tag>
                    {t('auto.unresolved_node_count', {
                      value1: entry.preview.unresolved_nodes.length
                    })}
                  </Tag>
                </Space>
              )
            },
            ...(results
              ? [
                  {
                    title: t('archive_import.result'),
                    key: 'result',
                    width: 150,
                    render: (
                      _: unknown,
                      entry: ConsoleApplicationArchivePreview['applications'][number]
                    ) => {
                      const result = results.results.find(
                        (item) => item.entry_index === entry.entry_index
                      );
                      if (!result) return null;
                      return (
                        <Space orientation="vertical" size={4}>
                          <Tag
                            color={
                              result.status === 'succeeded'
                                ? 'success'
                                : result.status === 'partial'
                                  ? 'warning'
                                  : 'error'
                            }
                          >
                            {result.status === 'succeeded'
                              ? t('archive_import.succeeded')
                              : result.status === 'partial'
                                ? t('archive_import.partial')
                                : t('archive_import.failed')}
                          </Tag>
                          {result.status !== 'succeeded' ? (
                            <Typography.Text type="danger">
                              {result.code === 'extension_slug'
                                ? t('archive_import.path_conflict')
                                : t('archive_import.entry_failed')}
                            </Typography.Text>
                          ) : null}
                          {result.status === 'partial' ? (
                            <a
                              href={`/applications/${result.application_id}/orchestration`}
                            >
                              {t('archive_import.inspect_application')}
                            </a>
                          ) : null}
                        </Space>
                      );
                    }
                  }
                ]
              : [])
          ]}
          expandable={{
            rowExpandable: (entry) =>
              entry.preview.dependencies.some((d) => d.status !== 'ready') ||
              entry.preview.unresolved_nodes.length > 0,
            expandedRowRender: (entry) => (
              <Space orientation="vertical">
                {entry.preview.dependencies
                  .filter((d) => d.status !== 'ready')
                  .map((dependency, index) => (
                    <Typography.Text key={index}>
                      {dependency.dependency.node_type ??
                        dependency.dependency.provider_code ??
                        dependency.dependency.plugin_id}
                      : {dependency.reason}
                    </Typography.Text>
                  ))}
                {entry.preview.unresolved_nodes.map((node) => (
                  <Typography.Text key={node.node_id}>
                    {node.alias}: {node.reason}
                  </Typography.Text>
                ))}
              </Space>
            )
          }}
        />
      </Space>
    </Modal>
  );
}
