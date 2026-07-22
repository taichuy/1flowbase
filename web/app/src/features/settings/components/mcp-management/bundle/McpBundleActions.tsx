import { UploadOutlined } from '@ant-design/icons';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Descriptions,
  Form,
  Input,
  Modal,
  Select,
  Space,
  Table,
  Tag,
  Typography,
  message
} from 'antd';
import { useRef, useState } from 'react';

import { useAuthStore } from '../../../../../state/auth-store';
import { i18nText } from '../../../../../shared/i18n/text';
import {
  exportSettingsMcpBundle,
  fetchSettingsOfficialMcpBundles,
  importSettingsMcpBundle,
  importSettingsOfficialMcpBundle,
  previewSettingsMcpBundle,
  previewSettingsOfficialMcpBundle,
  settingsMcpCatalogQueryKey,
  settingsOfficialMcpBundlesQueryKey,
  type ExportSettingsMcpBundleBody,
  type SettingsMcpBundleImportReport,
  type SettingsMcpBundlePreview,
  type SettingsOfficialMcpBundleEntry
} from '../../../api/mcp-management';

type BundleReview = SettingsMcpBundlePreview | SettingsMcpBundleImportReport;

function versionWarning(status: SettingsMcpBundlePreview['version_status']) {
  if (status === 'exported_from_older_system') {
    return i18nText('settingsMcpManagement', 'auto.mcp_bundle_source_older');
  }
  if (status === 'exported_from_newer_system') {
    return i18nText('settingsMcpManagement', 'auto.mcp_bundle_source_newer');
  }
  if (status === 'unknown_system_version') {
    return i18nText('settingsMcpManagement', 'auto.mcp_bundle_source_unknown');
  }
  return null;
}

function itemReason(reason: string | null) {
  switch (reason) {
    case 'interface_missing':
      return i18nText(
        'settingsMcpManagement',
        'auto.mcp_bundle_interface_missing'
      );
    case 'tool_id_conflict':
    case 'instance_id_conflict':
    case 'connection_id_conflict':
      return i18nText('settingsMcpManagement', 'auto.mcp_bundle_id_conflict');
    case 'connection_missing':
      return i18nText(
        'settingsMcpManagement',
        'auto.mcp_bundle_connection_missing'
      );
    case 'credentials_missing':
      return i18nText(
        'settingsMcpManagement',
        'auto.upstream_credentials_missing'
      );
    case 'binding_tool_missing':
      return i18nText(
        'settingsMcpManagement',
        'auto.mcp_bundle_binding_tool_missing'
      );
    default:
      return reason ?? '-';
  }
}

function resultColor(result: string) {
  if (result === 'imported') return 'green';
  if (result === 'unavailable' || result === 'failed') return 'red';
  return 'default';
}

function downloadBundle(blob: Blob, filename: string | null) {
  const url = window.URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = filename ?? 'mcp-bundle.zip';
  document.body.append(anchor);
  anchor.click();
  anchor.remove();
  window.URL.revokeObjectURL(url);
}

