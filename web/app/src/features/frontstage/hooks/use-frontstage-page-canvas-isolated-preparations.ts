import { useEffect, useMemo, useRef, useState } from 'react';

import type { NormalizedFrontstageBlockCatalogEntry } from '../lib/block-catalog';
import {
  prepareFrontstageIsolatedContribution,
  type FrontstageIsolatedContributionExpectation,
  type PreparedFrontstageIsolatedContribution
} from '../lib/isolated-frontend-block-contribution';
import type { FrontstagePageRenderPlan } from '../lib/page-canvas/render-plan';

export interface FrontstageIsolatedPreparationRequest extends FrontstageIsolatedContributionExpectation {
  slotIndex: number;
}

export interface UseFrontstagePageCanvasIsolatedPreparationsInput {
  actorId: string | null | undefined;
  actorWorkspaceId: string | null | undefined;
  workspaceId: string;
  renderPlan: FrontstagePageRenderPlan | null | undefined;
  catalogEntries?: readonly NormalizedFrontstageBlockCatalogEntry[] | null;
  fetchAsset?: typeof fetch;
}

export interface UseFrontstagePageCanvasIsolatedPreparationsResult {
  preparations: readonly PreparedFrontstageIsolatedContribution[];
  errorsByBlockId: Readonly<Record<string, Error>>;
}

interface IsolatedPreparationState extends UseFrontstagePageCanvasIsolatedPreparationsResult {
  owner: readonly FrontstageIsolatedPreparationRequest[] | null;
}

const EMPTY_RESULT: UseFrontstagePageCanvasIsolatedPreparationsResult = {
  preparations: [],
  errorsByBlockId: {}
};

export function createFrontstageIsolatedPreparationRequests({
  workspaceId,
  renderPlan
}: {
  workspaceId: string;
  renderPlan: FrontstagePageRenderPlan | null | undefined;
}): FrontstageIsolatedPreparationRequest[] {
  if (!renderPlan) return [];
  return renderPlan.items.flatMap((item, slotIndex) => {
    if (
      item.renderMode !== 'isolated_iframe' ||
      !item.canMountIsolatedIframe ||
      item.fallbackReasons.length > 0
    ) {
      return [];
    }
    const installationId = requiredString(item.catalog.installationId);
    const providerCode = requiredString(item.catalog.providerCode);
    const pluginId = requiredString(item.contribution.pluginId);
    const pluginVersion = requiredString(item.contribution.pluginVersion);
    const contributionCode = requiredString(item.contribution.code);
    if (
      !installationId ||
      !providerCode ||
      !pluginId ||
      !pluginVersion ||
      !contributionCode
    ) {
      return [];
    }
    return [
      {
        blockInstanceId: item.blockId,
        workspaceId,
        installationId,
        providerCode,
        pluginId,
        pluginVersion,
        contributionCode,
        props: { ...item.props },
        slotIndex
      }
    ];
  });
}

