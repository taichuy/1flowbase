import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { App, Form } from 'antd';
import { useState } from 'react';

import type {
  ConsoleMcpCatalog,
  ConsoleMcpInstance
} from '@1flowbase/api-client';
import {
  createSettingsMcpInstance,
  copySettingsMcpInstance,
  deleteSettingsMcpInstance,
  exportSettingsMcpInstanceBundle,
  fetchSettingsMcpBundleExportDefaults,
  settingsMcpBundleExportDefaultsQueryKey,
  settingsMcpCatalogQueryKey,
  updateSettingsMcpInstance,
  type ExportSettingsMcpBundleBody
} from '../../../api/mcp-management';
import { useAuthStore } from '../../../../../state/auth-store';
import { i18nText } from '../../../../../shared/i18n/text';

import { type McpBundleImportSource } from '../bundle/McpBundleImportFlow';
import { downloadMcpBundle } from '../bundle/mcp-bundle-download';
import { notifyWebMcpRegistrationsChanged } from '../../../../webmcp/registration-events';

import type {
  InstanceFormValues,
  CopyInstanceFormValues
} from './editor-types';

import type { useMcpDirectoryEditor } from './use-mcp-directory-editor';

type InstanceEditorSession = Pick<
  ReturnType<typeof useMcpDirectoryEditor>,
  'editingInstance' | 'setEditingInstance' | 'setInstanceModalOpen'
>;

/** Owns instance save/copy/delete, export/download and connection/policy dialog selection. */
export function useMcpInstanceActions(
  canManage: boolean,
  catalog: ConsoleMcpCatalog,
  {
    editingInstance,
    setEditingInstance,
    setInstanceModalOpen
  }: InstanceEditorSession
) {
  const { message } = App.useApp();
  const csrfToken = useAuthStore((state) => state.csrfToken ?? '');
  const queryClient = useQueryClient();

  const exportDefaults = useQuery({
    queryKey: settingsMcpBundleExportDefaultsQueryKey,
    queryFn: fetchSettingsMcpBundleExportDefaults,
    enabled: canManage
  });
  const [instanceForm] = Form.useForm<InstanceFormValues>();
  const [copyInstanceForm] = Form.useForm<CopyInstanceFormValues>();
  const [discoveryPolicyInstance, setDiscoveryPolicyInstance] =
    useState<ConsoleMcpInstance | null>(null);
  const [clientConfigurationInstance, setClientConfigurationInstance] =
    useState<ConsoleMcpInstance | null>(null);
  const [copyingInstance, setCopyingInstance] =
    useState<ConsoleMcpInstance | null>(null);
  const [bundleExportInstance, setBundleExportInstance] =
    useState<ConsoleMcpInstance | null>(null);
  const [exportingInstanceBundle, setExportingInstanceBundle] = useState(false);
  const [restoreSource, setRestoreSource] =
    useState<McpBundleImportSource | null>(null);

  const saveInstanceMutation = useMutation({
    mutationFn: (values: InstanceFormValues) => {
      if (editingInstance) {
        return updateSettingsMcpInstance(
          editingInstance.instance_id,
          values,
          csrfToken
        );
      }
      return createSettingsMcpInstance(values, csrfToken);
    },
    onSuccess: async () => {
      notifyWebMcpRegistrationsChanged();
      message.success(i18nText('settings', 'auto.mcp_saved'));
      setInstanceModalOpen(false);
      setEditingInstance(null);
      await queryClient.invalidateQueries({
        queryKey: settingsMcpCatalogQueryKey
      });
    }
  });
  const deleteInstanceMutation = useMutation({
    mutationFn: (instanceId: string) =>
      deleteSettingsMcpInstance(instanceId, csrfToken),
    onSuccess: async () => {
      notifyWebMcpRegistrationsChanged();
      message.success(i18nText('settings', 'auto.mcp_deleted'));
      await queryClient.invalidateQueries({
        queryKey: settingsMcpCatalogQueryKey
      });
    }
  });
  const copyInstanceMutation = useMutation({
    mutationFn: ({
      sourceInstanceId,
      values
    }: {
      sourceInstanceId: string;
      values: CopyInstanceFormValues;
    }) => copySettingsMcpInstance(sourceInstanceId, values, csrfToken),
    onSuccess: async () => {
      message.success(
        i18nText('settingsMcpManagement', 'auto.mcp_instance_copied')
      );
      setCopyingInstance(null);
      copyInstanceForm.resetFields();
      await queryClient.invalidateQueries({
        queryKey: settingsMcpCatalogQueryKey
      });
    }
  });
  async function handleExportInstanceBundle(
    values: ExportSettingsMcpBundleBody
  ) {
    if (!bundleExportInstance) return;
    setExportingInstanceBundle(true);
    try {
      const response = await exportSettingsMcpInstanceBundle(
        bundleExportInstance.instance_id,
        values,
        csrfToken
      );
      downloadMcpBundle(response.blob, response.filename);
      setBundleExportInstance(null);
      message.success(
        i18nText('settingsMcpManagement', 'auto.mcp_bundle_export_ready')
      );
    } catch (error) {
      message.error(error instanceof Error ? error.message : String(error));
    } finally {
      setExportingInstanceBundle(false);
    }
  }

  const selectedDiscoveryPolicy = discoveryPolicyInstance
    ? catalog.discovery_policies.find(
        (policy) => policy.instance_record_id === discoveryPolicyInstance.id
      )
    : undefined;
  return {
    exportDefaults,
    instanceForm,
    copyInstanceForm,
    discoveryPolicyInstance,
    setDiscoveryPolicyInstance,
    clientConfigurationInstance,
    setClientConfigurationInstance,
    copyingInstance,
    setCopyingInstance,
    bundleExportInstance,
    setBundleExportInstance,
    exportingInstanceBundle,
    restoreSource,
    setRestoreSource,
    saveInstanceMutation,
    deleteInstanceMutation,
    copyInstanceMutation,
    handleExportInstanceBundle,
    selectedDiscoveryPolicy
  };
}
