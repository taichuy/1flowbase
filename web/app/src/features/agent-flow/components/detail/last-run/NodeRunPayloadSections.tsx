import {
  RuntimeDebugPayloadBlock,
  type RuntimeDebugArtifactBatchLoader
} from './runtime-debug-payload';
import { i18nText } from '../../../../../shared/i18n/text';

type ConsoleLogLevel = 'info' | 'warn' | 'error';

interface ConsoleLogEntryView {
  level: ConsoleLogLevel;
  message: string;
  args: unknown[];
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value && typeof value === 'object' && !Array.isArray(value));
}

function normalizeConsoleLogLevel(value: unknown): ConsoleLogLevel {
  if (value === 'warn' || value === 'error') {
    return value;
  }

  return 'info';
}

function formatConsoleLogMessage(value: unknown) {
  if (typeof value === 'string') {
    return value;
  }

  if (Array.isArray(value)) {
    return value
      .map((item) => {
        if (typeof item === 'string') {
          return item;
        }

        const serialized = JSON.stringify(item);
        return serialized ?? String(item);
      })
      .join(' ');
  }

  return '';
}

function readConsoleLogs(debugPayload: unknown): ConsoleLogEntryView[] {
  if (!isRecord(debugPayload) || !Array.isArray(debugPayload.console_logs)) {
    return [];
  }

  const consoleLogs: ConsoleLogEntryView[] = [];

  for (const entry of debugPayload.console_logs) {
    if (!isRecord(entry)) {
      continue;
    }

    const args = Array.isArray(entry.args) ? entry.args : [];
    consoleLogs.push({
      level: normalizeConsoleLogLevel(entry.level),
      message: formatConsoleLogMessage(entry.message || args),
      args
    });
  }

  return consoleLogs;
}

function pickProcessPayload(debugPayload: unknown) {
  if (!isRecord(debugPayload)) {
    return {};
  }

  const consoleLogs = readConsoleLogs(debugPayload);

  if (consoleLogs.length === 0) {
    return debugPayload;
  }

  return {
    ...debugPayload,
    console_logs: consoleLogs
  };
}

function runtimePayloadHasValue(value: unknown): boolean {
  if (value === null || value === undefined) {
    return false;
  }

  if (Array.isArray(value)) {
    return value.length > 0;
  }

  if (typeof value === 'object') {
    return Object.keys(value).length > 0;
  }

  return true;
}

export function NodeRunPayloadSections({
  inputPayload,
  debugPayload,
  outputPayload,
  errorPayload,
  includeDebugPayload = true,
  hideEmptyPayloads = false,
  defaultCollapsed = false,
  onLoadArtifact,
  onLoadArtifacts
}: {
  inputPayload: unknown;
  debugPayload: unknown;
  outputPayload: unknown;
  errorPayload?: unknown;
  includeDebugPayload?: boolean;
  hideEmptyPayloads?: boolean;
  defaultCollapsed?: boolean;
  onLoadArtifact?: (artifactRef: string) => Promise<unknown>;
  onLoadArtifacts?: RuntimeDebugArtifactBatchLoader;
}) {
  const processPayload = pickProcessPayload(debugPayload);
  const showInputPayload =
    !hideEmptyPayloads || runtimePayloadHasValue(inputPayload);
  const showDebugPayload =
    includeDebugPayload &&
    (!hideEmptyPayloads || runtimePayloadHasValue(processPayload));
  const showOutputPayload =
    !hideEmptyPayloads || runtimePayloadHasValue(outputPayload);
  const showErrorPayload = runtimePayloadHasValue(errorPayload);

  return (
    <>
      {showInputPayload ? (
        <RuntimeDebugPayloadBlock
          defaultCollapsed={defaultCollapsed}
          payload={inputPayload}
          title={i18nText('agentFlow', 'auto.input')}
          onLoadArtifact={onLoadArtifact}
          onLoadArtifacts={onLoadArtifacts}
        />
      ) : null}
      {showDebugPayload ? (
        <RuntimeDebugPayloadBlock
          defaultCollapsed={defaultCollapsed}
          payload={processPayload}
          title={i18nText('agentFlow', 'auto.data_processing')}
          onLoadArtifact={onLoadArtifact}
          onLoadArtifacts={onLoadArtifacts}
        />
      ) : null}
      {showOutputPayload ? (
        <RuntimeDebugPayloadBlock
          defaultCollapsed={defaultCollapsed}
          payload={outputPayload}
          title={i18nText('agentFlow', 'auto.outputs')}
          onLoadArtifact={onLoadArtifact}
          onLoadArtifacts={onLoadArtifacts}
        />
      ) : null}
      {showErrorPayload ? (
        <RuntimeDebugPayloadBlock
          defaultCollapsed={defaultCollapsed}
          payload={errorPayload}
          title={i18nText('agentFlow', 'auto.error')}
          onLoadArtifact={onLoadArtifact}
          onLoadArtifacts={onLoadArtifacts}
        />
      ) : null}
    </>
  );
}
