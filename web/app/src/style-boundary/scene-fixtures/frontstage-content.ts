export function createStyleBoundaryFrontstagePageContent() {
  return {
    page: {
      id: 'page-1',
      title: 'Landing',
      kind: 'page' as const,
      parentId: null,
      rank: '001000',
      contentPresentation: 'single' as const
    },
    tab: {
      id: 'tab-1',
      pageId: 'page-1',
      title: 'Overview',
      rank: '001000',
      isDefault: true,
      routeSegment: null,
      documentRootUid: 'root-1'
    },
    document: {
      rootUid: 'root-1',
      payload: { blocks: [] }
    }
  };
}
