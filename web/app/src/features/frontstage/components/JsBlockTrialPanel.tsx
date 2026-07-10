import {
  BLOCK_DATA_PERMISSIONS,
  type BlockDataPermission
} from '@1flowbase/page-protocol';
import type { JsBlockHostEffectHandlers } from '@1flowbase/page-runtime';
import { Alert, Button, Descriptions, Input, Space, Typography } from 'antd';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import {
  RestrictedBlockRuntimePreview,
  type RestrictedBlockRuntimeActionEvent
} from './RestrictedBlockRuntimePreview';
import type { NormalizedFrontstageBlockCatalogEntry } from '../lib/block-catalog';
import {
  createFrontstageRestrictedBlockRuntimeSession,
  type FrontstageRestrictedBlockRuntimeHostOptions,
  type FrontstageRestrictedBlockRuntimeSession
} from '../lib/frontstage-restricted-block-runtime-host';
import type { FrontstageBlockInstance } from '../lib/page-document';
import {
  createRestrictedBlockRunPlan,
  type RestrictedBlockLoaderLimits
} from '../lib/restricted-block-loader';
import type { RestrictedBlockRuntimeHostSnapshot } from '../lib/restricted-block-runtime-host';
import { i18nText } from '../../../shared/i18n/text';

export type JsBlockTrialPanelRuntimeSessionFactory = (
  options: FrontstageRestrictedBlockRuntimeHostOptions
) => FrontstageRestrictedBlockRuntimeSession;

export interface JsBlockTrialPanelProps {
  block: FrontstageBlockInstance | null | undefined;
  catalogEntry: NormalizedFrontstageBlockCatalogEntry | null | undefined;
  code: string;
  contextSnapshot: Record<string, unknown>;
  handlers?: JsBlockHostEffectHandlers;
  limits?: RestrictedBlockLoaderLimits;
  runtimeSnapshot?: RestrictedBlockRuntimeHostSnapshot;
  runtimeSessionFactory?: JsBlockTrialPanelRuntimeSessionFactory;
  onCodeChange?: (code: string) => void;
  onContextSnapshotChange?: (contextSnapshot: Record<string, unknown>) => void;
  onLimitsChange?: (limits: RestrictedBlockLoaderLimits) => void;
  onRuntimeAction?: (event: RestrictedBlockRuntimeActionEvent) => void;
}

type JsonDraftKind = 'context' | 'limits';

interface JsonDraftError {
  kind: JsonDraftKind;
  message: string;
}

interface ActiveRuntimeSession {
  session: FrontstageRestrictedBlockRuntimeSession;
  unsubscribe: () => void;
}

const dataPermissionSet = new Set<string>(BLOCK_DATA_PERMISSIONS);

