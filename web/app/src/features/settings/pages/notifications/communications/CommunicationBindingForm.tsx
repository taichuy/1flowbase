import { forwardRef, useEffect, useImperativeHandle, useState } from 'react';
import { Alert, Form, Input, Select, Spin } from 'antd';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import {
  type CommunicationBindingInput,
  type NotificationType
} from '@1flowbase/api-client';
import {
  communicationBindingOptions,
  communicationOptions,
  communicationFileTablesOptions,
  getCommunicationBinding,
  putCommunicationBinding
} from '../../../../notifications/api/communications';
import { useAuthStore } from '../../../../../state/auth-store';
import { selectNavigationQueryScope } from '../../../../../state/navigation-query-scope';
export interface CommunicationBindingHandle {
  validate: () => boolean;
  save: (id: string) => Promise<void>;
}
export const CommunicationBindingForm = forwardRef<
  CommunicationBindingHandle,
  { instance_id?: string; receive: NonNullable<NotificationType['receive']> }
>(function CommunicationBindingForm({ instance_id, receive }, ref) {
  const { t } = useTranslation('notifications');
  const scope = useAuthStore(selectNavigationQueryScope);
  const csrfToken = useAuthStore((s) => s.csrfToken) ?? '';
  const [draft, setDraft] = useState<CommunicationBindingInput>({
    application_id: '',
    model: null,
    reasoning_effort: null,
    file_table_id: null,
    mcp_instance_ids: [],
    system_prompt: receive.default_system_prompt
  });
  const [dirty, setDirty] = useState(false);
  const [invalid, setInvalid] = useState(false);
  const binding = useQuery(communicationBindingOptions(scope, instance_id));
  const options = useQuery(
    communicationOptions(scope, draft.application_id || undefined)
  );
  const tables = useQuery(communicationFileTablesOptions(scope));
  useEffect(() => {
    if (dirty || !binding.data) return;
    const value = binding.data.binding;
    if (value)
      setDraft({
        application_id: value.application_id,
        model: value.model,
        reasoning_effort: value.reasoning_effort,
        file_table_id: value.file_table_id,
        mcp_instance_ids: value.mcp_instance_ids,
        system_prompt: value.system_prompt
      });
    else
      setDraft((d) => ({
        ...d,
        system_prompt:
          binding.data.default_system_prompt ?? receive.default_system_prompt,
        mcp_instance_ids: binding.data.plugin_mcp_instance_id
          ? [binding.data.plugin_mcp_instance_id]
          : []
      }));
  }, [binding.data, dirty, receive.default_system_prompt]);
  const change = (value: Partial<CommunicationBindingInput>) => {
    setDirty(true);
    setDraft((d) => ({ ...d, ...value }));
  };
  useImperativeHandle(ref, () => ({
    validate: () => {
      const valid =
        !!draft.application_id &&
        !options.isPending &&
        !options.isError &&
        (!instance_id || binding.isSuccess);
      setInvalid(!valid);
      return valid;
    },
    save: async (id) => {
      let value = draft;
      if (!instance_id) {
        const current = await getCommunicationBinding(id);
        if (current.plugin_mcp_instance_id)
          value = {
            ...draft,
            mcp_instance_ids: [
              ...new Set([
                ...draft.mcp_instance_ids,
                current.plugin_mcp_instance_id
              ])
            ]
          };
      }
      setDraft(value);
      await putCommunicationBinding(id, value, csrfToken);
    }
  }));
  const capabilities = options.data?.run_capabilities;
  const model = capabilities?.models.find((m) => m.id === draft.model);
  const mcpOptions =
    options.data?.enabled_mcp_instances.map((m) => ({
      value: m.instance_id,
      label: m.name
    })) ?? [];
  const pluginId = binding.data?.plugin_mcp_instance_id;
  if (pluginId && !mcpOptions.some((m) => m.value === pluginId))
    mcpOptions.push({ value: pluginId, label: t('channel_tools') });
  return (
    <Form layout="vertical" component={false}>
      {(options.isError || binding.isError || tables.isError) && (
        <Alert type="error" title={t('binding_load_failed')} />
      )}
      {instance_id && binding.isPending && <Spin />}
      <Form.Item
        label={t('application')}
        required
        validateStatus={invalid ? 'error' : undefined}
        help={invalid ? t('application_required') : undefined}
      >
        <Select
          aria-label={t('application')}
          value={draft.application_id || undefined}
          loading={options.isPending}
          options={options.data?.published_agent_flows.map((a) => ({
            value: a.application_id,
            label: a.name
          }))}
          onChange={(application_id) => {
            setInvalid(false);
            change({ application_id, model: null, reasoning_effort: null });
          }}
        />
      </Form.Item>
      <Form.Item label={t('model')}>
        <Select
          allowClear
          aria-label={t('model')}
          value={draft.model ?? undefined}
          disabled={!capabilities?.model_selection_enabled}
          options={capabilities?.models.map((m) => ({
            value: m.id,
            label: m.name ?? m.id
          }))}
          onChange={(model) =>
            change({ model: model ?? null, reasoning_effort: null })
          }
        />
      </Form.Item>
      <Form.Item label={t('reasoning_effort')}>
        <Select
          allowClear
          aria-label={t('reasoning_effort')}
          value={draft.reasoning_effort ?? undefined}
          disabled={!capabilities?.reasoning_effort_enabled || !model}
          options={model?.reasoning_efforts.map((value) => ({
            value,
            label: value
          }))}
          onChange={(reasoning_effort) =>
            change({ reasoning_effort: reasoning_effort ?? null })
          }
        />
      </Form.Item>
      <Form.Item label={t('file_table')}>
        <Select
          allowClear
          aria-label={t('file_table')}
          placeholder={t('default_file_table')}
          value={draft.file_table_id ?? undefined}
          loading={tables.isPending}
          options={tables.data?.map((table) => ({
            value: table.id,
            label: table.title
          }))}
          onChange={(file_table_id) =>
            change({ file_table_id: file_table_id ?? null })
          }
        />
      </Form.Item>
      <Form.Item label={t('mcp_instances')}>
        <Select
          mode="multiple"
          aria-label={t('mcp_instances')}
          value={draft.mcp_instance_ids}
          options={mcpOptions}
          onChange={(mcp_instance_ids) => change({ mcp_instance_ids })}
        />
      </Form.Item>
      <Form.Item label={t('system_prompt')}>
        <Input.TextArea
          rows={6}
          aria-label={t('system_prompt')}
          value={draft.system_prompt}
          onChange={(e) => change({ system_prompt: e.target.value })}
        />
      </Form.Item>
    </Form>
  );
});
