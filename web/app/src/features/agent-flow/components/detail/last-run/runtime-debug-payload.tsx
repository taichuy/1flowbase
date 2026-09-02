import LineHeightOutlined from '@ant-design/icons/es/icons/LineHeightOutlined';
import { App, Button, Space, Tag, Tooltip } from 'antd';
import { useMemo, useState } from 'react';

import { JsonPreviewBlock } from '../../../../../shared/ui/json-preview/JsonPreviewBlock';
import { i18nText } from '../../../../../shared/i18n/text';

interface RuntimeDebugArtifactLocation {
  artifactRef: string;
  path: string[];
}

export interface RuntimeDebugArtifactLoadResult {
  artifacts: Array<{
    artifact_ref: string;
    value: unknown;
  }>;
}

export type RuntimeDebugArtifactBatchLoader = (
  artifactRefs: string[]
) => Promise<RuntimeDebugArtifactLoadResult>;

function collectRuntimeDebugArtifactLocations(
  value: unknown,
  path: string[] = []
): RuntimeDebugArtifactLocation[] {
  if (!value || typeof value !== 'object') {
    return [];
  }

  if (Array.isArray(value)) {
    return value.flatMap((item, index) =>
      collectRuntimeDebugArtifactLocations(item, [...path, String(index)])
    );
  }

  const record = value as Record<string, unknown>;
  if (
    record.__runtime_debug_artifact === true &&
    typeof record.artifact_ref === 'string'
  ) {
    return [
      {
        artifactRef: record.artifact_ref,
        path
      }
    ];
  }

  return Object.entries(record).flatMap(([key, nestedValue]) =>
    collectRuntimeDebugArtifactLocations(nestedValue, [...path, key])
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value && typeof value === 'object' && !Array.isArray(value));
}

function replacePayloadAtPath(
  currentPayload: unknown,
  path: string[],
  fullPayload: unknown
): unknown {
  if (path.length === 0) {
    return fullPayload;
  }

  const [head, ...tail] = path;

  if (Array.isArray(currentPayload)) {
    const index = Number(head);

    if (
      !Number.isInteger(index) ||
      index < 0 ||
      index >= currentPayload.length
    ) {
      return currentPayload;
    }

    return currentPayload.map((item, itemIndex) =>
      itemIndex === index ? replacePayloadAtPath(item, tail, fullPayload) : item
    );
  }

  if (!isRecord(currentPayload) || !head) {
    return currentPayload;
  }

  return {
    ...currentPayload,
    [head]: replacePayloadAtPath(currentPayload[head], tail, fullPayload)
  };
}

function uniqueArtifactRefs(locations: RuntimeDebugArtifactLocation[]) {
  const refs: string[] = [];
  const seen = new Set<string>();

  for (const location of locations) {
    if (seen.has(location.artifactRef)) {
      continue;
    }

    seen.add(location.artifactRef);
    refs.push(location.artifactRef);
  }

  return refs;
}

function artifactValueMapFromResult(result: RuntimeDebugArtifactLoadResult) {
  return new Map(
    result.artifacts.map((artifact) => [artifact.artifact_ref, artifact.value])
  );
}

export function RuntimeDebugPayloadBlock({
  title,
  payload,
  defaultCollapsed = false,
  height = '220px',
  onLoadArtifact,
  onLoadArtifacts
}: {
  title: string;
  payload: unknown;
  defaultCollapsed?: boolean;
  height?: string;
  onLoadArtifact?: (artifactRef: string) => Promise<unknown>;
  onLoadArtifacts?: RuntimeDebugArtifactBatchLoader;
}) {
  const { message } = App.useApp();
  const [loadedPayloadState, setLoadedPayloadState] = useState<{
    source: unknown;
    value: unknown;
  } | null>(null);
  const [loadingArtifacts, setLoadingArtifacts] = useState(false);
  const hasLoadedPayload =
    loadedPayloadState !== null && loadedPayloadState.source === payload;
  const displayPayload = hasLoadedPayload ? loadedPayloadState.value : payload;
  const artifactLocations = useMemo(
    () => collectRuntimeDebugArtifactLocations(displayPayload),
    [displayPayload]
  );
  const artifactRefs = useMemo(
    () => uniqueArtifactRefs(artifactLocations),
    [artifactLocations]
  );
  const canLoadArtifacts =
    artifactRefs.length > 0 && Boolean(onLoadArtifacts || onLoadArtifact);

  const handleLoadFullValue = async () => {
    if (artifactLocations.length === 0 || !canLoadArtifacts) {
      return;
    }

    setLoadingArtifacts(true);
    try {
      const artifactValues = onLoadArtifacts
        ? artifactValueMapFromResult(await onLoadArtifacts(artifactRefs))
        : new Map(
            await Promise.all(
              artifactRefs.map(
                async (artifactRef) =>
                  [artifactRef, await onLoadArtifact!(artifactRef)] as const
              )
            )
          );

      for (const artifactRef of artifactRefs) {
        if (!artifactValues.has(artifactRef)) {
          throw new Error(`Missing runtime debug artifact ${artifactRef}`);
        }
      }

      const nextPayload = artifactLocations.reduce(
        (currentPayload, location) =>
          replacePayloadAtPath(
            currentPayload,
            location.path,
            artifactValues.get(location.artifactRef)
          ),
        displayPayload
      );
      setLoadedPayloadState({ source: payload, value: nextPayload });
      message.success(i18nText('agentFlow', 'auto.full_value_loaded'));
    } catch {
      message.error(i18nText('agentFlow', 'auto.failed_load_full_value'));
    } finally {
      setLoadingArtifacts(false);
    }
  };

  return (
    <JsonPreviewBlock
      actions={
        artifactLocations.length > 0 ? (
          <Space size={6} wrap>
            <Tag color="warning">{i18nText('agentFlow', 'auto.truncated')}</Tag>
            <Tooltip title={i18nText('agentFlow', 'auto.load_full_value')}>
              <Button
                aria-label={i18nText('agentFlow', 'auto.load_full_value')}
                disabled={!canLoadArtifacts}
                icon={<LineHeightOutlined />}
                loading={loadingArtifacts}
                onClick={handleLoadFullValue}
                size="small"
                type="text"
              />
            </Tooltip>
          </Space>
        ) : null
      }
      defaultCollapsed={defaultCollapsed}
      height={height}
      title={title}
      value={displayPayload}
    />
  );
}