export function McpBundleActions({ canManage }: { canManage: boolean }) {
  const csrfToken = useAuthStore((state) => state.csrfToken ?? '');
  const queryClient = useQueryClient();
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [selectedOfficial, setSelectedOfficial] =
    useState<SettingsOfficialMcpBundleEntry | null>(null);
  const [sourceOpen, setSourceOpen] = useState(false);
  const [review, setReview] = useState<BundleReview | null>(null);
  const [previewing, setPreviewing] = useState(false);
  const [importing, setImporting] = useState(false);
  const [exportOpen, setExportOpen] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [exportForm] = Form.useForm<ExportSettingsMcpBundleBody>();
  const officialBundles = useQuery({
    queryKey: settingsOfficialMcpBundlesQueryKey,
    queryFn: fetchSettingsOfficialMcpBundles,
    enabled: canManage && sourceOpen
  });

  if (!canManage) {
    return null;
  }

  async function handleFile(file: File) {
    setSelectedFile(file);
    setSelectedOfficial(null);
    setSourceOpen(false);
    setPreviewing(true);
    try {
      setReview(await previewSettingsMcpBundle(file, csrfToken));
    } catch (error) {
      setSelectedFile(null);
      message.error(error instanceof Error ? error.message : String(error));
    } finally {
      setPreviewing(false);
      if (fileInputRef.current) fileInputRef.current.value = '';
    }
  }

  async function handleOfficial(entry: SettingsOfficialMcpBundleEntry) {
    setSelectedFile(null);
    setSelectedOfficial(entry);
    setPreviewing(true);
    try {
      setReview(
        await previewSettingsOfficialMcpBundle(
          { organization: entry.organization, bundle_id: entry.bundle_id },
          csrfToken
        )
      );
      setSourceOpen(false);
    } catch (error) {
      setSelectedOfficial(null);
      message.error(error instanceof Error ? error.message : String(error));
    } finally {
      setPreviewing(false);
    }
  }

  async function handleImport() {
    if (!selectedFile && !selectedOfficial) return;
    setImporting(true);
    try {
      const report = selectedFile
        ? await importSettingsMcpBundle(selectedFile, csrfToken)
        : await importSettingsOfficialMcpBundle(
            {
              organization: selectedOfficial!.organization,
              bundle_id: selectedOfficial!.bundle_id
            },
            csrfToken
          );
      setReview(report);
      await queryClient.invalidateQueries({
        queryKey: settingsMcpCatalogQueryKey
      });
    } catch (error) {
      message.error(error instanceof Error ? error.message : String(error));
    } finally {
      setImporting(false);
    }
  }

  async function handleExport(values: ExportSettingsMcpBundleBody) {
    setExporting(true);
    try {
      const response = await exportSettingsMcpBundle(values, csrfToken);
      downloadBundle(response.blob, response.filename);
      setExportOpen(false);
      message.success(
        i18nText('settingsMcpManagement', 'auto.mcp_bundle_export_ready')
      );
    } catch (error) {
      message.error(error instanceof Error ? error.message : String(error));
    } finally {
      setExporting(false);
    }
  }

  const warning = review ? versionWarning(review.version_status) : null;
  const imported = review && 'status' in review;
  const rows = review
    ? [
        ...review.tools.map((item) => ({ ...item, kind: 'Tool' })),
        ...review.instances.map((item) => ({ ...item, kind: 'Instance' })),
        ...review.connections.map((item) => ({ ...item, kind: 'Connection' }))
      ]
    : [];

  return (
    <>
      <input
        ref={fileInputRef}
        aria-label={i18nText(
          'settingsMcpManagement',
          'auto.mcp_bundle_select_file'
        )}
        hidden
        type="file"
        accept=".zip,application/zip"
        onChange={(event) => {
          const file = event.target.files?.[0];
          if (file) void handleFile(file);
        }}
      />
      <Space size="small">
        <Button
          icon={<UploadOutlined />}
          loading={previewing}
          onClick={() => setSourceOpen(true)}
        >
          {i18nText('settingsMcpManagement', 'auto.mcp_bundle_import_all')}
        </Button>
        <Button icon={<UploadOutlined />} onClick={() => setExportOpen(true)}>
          {i18nText('settingsMcpManagement', 'auto.mcp_bundle_export_all')}
        </Button>
      </Space>

      <Modal
        width={760}
        open={sourceOpen}
        title={i18nText(
          'settingsMcpManagement',
          'auto.mcp_bundle_source_title'
        )}
        footer={null}
        onCancel={() => setSourceOpen(false)}
      >
        <Space direction="vertical" size="middle" style={{ width: '100%' }}>
          <Button
            block
            icon={<UploadOutlined />}
            onClick={() => fileInputRef.current?.click()}
          >
            {i18nText('settingsMcpManagement', 'auto.mcp_bundle_upload_local')}
          </Button>
          <Typography.Title level={5} style={{ margin: 0 }}>
            {i18nText('settingsMcpManagement', 'auto.mcp_bundle_official')}
          </Typography.Title>
          {officialBundles.isError ? (
            <Alert
              showIcon
              type="error"
              message={
                officialBundles.error instanceof Error
                  ? officialBundles.error.message
                  : String(officialBundles.error)
              }
            />
          ) : null}
          <Table
            size="small"
            rowKey={(entry) => `${entry.organization}/${entry.bundle_id}`}
            loading={officialBundles.isLoading || previewing}
            pagination={false}
            dataSource={officialBundles.data?.entries ?? []}
            columns={[
              {
                title: i18nText(
                  'settingsMcpManagement',
                  'auto.mcp_bundle_name'
                ),
                render: (_, entry) => `${entry.organization}/${entry.bundle_id}`
              },
              {
                title: i18nText(
                  'settingsMcpManagement',
                  'auto.mcp_bundle_version'
                ),
                dataIndex: 'latest_version'
              },
              { title: 'Locale', dataIndex: 'locale' },
              {
                title: i18nText(
                  'settingsMcpManagement',
                  'auto.mcp_bundle_action'
                ),
                render: (_, entry) => (
                  <Button
                    type="link"
                    onClick={() => void handleOfficial(entry)}
                  >
                    {i18nText(
                      'settingsMcpManagement',
                      'auto.mcp_bundle_preview'
                    )}
                  </Button>
                )
              }
            ]}
          />
        </Space>
      </Modal>

      <Modal
        width={760}
        open={review !== null}
        title={i18nText(
          'settingsMcpManagement',
          'auto.mcp_bundle_import_title'
        )}
        okText={
          imported
            ? i18nText('settings', 'auto.close')
            : i18nText('settingsMcpManagement', 'auto.mcp_bundle_import_anyway')
        }
        cancelButtonProps={{
          style: imported ? { display: 'none' } : undefined
        }}
        confirmLoading={importing}
        onCancel={() => {
          setReview(null);
          setSelectedFile(null);
          setSelectedOfficial(null);
        }}
        onOk={() => {
          if (imported) {
            setReview(null);
            setSelectedFile(null);
            setSelectedOfficial(null);
          } else {
            void handleImport();
          }
        }}
      >
        {review ? (
          <Space direction="vertical" size="middle" style={{ width: '100%' }}>
            {warning ? (
              <Alert showIcon type="warning" message={warning} />
            ) : null}
            {imported ? (
              <Alert
                showIcon
                type={review.status === 'completed' ? 'success' : 'warning'}
                message={
                  review.status === 'completed'
                    ? i18nText(
                        'settingsMcpManagement',
                        'auto.mcp_bundle_import_completed'
                      )
                    : i18nText(
                        'settingsMcpManagement',
                        'auto.mcp_bundle_import_completed_with_warnings'
                      )
                }
              />
            ) : null}
            <Descriptions size="small" column={2}>
              <Descriptions.Item
                label={i18nText(
                  'settingsMcpManagement',
                  'auto.mcp_bundle_name'
                )}
              >
                {review.manifest.organization}/{review.manifest.bundle_id}
              </Descriptions.Item>
              <Descriptions.Item
                label={i18nText(
                  'settingsMcpManagement',
                  'auto.mcp_bundle_version'
                )}
              >
                {review.manifest.bundle_version}
              </Descriptions.Item>
              <Descriptions.Item
                label={i18nText(
                  'settingsMcpManagement',
                  'auto.mcp_bundle_source_version'
                )}
              >
                {review.manifest.exported_from_system_version}
              </Descriptions.Item>
              <Descriptions.Item
                label={i18nText(
                  'settingsMcpManagement',
                  'auto.mcp_bundle_current_version'
                )}
              >
                {review.current_system_version}
              </Descriptions.Item>
            </Descriptions>
            <Table
              size="small"
              rowKey={(item) => `${item.kind}:${item.id}`}
              pagination={false}
              dataSource={rows}
              columns={[
                {
                  title: i18nText(
                    'settingsMcpManagement',
                    'auto.mcp_bundle_item'
                  ),
                  dataIndex: 'id'
                },
                {
                  title: i18nText(
                    'settingsMcpManagement',
                    'auto.mcp_bundle_kind'
                  ),
                  dataIndex: 'kind'
                },
                {
                  title: i18nText(
                    'settingsMcpManagement',
                    'auto.mcp_bundle_result'
                  ),
                  dataIndex: 'result',
                  render: (value: string) => (
                    <Tag color={resultColor(value)}>{value}</Tag>
                  )
                },
                {
                  title: i18nText(
                    'settingsMcpManagement',
                    'auto.mcp_bundle_reason'
                  ),
                  dataIndex: 'reason',
                  render: (value: string | null) => itemReason(value)
                }
              ]}
            />
          </Space>
        ) : null}
      </Modal>

      <Modal
        open={exportOpen}
        title={i18nText(
          'settingsMcpManagement',
          'auto.mcp_bundle_export_title'
        )}
        okText={i18nText('settingsMcpManagement', 'auto.mcp_bundle_export_all')}
        confirmLoading={exporting}
        onCancel={() => setExportOpen(false)}
        onOk={() => exportForm.submit()}
      >
        <Form<ExportSettingsMcpBundleBody>
          form={exportForm}
          layout="vertical"
          initialValues={{
            organization: 'taichuy',
            bundle_id: '1flowbase_zh_hans',
            bundle_version: '1.0.0',
            locale: 'zh_Hans',
            minimum_host_version: '0.2.6'
          }}
          onFinish={(values) => void handleExport(values)}
        >
          <Form.Item
            name="organization"
            label="organization"
            rules={[{ required: true }]}
          >
            <Input />
          </Form.Item>
          <Form.Item
            name="bundle_id"
            label="bundle_id"
            rules={[{ required: true }]}
          >
            <Input />
          </Form.Item>
          <Form.Item
            name="bundle_version"
            label="bundle_version"
            rules={[{ required: true }]}
          >
            <Input />
          </Form.Item>
          <Form.Item name="locale" label="locale" rules={[{ required: true }]}>
            <Select options={[{ value: 'zh_Hans' }, { value: 'en_US' }]} />
          </Form.Item>
          <Form.Item
            name="minimum_host_version"
            label="minimum_host_version"
            rules={[{ required: true }]}
          >
            <Input />
          </Form.Item>
          <Typography.Text type="secondary">
            {i18nText(
              'settingsMcpManagement',
              'auto.mcp_bundle_system_version_recorded'
            )}
          </Typography.Text>
        </Form>
      </Modal>
    </>
  );
}
