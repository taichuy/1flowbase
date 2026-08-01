import { UploadOutlined } from '@ant-design/icons';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Alert, Button, Modal, Space, Table, Typography, message } from 'antd';
import { useRef, useState } from 'react';

import { useAuthStore } from '../../../../../state/auth-store';
import { i18nText } from '../../../../../shared/i18n/text';
import {
  exportSettingsMcpBundle,
  fetchSettingsMcpBundleExportDefaults,
  fetchSettingsOfficialMcpBundles,
  importSettingsMcpBundle,
  importSettingsOfficialMcpBundle,
  previewSettingsMcpBundle,
  previewSettingsOfficialMcpBundle,
  settingsMcpCatalogQueryKey,
  settingsMcpBundleExportDefaultsQueryKey,
  settingsOfficialMcpBundlesQueryKey,
  type ExportSettingsMcpBundleBody,
  type SettingsOfficialMcpBundleEntry
} from '../../../api/mcp-management';
import { McpBundleExportModal } from './McpBundleExportModal';
import {
  McpBundleReviewModal,
  type McpBundleReview
} from './McpBundleReviewModal';
import { downloadMcpBundle } from './mcp-bundle-download';

export function McpBundleActions({ canManage }: { canManage: boolean }) {
  const csrfToken = useAuthStore((state) => state.csrfToken ?? '');
  const queryClient = useQueryClient();
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [selectedOfficial, setSelectedOfficial] =
    useState<SettingsOfficialMcpBundleEntry | null>(null);
  const [sourceOpen, setSourceOpen] = useState(false);
  const [review, setReview] = useState<McpBundleReview | null>(null);
  const [previewing, setPreviewing] = useState(false);
  const [importing, setImporting] = useState(false);
  const [exportOpen, setExportOpen] = useState(false);
  const [exporting, setExporting] = useState(false);
  const officialBundles = useQuery({
    queryKey: settingsOfficialMcpBundlesQueryKey,
    queryFn: fetchSettingsOfficialMcpBundles,
    enabled: canManage && sourceOpen
  });
  const exportDefaults = useQuery({
    queryKey: settingsMcpBundleExportDefaultsQueryKey,
    queryFn: fetchSettingsMcpBundleExportDefaults,
    enabled: canManage
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
      downloadMcpBundle(response.blob, response.filename);
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
          {i18nText('settingsMcpManagement', 'auto.mcp_bundle_import')}
        </Button>
        <Button icon={<UploadOutlined />} onClick={() => setExportOpen(true)}>
          {i18nText('settingsMcpManagement', 'auto.mcp_bundle_export')}
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

      <McpBundleReviewModal
        review={review}
        importing={importing}
        onCancel={() => {
          setReview(null);
          setSelectedFile(null);
          setSelectedOfficial(null);
        }}
        onImport={() => void handleImport()}
      />

      <McpBundleExportModal
        open={exportOpen}
        title={i18nText(
          'settingsMcpManagement',
          'auto.mcp_bundle_export_title'
        )}
        okText={i18nText('settingsMcpManagement', 'auto.mcp_bundle_export')}
        defaultBundleId="1flowbase_zh_hans"
        exportDefaults={exportDefaults.data}
        exporting={exporting}
        onCancel={() => setExportOpen(false)}
        onExport={handleExport}
      />
    </>
  );
}
