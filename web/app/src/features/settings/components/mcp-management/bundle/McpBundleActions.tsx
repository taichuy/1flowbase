import { UploadOutlined } from '@ant-design/icons';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Button, Modal, Space, message } from 'antd';
import { useCallback, useRef, useState } from 'react';

import { useAuthStore } from '../../../../../state/auth-store';
import { i18nText } from '../../../../../shared/i18n/text';
import {
  exportSettingsMcpBundle,
  fetchSettingsMcpBundleExportDefaults,
  settingsMcpCatalogQueryKey,
  settingsMcpBundleExportDefaultsQueryKey,
  type ExportSettingsMcpBundleBody
} from '../../../api/mcp-management';
import { McpBundleExportModal } from './McpBundleExportModal';
import {
  McpBundleImportFlow,
  type McpBundleImportSource
} from './McpBundleImportFlow';
import { downloadMcpBundle } from './mcp-bundle-download';
import { McpTemplateLibrary } from './McpTemplateLibrary';

export function McpBundleActions({ canManage }: { canManage: boolean }) {
  const csrfToken = useAuthStore((state) => state.csrfToken ?? '');
  const queryClient = useQueryClient();
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const [importSource, setImportSource] =
    useState<McpBundleImportSource | null>(null);
  const [sourceOpen, setSourceOpen] = useState(false);
  const [exportOpen, setExportOpen] = useState(false);
  const [exporting, setExporting] = useState(false);
  const exportDefaults = useQuery({
    queryKey: settingsMcpBundleExportDefaultsQueryKey,
    queryFn: fetchSettingsMcpBundleExportDefaults,
    enabled: canManage
  });
  const closeImportFlow = useCallback(() => setImportSource(null), []);
  const refreshMcpCatalog = useCallback(async () => {
    await queryClient.invalidateQueries({
      queryKey: settingsMcpCatalogQueryKey
    });
  }, [queryClient]);

  if (!canManage) {
    return null;
  }

  function handleFile(file: File) {
    setImportSource({ kind: 'upload', file });
    setSourceOpen(false);
    if (fileInputRef.current) fileInputRef.current.value = '';
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
        <Button icon={<UploadOutlined />} onClick={() => setSourceOpen(true)}>
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
          <McpTemplateLibrary
            enabled={sourceOpen}
            variant="compact"
            onImportOpen={() => setSourceOpen(false)}
          />
        </Space>
      </Modal>

      <McpBundleImportFlow
        source={importSource}
        csrfToken={csrfToken}
        onClose={closeImportFlow}
        onApplied={refreshMcpCatalog}
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
