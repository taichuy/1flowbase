import type {
  FrontstagePageContent,
  FrontstagePageContentNode,
  FrontstagePageContentTab,
  FrontstageTabDocument
} from '../api/page-content';

type LegacyDocumentFixture = {
  rootUid?: string;
  uid?: string;
  payload: unknown;
};

export type FrontstagePageContentFixtureOverrides = Omit<
  Partial<FrontstagePageContent>,
  'page' | 'tab' | 'document'
> & {
  page?: Partial<FrontstagePageContentNode>;
  tab?: Partial<FrontstagePageContentTab>;
  document?: Partial<FrontstageTabDocument>;
  schema?: LegacyDocumentFixture;
  root?: LegacyDocumentFixture;
};

function hasBlockArray(payload: unknown): boolean {
  return (
    typeof payload === 'object' &&
    payload !== null &&
    !Array.isArray(payload) &&
    Array.isArray((payload as Record<string, unknown>).blocks)
  );
}

/**
 * Test fixtures may describe a historical schema/root payload, but all test
 * subjects receive the current single Tab Document contract.
 */
export function createFrontstagePageContentFixture(
  overrides: FrontstagePageContentFixtureOverrides = {}
): FrontstagePageContent {
  const { page: pageOverride, tab: tabOverride, document, schema, root } =
    overrides;
  const page: FrontstagePageContentNode = {
    id: 'page-1',
    title: 'Landing',
    kind: 'page',
    parentId: null,
    rank: '001000',
    contentPresentation: 'single',
    ...pageOverride
  };
  const tab: FrontstagePageContentTab = {
    id: 'tab-1',
    pageId: page.id,
    title: '概览',
    rank: '001000',
    isDefault: true,
    routeSegment: null,
    documentRootUid: 'root-1',
    ...tabOverride
  };
  const fallbackPayload = hasBlockArray(root?.payload)
    ? root?.payload
    : hasBlockArray(schema?.payload)
      ? schema?.payload
      : root?.payload ?? schema?.payload ?? {};

  return {
    page,
    tab,
    document: {
      rootUid: root?.uid ?? schema?.rootUid ?? tab.documentRootUid,
      payload: fallbackPayload,
      ...document
    }
  };
}
