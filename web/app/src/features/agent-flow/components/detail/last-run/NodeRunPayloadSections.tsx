import { useState, type ReactNode } from 'react';
import { Alert, Button, Spin } from 'antd';
import { CollapseShell } from '../../../../../shared/ui/collapse-shell/CollapseShell';
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
  onLoadArtifacts,
  onLoadSection,
  processAction
}: {
  onLoadSection?: (section: NodeRunPayloadSection) => Promise<unknown>;
  processAction?: ReactNode;
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
  if (onLoadSection) {
    return (
      <>
        {(['input_payload', 'debug_payload', 'output_payload'] as const).map(
          (section) => (
            <LazyNodeRunPayloadSection
              key={section}
              section={section}
              load={onLoadSection}
              action={section === 'debug_payload' ? processAction : undefined}
              onLoadArtifact={onLoadArtifact}
              onLoadArtifacts={onLoadArtifacts}
            />
          )
        )}
      </>
    );
  }
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
      {processAction}
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

export type NodeRunPayloadSection =
  | 'input_payload'
  | 'debug_payload'
  | 'output_payload';

function LazyNodeRunPayloadSection({
  section,
  load,
  action,
  onLoadArtifact,
  onLoadArtifacts
}: {
  section: NodeRunPayloadSection;
  load: (section: NodeRunPayloadSection) => Promise<unknown>;
  action?: ReactNode;
  onLoadArtifact?: (artifactRef: string) => Promise<unknown>;
  onLoadArtifacts?: RuntimeDebugArtifactBatchLoader;
}) {
  const [open, setOpen] = useState(false);
  const [state, setState] = useState<{
    status: 'idle' | 'loading' | 'ready' | 'error';
    value?: unknown;
  }>({ status: 'idle' });
  const title =
    section === 'input_payload'
      ? i18nText('agentFlow', 'auto.input')
      : section === 'debug_payload'
        ? i18nText('agentFlow', 'auto.data_processing')
        : i18nText('agentFlow', 'auto.outputs');
  const fetchSection = async () => {
    if (state.status === 'loading' || state.status === 'ready') return;
    setState({ status: 'loading' });
    try {
      setState({ status: 'ready', value: await load(section) });
    } catch {
      setState({ status: 'error' });
    }
  };
  return (
    <CollapseShell
      variant="compact"
      activeKey={open ? [section] : []}
      onChange={(keys) => {
        const next = keys.includes(section);
        setOpen(next);
        if (next) void fetchSection();
      }}
      items={[
        {
          key: section,
          header: (
            <span
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                width: '100%'
              }}
            >
              {title}
              <span onClick={(event) => event.stopPropagation()}>{action}</span>
            </span>
          ),
          children:
            state.status === 'ready' ? (
              <RuntimeDebugPayloadBlock
                title={title}
                payload={state.value}
                onLoadArtifact={onLoadArtifact}
                onLoadArtifacts={onLoadArtifacts}
              />
            ) : state.status === 'error' ? (
              <Alert
                type="error"
                showIcon
                title={i18nText('agentFlow', 'auto.loading_failed')}
                action={
                  <Button size="small" onClick={() => void fetchSection()}>
                    {i18nText('agentFlow', 'auto.retry')}
                  </Button>
                }
              />
            ) : (
              <Spin />
            )
        }
      ]}
    />
  );
}
