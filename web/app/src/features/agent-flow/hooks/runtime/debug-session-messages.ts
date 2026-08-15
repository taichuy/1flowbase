import type {
  AgentFlowDebugMessage,
  AgentFlowPageReference,
  AgentFlowRunContext,
  FlowDebugRunDetail,
  FlowDebugRunStreamEvent
} from '../../api/runtime';

let debugMessageIdSequence = 0;

export function createUserMessage(
  prompt: string,
  pageReferences?: AgentFlowPageReference[]
): AgentFlowDebugMessage {
  return {
    id: createDebugMessageId('user'),
    role: 'user',
    content: prompt,
    pageReferences,
    status: 'completed',
    runId: null,
    rawOutput: null,
    traceSummary: []
  };
}

export function createRunningAssistantMessage(): AgentFlowDebugMessage {
  return {
    id: createDebugMessageId('assistant-pending'),
    role: 'assistant',
    content: '',
    status: 'running',
    runId: null,
    rawOutput: null,
    traceSummary: []
  };
}

function createDebugMessageId(prefix: string) {
  const randomId =
    typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function'
      ? crypto.randomUUID()
      : `${Date.now().toString(36)}-${(debugMessageIdSequence += 1).toString(36)}`;

  return `${prefix}-${randomId}`;
}

export function resolvePrompt(
  runContext: AgentFlowRunContext,
  prompt: string | undefined
): string {
  if (typeof prompt === 'string') {
    return prompt;
  }

  const queryField = runContext.fields.find((field) => field.key === 'query');

  return typeof queryField?.value === 'string' ? queryField.value : '';
}

export function updateRunContextQuery(
  runContext: AgentFlowRunContext,
  prompt: string
): AgentFlowRunContext {
  return {
    ...runContext,
    fields: runContext.fields.map((field) =>
      field.key === 'query' ? { ...field, value: prompt } : field
    )
  };
}

export function clearRunContextQuery(
  runContext: AgentFlowRunContext
): AgentFlowRunContext {
  return updateRunContextQuery(runContext, '');
}

export function buildStreamEventDedupKeys(event: FlowDebugRunStreamEvent) {
  const keys: string[] = [];

  if (event.event_id) {
    keys.push(`eid:${event.event_id}`);
  }

  if ('run_id' in event && event.run_id && event.sequence !== undefined) {
    keys.push(`seq:${event.run_id}:${event.sequence}`);
  }

  return keys;
}

export function replaceAssistantMessage(
  currentMessages: AgentFlowDebugMessage[],
  nextMessage: AgentFlowDebugMessage,
  fallbackMessageId?: string | null
) {
  let replaced = false;
  const nextMessages = currentMessages.map((message) => {
    const matchedById = fallbackMessageId
      ? message.id === fallbackMessageId
      : false;
    const matchedByRunId =
      nextMessage.runId !== null && message.runId === nextMessage.runId;

    if (!matchedById && !matchedByRunId) {
      return message;
    }

    replaced = true;
    return {
      ...nextMessage,
      id: message.id
    };
  });

  return replaced ? nextMessages : [...nextMessages, nextMessage];
}

export function replaceAssistantMessageWithError(
  currentMessages: AgentFlowDebugMessage[],
  errorMessage: string,
  options?: {
    fallbackMessageId?: string | null;
    runId?: string | null;
  }
) {
  let replaced = false;
  const nextMessages = currentMessages.map((message) => {
    const matchedById = options?.fallbackMessageId
      ? message.id === options.fallbackMessageId
      : false;
    const matchedByRunId = options?.runId
      ? message.runId === options.runId
      : false;

    if (!matchedById && !matchedByRunId) {
      return message;
    }

    replaced = true;
    return {
      ...message,
      status: 'failed',
      content: errorMessage
    } satisfies AgentFlowDebugMessage;
  });

  if (replaced) {
    return nextMessages;
  }

  return [
    ...nextMessages,
    {
      id: createDebugMessageId('assistant-error'),
      role: 'assistant',
      content: errorMessage,
      status: 'failed',
      runId: options?.runId ?? null,
      rawOutput: null,
      traceSummary: []
    } satisfies AgentFlowDebugMessage
  ];
}

export function shouldPollRun(detail: FlowDebugRunDetail) {
  return detail.flow_run.status === 'running';
}
