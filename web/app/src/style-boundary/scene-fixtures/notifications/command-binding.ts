import type {
  CommunicationBindingInput,
  CommunicationBindingRecord,
  CommunicationCommandView,
  CommunicationOptions,
  NotificationInstance,
  NotificationType
} from '@1flowbase/api-client';

export const commandBindingType: NotificationType = {
  type_code: 'team-chat',
  display_name: '团队聊天',
  description: '团队聊天消息接收与回复',
  config_schema: {},
  config_form: { schema_version: '1.0.0', fields: [] },
  input_schema: {},
  input_form: { schema_version: '1.0.0', fields: [] },
  output_schema: {},
  receive: {
    default_system_prompt: '协助处理团队工作与聊天消息。',
    mcp_tools: []
  },
  contribution_id: 'style.chat',
  installation_id: 'style-chat-plugin',
  available: true,
  unavailable_reason: null
};
export const commandBindingInstance: NotificationInstance = {
  id: 'style-command-channel',
  workspace_id: 'workspace-1',
  type_code: commandBindingType.type_code,
  installation_id: commandBindingType.installation_id,
  display_name: '团队聊天指令配置',
  status: 'enabled',
  authorization: null,
  config_json: {},
  configured_secret_fields: [],
  available: true,
  unavailable_reason: null,
  created_at: '2026-10-28T00:00:00Z',
  updated_at: '2026-10-28T00:00:00Z'
};
const declarations: CommunicationCommandView[] = [
  ...(['new', 'model', 'help'] as const).map((name) => ({
    command_id: `host/${name}`,
    name,
    description: name,
    usage: name === 'model' ? '/model [model]' : `/${name}`,
    source_kind: 'builtin' as const,
    source_name: '1flowbase',
    installation_id: null,
    enabled: true,
    available: true,
    unavailable_reason: null
  })),
  {
    command_id: 'acme/reporting/status',
    name: 'status',
    description: '查看项目当前进度与需要跟进的事项。说明由已安装插件提供。',
    usage: '/status [project]',
    source_kind: 'plugin',
    source_name: '团队项目进度与任务报告',
    installation_id: 'style-reporting-plugin',
    enabled: false,
    available: true,
    unavailable_reason: null
  },
  {
    command_id: 'acme/archive/export',
    name: 'acme/archive/export',
    description: 'Command declaration is no longer installed',
    usage: '',
    source_kind: 'plugin',
    source_name: 'acme/archive',
    installation_id: null,
    enabled: true,
    available: false,
    unavailable_reason: 'command_not_installed'
  }
];

export function createStyleBoundaryCommandBindingResponse() {
  let binding: CommunicationBindingRecord = {
    workspace_id: 'workspace-1',
    instance_id: commandBindingInstance.id,
    actor_user_id: 'user-1',
    plugin_mcp_instance_id: 'style-chat-tools',
    revision: 1,
    application_id: 'style-chat-application',
    model: 'model-1',
    reasoning_effort: null,
    file_table_id: null,
    mcp_instance_ids: ['style-chat-tools'],
    system_prompt: '协助处理团队工作与聊天消息。',
    command_settings: declarations.map(({ command_id, enabled }) => ({
      command_id,
      enabled
    }))
  };
  const catalog = (application_id: string | null, instance_id: string | null) =>
    declarations
      .filter(
        (command) => instance_id || command.command_id !== 'acme/archive/export'
      )
      .map((command) => ({
        ...command,
        enabled: instance_id
          ? (binding.command_settings.find(
              (setting) => setting.command_id === command.command_id
            )?.enabled ?? command.enabled)
          : command.enabled,
        ...(command.command_id === 'host/model' &&
        application_id !== 'style-chat-application'
          ? {
              available: false,
              unavailable_reason: 'model_selection_unavailable'
            }
          : {})
      }));
  return (url: URL, init?: RequestInit): Response | null => {
    const method = init?.method ?? 'GET';
    let data: unknown;
    if (
      url.pathname ===
      `/api/console/communications/instances/${commandBindingInstance.id}/binding`
    ) {
      if (method === 'PUT') {
        const input: CommunicationBindingInput = JSON.parse(String(init?.body));
        binding = { ...binding, ...input, revision: binding.revision + 1 };
        data = binding;
      } else if (method === 'GET') {
        data = {
          binding,
          plugin_mcp_instance_id: binding.plugin_mcp_instance_id,
          default_system_prompt:
            commandBindingType.receive!.default_system_prompt,
          commands: catalog(binding.application_id, binding.instance_id)
        };
      }
    } else if (
      url.pathname === '/api/console/communications/options' &&
      method === 'GET'
    ) {
      const application_id = url.searchParams.get('application_id');
      const options: CommunicationOptions = {
        published_agent_flows: [
          { application_id: 'style-chat-application', name: '团队工作助手' },
          { application_id: 'style-fixed-application', name: '固定模型应用' }
        ],
        enabled_mcp_instances: [
          { instance_id: 'style-chat-tools', name: '团队聊天工具' }
        ],
        run_capabilities: {
          model_selection_enabled: application_id === 'style-chat-application',
          reasoning_effort_enabled: false,
          models: [
            {
              id: 'model-1',
              name: '团队默认模型',
              context_window: null,
              reasoning_efforts: [],
              default_reasoning_effort: null
            }
          ]
        },
        commands: catalog(application_id, url.searchParams.get('instance_id'))
      };
      data = options;
    } else if (
      url.pathname === '/api/console/settings/files/tables' &&
      method === 'GET'
    ) {
      data = [];
    } else if (
      url.pathname ===
        `/api/console/communications/instances/${commandBindingInstance.id}` &&
      method === 'PATCH'
    ) {
      data = { ...commandBindingInstance, ...JSON.parse(String(init?.body)) };
    }
    return data === undefined
      ? null
      : new Response(JSON.stringify({ data, meta: null }), {
          headers: { 'content-type': 'application/json' }
        });
  };
}
