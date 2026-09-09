import { useState, type ComponentProps } from 'react';
import Conversations from '@ant-design/x/es/conversations';
import { Alert, Empty, Form } from 'antd';
import { useQuery } from '@tanstack/react-query';
import { AssistantSettingsModal } from './AssistantSettingsModal';
import {
  agentFlowMcpInstanceOptionsQueryKey,
  fetchAgentFlowMcpInstanceOptions
} from '../../api/mcp-instance-options';
import type {
  FlowAuthoringDocument,
  FlowStartModelDescriptor
} from '@1flowbase/flow-schema';
import {
  AssistantPanelPlugin,
  type AssistantPanelRuntime
} from './AssistantPanelPlugin';
import { AgentFlowDockPanel } from '../editor/AgentFlowDockPanel';
import { i18nText } from '../../../../shared/i18n/text';

type DraftAssistantPreviewProps = Omit<
  ComponentProps<typeof AssistantPanelPlugin>,
  'runtime' | 'activity' | 'history' | 'settingsAction' | 'subtitle'
> & {
  document: FlowAuthoringDocument;
  applicationName: string;
  applicationId: string;
  mcp_instance_ids: string[];
  onChangeMcpInstanceIds: (ids: string[]) => void;
};

export function DraftAssistantPreview({
  document,
  applicationName,
  applicationId,
  mcp_instance_ids,
  onChangeMcpInstanceIds,
  ...conversation
}: DraftAssistantPreviewProps) {
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [form] = Form.useForm();
  const mcpOptions = useQuery({
    queryKey: agentFlowMcpInstanceOptionsQueryKey,
    queryFn: fetchAgentFlowMcpInstanceOptions,
    enabled: settingsOpen
  });
  const [historyOpen, setHistoryOpen] = useState(false);
  const start = document.graph.nodes.find((node) => node.type === 'start');
  const models = (
    (start?.config.model_list ?? []) as Array<FlowStartModelDescriptor | string>
  ).map((model) => (typeof model === 'string' ? { id: model } : model));
  const modelField = conversation.runContext.fields.find(
    (field) => field.key === 'model'
  );
  const reasoningField = conversation.runContext.fields.find(
    (field) => field.key === 'reasoning_effort'
  );
  const runs = conversation.messages.filter(
    (message) => message.role === 'assistant' && message.runId
  );
  const latestRun = runs.at(-1);
  const runtime: AssistantPanelRuntime = {
    run_capabilities: {
      model_selection_enabled: Boolean(modelField && models.length),
      reasoning_effort_enabled: Boolean(
        reasoningField &&
        document.graph.nodes.some(
          (node) =>
            node.type === 'llm' &&
            (
              node.config.external_reasoning_policy as
                | { follow_external_reasoning?: boolean }
                | undefined
            )?.follow_external_reasoning
        )
      ),
      models: models.map((model) => ({
        id: model.id,
        name: model.name ?? null,
        context_window: model.context_window ?? null,
        reasoning_efforts: model.reasoning?.supported_efforts ?? [],
        default_reasoning_effort: model.reasoning?.default_effort ?? null
      }))
    },
    preference: {
      model:
        typeof modelField?.value === 'string' && modelField.value
          ? modelField.value
          : null,
      reasoning_effort:
        typeof reasoningField?.value === 'string' && reasoningField.value
          ? reasoningField.value
          : null
    },
    onChangePreference(patch) {
      if (modelField)
        conversation.onChangeRunContextValue(
          modelField.nodeId,
          modelField.key,
          patch.model ?? ''
        );
      if (reasoningField)
        conversation.onChangeRunContextValue(
          reasoningField.nodeId,
          reasoningField.key,
          patch.reasoning_effort ?? ''
        );
    }
  };

  return (
    <>
      {historyOpen ? (
        <AgentFlowDockPanel
          title={i18nText('appShell', 'auto.assistant_history')}
          onClose={() => setHistoryOpen(false)}
        >
          {runs.length ? (
            <Conversations
              items={runs.map((run) => ({
                key: run.id,
                label: run.content || run.runId
              }))}
              onActiveChange={(id) => {
                const run = runs.find((item) => item.id === id);
                if (run) conversation.onOpenMessageLog?.(run);
              }}
            />
          ) : (
            <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} />
          )}
        </AgentFlowDockPanel>
      ) : (
        <AssistantPanelPlugin
          {...conversation}
          subtitle={applicationName}
          runtime={runtime}
          activity={{
            disabled: !latestRun,
            onClick: () => {
              if (latestRun) conversation.onOpenMessageLog?.(latestRun);
            }
          }}
          history={{ onClick: () => setHistoryOpen(true) }}
          settingsAction={{
            onClick: () => {
              form.setFieldsValue({
                application_id: applicationId,
                mcp_instance_ids
              });
              setSettingsOpen(true);
            }
          }}
        />
      )}
      <AssistantSettingsModal
        form={form}
        open={settingsOpen}
        applications={[
          { application_id: applicationId, name: applicationName }
        ]}
        applicationFixed
        mcpInstances={mcpOptions.data ?? []}
        mcpLoading={mcpOptions.isPending}
        okButtonProps={{ disabled: mcpOptions.isPending || mcpOptions.isError }}
        onCancel={() => setSettingsOpen(false)}
        onOk={() => {
          void form.validateFields().then((values) => {
            onChangeMcpInstanceIds(values.mcp_instance_ids);
            setSettingsOpen(false);
          });
        }}
      >
        {mcpOptions.isError ? (
          <Alert
            type="error"
            title={i18nText('agentFlow', 'auto.loading_failed')}
          />
        ) : null}
      </AssistantSettingsModal>
    </>
  );
}