function stringifyDraft(value: unknown): string {
  return JSON.stringify(value ?? {}, null, 2);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function parseJsonObject(
  value: string,
  label: string
):
  | { ok: true; value: Record<string, unknown> }
  | { ok: false; message: string } {
  try {
    const parsed = JSON.parse(value) as unknown;
    if (!isRecord(parsed)) {
      return { ok: false, message: i18nText("frontstage", "auto.json_object_required", { value1: label }) };
    }
    return { ok: true, value: parsed };
  } catch {
    return { ok: false, message: i18nText("frontstage", "auto.invalid_json", { value1: label }) };
  }
}

function formatList(values: readonly unknown[] | undefined): string {
  if (!values || values.length === 0) {
    return i18nText("frontstage", "auto.none");
  }
  return values.map(String).join(', ');
}

function formatKeys(value: Record<string, unknown>): string {
  return formatList(Object.keys(value));
}

function formatNumber(value: number | undefined): string {
  return typeof value === 'number' ? String(value) : i18nText("frontstage", "auto.not_set");
}

function toRuntimeLimitsDraft(
  value: Record<string, unknown>
):
  | { ok: true; value: RestrictedBlockLoaderLimits }
  | { ok: false; message: string } {
  const timeoutMs = readPositiveNumber(value, 'timeoutMs');
  if (timeoutMs === null) {
    return {
      ok: false,
      message: i18nText("frontstage", "auto.timeout_positive")
    };
  }

  const maxRenderDepth = readOptionalPositiveNumber(value, 'maxRenderDepth');
  if (!maxRenderDepth.ok) {
    return { ok: false, message: i18nText("frontstage", "auto.max_render_depth_positive") };
  }

  const maxRenderNodes = readOptionalPositiveNumber(value, 'maxRenderNodes');
  if (!maxRenderNodes.ok) {
    return { ok: false, message: i18nText("frontstage", "auto.max_render_nodes_positive") };
  }

  const maxEventChainDepth = readOptionalPositiveNumber(
    value,
    'maxEventChainDepth'
  );
  if (!maxEventChainDepth.ok) {
    return {
      ok: false,
      message: i18nText("frontstage", "auto.max_event_chain_depth_positive")
    };
  }

  const allowedActions = readStringArray(value, 'allowedActions');
  if (!allowedActions.ok) {
    return { ok: false, message: i18nText("frontstage", "auto.allowed_actions_string_array") };
  }

  const allowedEvents = readStringArray(value, 'allowedEvents');
  if (!allowedEvents.ok) {
    return { ok: false, message: i18nText("frontstage", "auto.allowed_events_string_array") };
  }

  const allowedQueries = readStringArray(value, 'allowedQueries');
  if (!allowedQueries.ok) {
    return {
      ok: false,
      message: i18nText("frontstage", "auto.allowed_data_models_string_array")
    };
  }

  const allowedDataOperations = readDataPermissions(
    value,
    'allowedDataOperations'
  );
  if (!allowedDataOperations.ok) {
    return {
      ok: false,
      message: i18nText("frontstage", "auto.allowed_data_operations_invalid")
    };
  }

  const limits: RestrictedBlockLoaderLimits = { timeoutMs };
  if (maxRenderDepth.value !== undefined) {
    limits.maxRenderDepth = maxRenderDepth.value;
  }
  if (maxRenderNodes.value !== undefined) {
    limits.maxRenderNodes = maxRenderNodes.value;
  }
  if (maxEventChainDepth.value !== undefined) {
    limits.maxEventChainDepth = maxEventChainDepth.value;
  }
  if (allowedActions.value !== undefined) {
    limits.allowedActions = allowedActions.value;
  }
  if (allowedEvents.value !== undefined) {
    limits.allowedEvents = allowedEvents.value;
  }
  if (allowedQueries.value !== undefined) {
    limits.allowedQueries = allowedQueries.value;
  }
  if (allowedDataOperations.value !== undefined) {
    limits.allowedDataOperations = allowedDataOperations.value;
  }

  return { ok: true, value: limits };
}

function readPositiveNumber(
  value: Record<string, unknown>,
  key: string
): number | null {
  const nextValue = value[key];
  return typeof nextValue === 'number' &&
    Number.isFinite(nextValue) &&
    nextValue > 0
    ? nextValue
    : null;
}

function readOptionalPositiveNumber(
  value: Record<string, unknown>,
  key: string
): { ok: true; value?: number } | { ok: false } {
  if (value[key] === undefined) {
    return { ok: true };
  }

  const nextValue = readPositiveNumber(value, key);
  return nextValue === null ? { ok: false } : { ok: true, value: nextValue };
}

function readStringArray(
  value: Record<string, unknown>,
  key: string
): { ok: true; value?: string[] } | { ok: false } {
  const nextValue = value[key];
  if (nextValue === undefined) {
    return { ok: true };
  }

  return Array.isArray(nextValue) &&
    nextValue.every((item): item is string => typeof item === 'string')
    ? { ok: true, value: nextValue }
    : { ok: false };
}

function readDataPermissions(
  value: Record<string, unknown>,
  key: string
): { ok: true; value?: BlockDataPermission[] } | { ok: false } {
  const nextValue = value[key];
  if (nextValue === undefined) {
    return { ok: true };
  }

  return Array.isArray(nextValue) &&
    nextValue.every(
      (item): item is BlockDataPermission =>
        typeof item === 'string' && dataPermissionSet.has(item)
    )
    ? { ok: true, value: nextValue }
    : { ok: false };
}

export function JsBlockTrialPanel({
  block,
  catalogEntry,
  code,
  contextSnapshot,
  handlers,
  limits,
  runtimeSnapshot,
  runtimeSessionFactory = createFrontstageRestrictedBlockRuntimeSession,
  onCodeChange,
  onContextSnapshotChange,
  onLimitsChange,
  onRuntimeAction
}: JsBlockTrialPanelProps) {
  const [contextDraft, setContextDraft] = useState(() =>
    stringifyDraft(contextSnapshot)
  );
  const [limitsDraft, setLimitsDraft] = useState(() => stringifyDraft(limits));
  const [draftError, setDraftError] = useState<JsonDraftError | null>(null);
  const [internalRuntimeSnapshot, setInternalRuntimeSnapshot] =
    useState<RestrictedBlockRuntimeHostSnapshot | null>(null);
  const activeRuntimeSessionRef = useRef<ActiveRuntimeSession | null>(null);

  const disposeActiveRuntimeSession = useCallback(
    (options: { updateSnapshot?: boolean } = {}) => {
      const activeRuntimeSession = activeRuntimeSessionRef.current;
      if (!activeRuntimeSession) {
        return null;
      }

      activeRuntimeSessionRef.current = null;
      activeRuntimeSession.unsubscribe();
      const snapshot = activeRuntimeSession.session.dispose();
      if (options.updateSnapshot !== false) {
        setInternalRuntimeSnapshot(snapshot);
      }
      return snapshot;
    },
    []
  );

  useEffect(() => {
    setContextDraft(stringifyDraft(contextSnapshot));
  }, [contextSnapshot]);

  useEffect(() => {
    setLimitsDraft(stringifyDraft(limits));
  }, [limits]);

  const runPlan = useMemo(() => {
    if (!block || !catalogEntry) {
      return null;
    }

    return createRestrictedBlockRunPlan({
      block,
      catalogEntry,
      code,
      contextSnapshot,
      limits
    });
  }, [block, catalogEntry, code, contextSnapshot, limits]);

  useEffect(
    () => () => {
      disposeActiveRuntimeSession({ updateSnapshot: false });
    },
    [disposeActiveRuntimeSession]
  );

  useEffect(() => {
    if (!runPlan?.ok) {
      disposeActiveRuntimeSession();
    }
  }, [disposeActiveRuntimeSession, runPlan]);

  function applyContextDraft() {
    const parsed = parseJsonObject(contextDraft, 'Context snapshot');
    if (!parsed.ok) {
      setDraftError({ kind: 'context', message: parsed.message });
      return;
    }

    setDraftError(null);
    onContextSnapshotChange?.(parsed.value);
  }

  function applyLimitsDraft() {
    const parsed = parseJsonObject(limitsDraft, 'Runtime limits');
    if (!parsed.ok) {
      setDraftError({ kind: 'limits', message: parsed.message });
      return;
    }

    const runtimeLimits = toRuntimeLimitsDraft(parsed.value);
    if (!runtimeLimits.ok) {
      setDraftError({ kind: 'limits', message: runtimeLimits.message });
      return;
    }

    setDraftError(null);
    onLimitsChange?.(runtimeLimits.value);
  }

  function runRuntimeSession() {
    if (!runPlan?.ok) {
      return;
    }

    disposeActiveRuntimeSession({ updateSnapshot: false });

    const runtimeOptions: FrontstageRestrictedBlockRuntimeHostOptions = {
      runPlan
    };
    if (handlers) {
      runtimeOptions.handlers = handlers;
    }

    const session = runtimeSessionFactory(runtimeOptions);
    const unsubscribe = session.subscribe((snapshot) => {
      setInternalRuntimeSnapshot(snapshot);
    });
    activeRuntimeSessionRef.current = { session, unsubscribe };
    setInternalRuntimeSnapshot(session.run());
  }

  function stopRuntimeSession() {
    disposeActiveRuntimeSession();
  }

  const canStopRuntimeSession = activeRuntimeSessionRef.current !== null;
  const activeRuntimeSnapshot = internalRuntimeSnapshot ?? runtimeSnapshot;

  if (!block) {
    return (
      <Alert
        type="info"
        showIcon
        message={i18nText("frontstage", "auto.select_block")}
        description={i18nText("frontstage", "auto.js_block_trial_requires_selected_block")}
      />
    );
  }

  if (!catalogEntry) {
    return (
      <Alert
        type="warning"
        showIcon
        message={i18nText("frontstage", "auto.missing_block_catalog_entry")}
        description={i18nText("frontstage", "auto.no_matching_block_catalog_entry")}
      />
    );
  }

  return (
    <Space direction="vertical" size="small" style={{ width: '100%' }}>
      <Typography.Title level={5} style={{ margin: 0 }}>
        {i18nText("frontstage", "auto.js_block_trial_panel")}</Typography.Title>

      <Space direction="vertical" size="small" style={{ width: '100%' }}>
        <Typography.Text strong>{i18nText("frontstage", "auto.js_code")}</Typography.Text>
        <Input.TextArea
          aria-label={i18nText("frontstage", "auto.js_code")}
          value={code}
          rows={5}
          readOnly={!onCodeChange}
          onChange={(event) => onCodeChange?.(event.target.value)}
        />
      </Space>

      <Space direction="vertical" size="small" style={{ width: '100%' }}>
        <Typography.Text strong>{i18nText("frontstage", "auto.context_snapshot")}</Typography.Text>
        <Input.TextArea
          aria-label={i18nText("frontstage", "auto.context_snapshot")}
          value={contextDraft}
          rows={4}
          onChange={(event) => setContextDraft(event.target.value)}
        />
        <Button
          size="small"
          disabled={!onContextSnapshotChange}
          onClick={applyContextDraft}
        >
          {i18nText("frontstage", "auto.update_context")}</Button>
      </Space>

      <Space direction="vertical" size="small" style={{ width: '100%' }}>
        <Typography.Text strong>{i18nText("frontstage", "auto.runtime_limits")}</Typography.Text>
        <Input.TextArea
          aria-label={i18nText("frontstage", "auto.runtime_limits")}
          value={limitsDraft}
          rows={4}
          onChange={(event) => setLimitsDraft(event.target.value)}
        />
        <Button
          size="small"
          disabled={!onLimitsChange}
          onClick={applyLimitsDraft}
        >
          {i18nText("frontstage", "auto.update_limits")}</Button>
      </Space>

      {draftError ? (
        <Alert
          type="error"
          showIcon
          message={
            draftError.kind === 'context'
              ? i18nText("frontstage", "auto.context_update_failed")
              : i18nText("frontstage", "auto.limits_update_failed")
          }
          description={draftError.message}
        />
      ) : null}

      {runPlan?.ok ? (
        <Space direction="vertical" size="small" style={{ width: '100%' }}>
          <Alert type="success" showIcon message={i18nText("frontstage", "auto.run_plan_generated")} />
          <Space size="small" wrap>
            <Button
              aria-label={i18nText("frontstage", "auto.run")}
              size="small"
              type="primary"
              onClick={runRuntimeSession}
            >
              {canStopRuntimeSession ? i18nText("frontstage", "auto.rerun") : i18nText("frontstage", "auto.run")}
            </Button>
            <Button
              aria-label={i18nText("frontstage", "auto.stop")}
              size="small"
              disabled={!canStopRuntimeSession}
              onClick={stopRuntimeSession}
            >
              {i18nText("frontstage", "auto.stop")}</Button>
          </Space>
          <Descriptions
            bordered
            size="small"
            column={1}
            title={i18nText("frontstage", "auto.request_summary")}
            items={[
              {
                key: 'requestId',
                label: i18nText("frontstage", "auto.request_id"),
                children: runPlan.request.requestId
              },
              {
                key: 'blockId',
                label: i18nText("frontstage", "auto.block_id"),
                children: runPlan.request.blockId
              },
              {
                key: 'sourceLength',
                label: i18nText("frontstage", "auto.source_length"),
                children: i18nText("frontstage", "auto.char_count", {
                  value1: String(runPlan.request.source.length)
                })
              },
              {
                key: 'timeout',
                label: i18nText("frontstage", "auto.timeout"),
                children: `${runPlan.request.limits.timeoutMs}ms`
              },
              {
                key: 'props',
                label: i18nText("frontstage", "auto.props_keys"),
                children: formatKeys(runPlan.request.props)
              },
              {
                key: 'context',
                label: i18nText("frontstage", "auto.context_keys"),
                children: formatKeys(runPlan.request.contextSnapshot)
              }
            ]}
          />
          <Descriptions
            bordered
            size="small"
            column={1}
            title={i18nText("frontstage", "auto.schema_validation_options")}
            data-testid="js-block-trial-schema-options"
            items={[
              {
                key: 'maxDepth',
                label: i18nText("frontstage", "auto.max_depth"),
                children: formatNumber(runPlan.schemaValidationOptions.maxDepth)
              },
              {
                key: 'maxNodes',
                label: i18nText("frontstage", "auto.max_nodes"),
                children: formatNumber(runPlan.schemaValidationOptions.maxNodes)
              },
              {
                key: 'data',
                label: i18nText("frontstage", "auto.data_permissions"),
                children: formatList(
                  runPlan.schemaValidationOptions.allowedDataPermissions
                )
              },
              {
                key: 'actions',
                label: i18nText("frontstage", "auto.actions"),
                children: formatList(
                  runPlan.schemaValidationOptions.allowedActions
                )
              },
              {
                key: 'events',
                label: i18nText("frontstage", "auto.events"),
                children: formatList(runPlan.schemaValidationOptions.allowedEvents)
              }
            ]}
          />
          <Descriptions
            bordered
            size="small"
            column={1}
            title={i18nText("frontstage", "auto.mediator_policy")}
            data-testid="js-block-trial-mediator-policy"
            items={[
              {
                key: 'actions',
                label: i18nText("frontstage", "auto.actions"),
                children: formatList(runPlan.mediatorPolicy.allowedActions)
              },
              {
                key: 'events',
                label: i18nText("frontstage", "auto.events"),
                children: formatList(runPlan.mediatorPolicy.allowedEvents)
              },
              {
                key: 'models',
                label: i18nText("frontstage", "auto.data_models"),
                children: formatList(runPlan.mediatorPolicy.allowedQueries)
              },
              {
                key: 'operations',
                label: i18nText("frontstage", "auto.data_operations"),
                children: formatList(runPlan.mediatorPolicy.allowedDataOperations)
              },
              {
                key: 'maxEventChainDepth',
                label: i18nText("frontstage", "auto.max_event_chain_depth"),
                children: formatNumber(runPlan.mediatorPolicy.maxEventChainDepth)
              }
            ]}
          />
        </Space>
      ) : null}

      {runPlan && !runPlan.ok ? (
        <Space direction="vertical" size="small" style={{ width: '100%' }}>
          <Alert
            type="error"
            showIcon
            message={i18nText("frontstage", "auto.run_plan_rejected")}
            description={runPlan.message}
          />
          <Descriptions
            bordered
            size="small"
            column={1}
            title={i18nText("frontstage", "auto.rejection")}
            items={[
              { key: 'code', label: i18nText("frontstage", "auto.code"), children: runPlan.code },
              { key: 'path', label: i18nText("frontstage", "auto.path"), children: runPlan.path },
              { key: 'blockId', label: i18nText("frontstage", "auto.block_id"), children: runPlan.blockId },
              {
                key: 'catalogId',
                label: i18nText("frontstage", "auto.catalog_id"),
                children: runPlan.catalogId
              }
            ]}
          />
        </Space>
      ) : null}

      {activeRuntimeSnapshot ? (
        <RestrictedBlockRuntimePreview
          snapshot={activeRuntimeSnapshot}
          onAction={onRuntimeAction}
        />
      ) : null}
    </Space>
  );
}