export function useFrontstagePageCanvasIsolatedPreparations({
  actorId,
  actorWorkspaceId,
  workspaceId,
  renderPlan,
  catalogEntries,
  fetchAsset = fetchIsolatedAsset
}: UseFrontstagePageCanvasIsolatedPreparationsInput): UseFrontstagePageCanvasIsolatedPreparationsResult {
  const stableCatalogEntries =
    useSemanticallyStableCatalogEntries(catalogEntries);
  const requests = useMemo(
    () =>
      createFrontstageIsolatedPreparationRequests({ workspaceId, renderPlan }),
    [renderPlan, workspaceId]
  );
  const identityErrorsByBlockId = useMemo(
    () => createIsolatedIdentityErrors(renderPlan, requests),
    [renderPlan, requests]
  );
  const emptyCurrentResult =
    useMemo<UseFrontstagePageCanvasIsolatedPreparationsResult>(
      () => ({ preparations: [], errorsByBlockId: identityErrorsByBlockId }),
      [identityErrorsByBlockId]
    );
  const [state, setState] = useState<IsolatedPreparationState>({
    owner: null,
    ...EMPTY_RESULT
  });

  useEffect(() => {
    if (
      !actorId ||
      actorWorkspaceId !== workspaceId ||
      stableCatalogEntries === null ||
      requests.length === 0
    ) {
      setState({ owner: requests, ...emptyCurrentResult });
      return;
    }
    const controller = new AbortController();
    setState({ owner: requests, ...emptyCurrentResult });
    void Promise.all(
      requests.map(async (request) => {
        try {
          const catalogEntry = stableCatalogEntries?.find(
            (entry) =>
              entry.installationId === request.installationId &&
              entry.providerCode === request.providerCode &&
              entry.pluginId === request.pluginId &&
              entry.pluginVersion === request.pluginVersion &&
              entry.contributionCode === request.contributionCode
          );
          if (!catalogEntry) {
            throw new Error(
              'Isolated frontend contribution binding is unavailable.'
            );
          }
          const preparation = await prepareFrontstageIsolatedContribution(
            catalogEntry.raw,
            request,
            (input, init) =>
              fetchAsset(input, { ...init, signal: controller.signal })
          );
          return { status: 'prepared', request, preparation } as const;
        } catch (error) {
          return { status: 'failed', request, error: toError(error) } as const;
        }
      })
    ).then((outcomes) => {
      if (controller.signal.aborted) return;
      const preparations: PreparedFrontstageIsolatedContribution[] = [];
      const errorsByBlockId: Record<string, Error> = {
        ...identityErrorsByBlockId
      };
      for (const outcome of outcomes) {
        if (outcome.status === 'failed') {
          errorsByBlockId[outcome.request.blockInstanceId] = outcome.error;
        } else {
          preparations.push(outcome.preparation);
        }
      }
      setState({ owner: requests, preparations, errorsByBlockId });
    });
    return () => controller.abort();
  }, [
    actorId,
    actorWorkspaceId,
    emptyCurrentResult,
    fetchAsset,
    identityErrorsByBlockId,
    requests,
    stableCatalogEntries,
    workspaceId
  ]);

  return state.owner === requests ? state : emptyCurrentResult;
}

function useSemanticallyStableCatalogEntries(
  entries: readonly NormalizedFrontstageBlockCatalogEntry[] | null | undefined
): readonly NormalizedFrontstageBlockCatalogEntry[] | null | undefined {
  const identity =
    entries === undefined
      ? 'undefined'
      : entries === null
        ? 'null'
        : JSON.stringify(entries.map((entry) => entry.raw));
  const snapshot = useRef({ identity, entries });
  if (snapshot.current.identity !== identity) {
    snapshot.current = { identity, entries };
  }
  return snapshot.current.entries;
}

function createIsolatedIdentityErrors(
  renderPlan: FrontstagePageRenderPlan | null | undefined,
  requests: readonly FrontstageIsolatedPreparationRequest[]
): Readonly<Record<string, Error>> {
  if (!renderPlan) return {};
  const requestBlockIds = new Set(
    requests.map((request) => request.blockInstanceId)
  );
  return Object.fromEntries(
    renderPlan.items.flatMap((item) =>
      item.renderMode === 'isolated_iframe' &&
      item.canMountIsolatedIframe &&
      !requestBlockIds.has(item.blockId)
        ? [
            [
              item.blockId,
              new Error(
                'Isolated frontend contribution identity is unavailable.'
              )
            ]
          ]
        : []
    )
  );
}

function requiredString(value: string | null): string | null {
  return value?.trim() || null;
}

function fetchIsolatedAsset(
  input: RequestInfo | URL,
  init?: RequestInit
): Promise<Response> {
  return globalThis.fetch(input, init);
}

function toError(error: unknown): Error {
  return error instanceof Error
    ? error
    : new Error('Isolated frontend contribution preparation failed.');
}
