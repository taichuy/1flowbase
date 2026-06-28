import { useDeferredValue, useEffect, useMemo, useState } from 'react';
import type { MouseEvent, UIEvent } from 'react';
import { createPortal } from 'react-dom';

import { ApiReferenceReact } from '@scalar/api-reference-react';
import '@scalar/api-reference-react/style.css';
import { useInfiniteQuery, useQueries, useQuery } from '@tanstack/react-query';
import { message, Result, Select, Spin, Typography } from 'antd';

import './api-docs-explorer.css';
import { ApiDocsOperationListPane } from './ApiDocsOperationListPane';
import { i18nText } from '../../i18n/text';
import { copyTextToClipboard } from '../clipboard/copy-text';

type ScalarReferenceConfiguration = Exclude<
  Parameters<typeof ApiReferenceReact>[0]['configuration'],
  unknown[]
>;
type ScalarAuthenticationConfiguration =
  ScalarReferenceConfiguration['authentication'];
type ScalarDocumentContent = ScalarReferenceConfiguration['content'];

const emptyCategories: ApiDocsCatalogCategory[] = [];
const apiDocsOperationsPageSize = 20;
const scalarOperationHashPattern =
  /(?:^|\/)(GET|POST|PUT|PATCH|DELETE|HEAD|OPTIONS|TRACE)\/(.+)$/i;
const scalarOperationMethodPattern =
  /^(GET|POST|PUT|PATCH|DELETE|HEAD|OPTIONS|TRACE)$/i;
const scalarOperationTextPattern =
  /^(GET|POST|PUT|PATCH|DELETE|HEAD|OPTIONS|TRACE)\s+(\/\S.*)$/i;

export interface ApiDocsCatalogOperation {
  id: string;
  method: string;
  path: string;
  summary: string | null;
  description: string | null;
  tags: string[];
  group: string;
  deprecated: boolean;
}

export interface ApiDocsCatalogCategory {
  id: string;
  label: string;
  operation_count: number;
}

export interface ApiDocsCatalog {
  title: string;
  version: string;
  categories: ApiDocsCatalogCategory[];
}

export interface ApiDocsCategoryOperations {
  id: string;
  label: string;
  operations: ApiDocsCatalogOperation[];
  total?: number;
  offset?: number;
  limit?: number;
  has_more?: boolean;
  next_offset?: number | null;
}

export interface ApiDocsCategoryOperationsRequest {
  offset?: number;
  limit?: number;
  q?: string | null;
}

export interface ApiDocsExplorerQueryState {
  categoryId: string | null;
  operationId: string | null;
}

type QueryKey = readonly unknown[];

type CategorySelectOption = {
  value: string;
  label: string;
  categoryId: string;
  operationCount: number;
  searchText: string;
};

export type ApiDocsOperationWithCategory = ApiDocsCatalogOperation & {
  categoryId: string | null;
  categoryLabel: string | null;
};

export interface ApiDocsExplorerProps<TAuthenticationSnapshot = unknown> {
  queryState: ApiDocsExplorerQueryState;
  onQueryStateChange: (
    nextState: ApiDocsExplorerQueryState,
    mode?: 'push' | 'replace'
  ) => void;
  catalogQueryKey: QueryKey;
  fetchCatalog: () => Promise<ApiDocsCatalog>;
  categoryOperationsQueryKey: (categoryId: string) => QueryKey;
  fetchCategoryOperations: (
    categoryId: string,
    request?: ApiDocsCategoryOperationsRequest
  ) => Promise<ApiDocsCategoryOperations>;
  operationSpecQueryKey: (operationId: string) => QueryKey;
  fetchOperationSpec: (operationId: string) => Promise<ScalarDocumentContent>;
  baseServerUrl: string | (() => string);
  showAllOperationsWhenNoCategory?: boolean;
  selectFirstCategoryWhenEmpty?: boolean;
  toolbarPortalId?: string;
  authentication?: {
    queryKey: QueryKey;
    queryFn: () => Promise<TAuthenticationSnapshot>;
    buildConfig: (
      operationSpec: ScalarDocumentContent | undefined,
      snapshot: TAuthenticationSnapshot | undefined
    ) => ScalarAuthenticationConfiguration;
  };
}

function normalizeSearchText(input: string): string {
  return input.toLowerCase().replace(/[\s\-/:_]+/g, '');
}

function buildCategorySearchText(category: ApiDocsCatalogCategory): string {
  return normalizeSearchText(
    `${category.id} ${category.label} ${category.operation_count}`
  );
}

function buildOperationSearchText(operation: ApiDocsCatalogOperation): string {
  return normalizeSearchText(
    `${operation.method} ${operation.path} ${operation.summary ?? ''} ${operation.description ?? ''} ${operation.group} ${operation.id}`
  );
}

function buildScalarOperationCopyText(
  anchor: HTMLAnchorElement
): string | null {
  const endpointMethod = anchor
    .querySelector('.endpoint-method')
    ?.textContent?.trim();
  const endpointPath = anchor
    .querySelector('.endpoint-path')
    ?.textContent?.trim();

  if (
    endpointMethod &&
    endpointPath &&
    scalarOperationMethodPattern.test(endpointMethod) &&
    endpointPath.startsWith('/')
  ) {
    return `${endpointMethod.toUpperCase()} ${endpointPath}`;
  }

  const hashContent = anchor.hash
    ? decodeURIComponent(anchor.hash.slice(1))
    : '';
  const hashMatch = hashContent.match(scalarOperationHashPattern);

  if (hashMatch) {
    const path = hashMatch[2].startsWith('/')
      ? hashMatch[2]
      : `/${hashMatch[2]}`;

    return `${hashMatch[1].toUpperCase()} ${path}`;
  }

  const text = anchor.textContent?.trim().replace(/\s+/g, ' ') ?? '';
  const textMatch = text.match(scalarOperationTextPattern);

  if (!textMatch) {
    return null;
  }

  return `${textMatch[1].toUpperCase()} ${textMatch[2]}`;
}

export function ApiDocsExplorer<TAuthenticationSnapshot = unknown>({
  queryState,
  onQueryStateChange,
  catalogQueryKey,
  fetchCatalog,
  categoryOperationsQueryKey,
  fetchCategoryOperations,
  operationSpecQueryKey,
  fetchOperationSpec,
  baseServerUrl,
  showAllOperationsWhenNoCategory = false,
  selectFirstCategoryWhenEmpty = false,
  toolbarPortalId,
  authentication
}: ApiDocsExplorerProps<TAuthenticationSnapshot>) {
  const [operationSearch, setOperationSearch] = useState('');
  const [toolbarPortalElement, setToolbarPortalElement] =
    useState<HTMLElement | null>(null);
  const deferredOperationSearch = useDeferredValue(operationSearch);
  const normalizedOperationSearch = useMemo(
    () => normalizeSearchText(deferredOperationSearch),
    [deferredOperationSearch]
  );
  const catalogQuery = useQuery({
    queryKey: catalogQueryKey,
    queryFn: fetchCatalog
  });
  const categories = catalogQuery.data?.categories ?? emptyCategories;
  const selectedCategoryId =
    categories.find((category) => category.id === queryState.categoryId)?.id ??
    null;
  const selectedCategory =
    categories.find((category) => category.id === selectedCategoryId) ?? null;
  const totalOperations = categories.reduce(
    (total, category) => total + category.operation_count,
    0
  );
  const categoryOptions: CategorySelectOption[] = categories.map(
    (category) => ({
      value: category.id,
      label: category.label,
      categoryId: category.id,
      operationCount: category.operation_count,
      searchText: buildCategorySearchText(category)
    })
  );

  useEffect(() => {
    if (
      catalogQuery.isLoading ||
      !queryState.categoryId ||
      selectedCategoryId
    ) {
      return;
    }

    onQueryStateChange({ categoryId: null, operationId: null }, 'replace');
  }, [
    catalogQuery.isLoading,
    onQueryStateChange,
    queryState.categoryId,
    selectedCategoryId
  ]);

  useEffect(() => {
    if (
      catalogQuery.isLoading ||
      !selectFirstCategoryWhenEmpty ||
      showAllOperationsWhenNoCategory ||
      queryState.categoryId ||
      categories.length === 0
    ) {
      return;
    }

    onQueryStateChange(
      { categoryId: categories[0].id, operationId: null },
      'replace'
    );
  }, [
    catalogQuery.isLoading,
    categories,
    onQueryStateChange,
    queryState.categoryId,
    selectFirstCategoryWhenEmpty,
    showAllOperationsWhenNoCategory
  ]);

  const categoryOperationsQuery = useInfiniteQuery({
    queryKey: [
      ...categoryOperationsQueryKey(selectedCategoryId ?? ''),
      'search',
      normalizedOperationSearch,
      'page-size',
      apiDocsOperationsPageSize
    ],
    initialPageParam: 0,
    queryFn: ({ pageParam }) =>
      fetchCategoryOperations(selectedCategoryId!, {
        offset: Number(pageParam),
        limit: apiDocsOperationsPageSize,
        q: deferredOperationSearch.trim() || null
      }),
    getNextPageParam: (lastPage) => {
      if (!lastPage.has_more) {
        return undefined;
      }

      return (
        lastPage.next_offset ??
        (lastPage.offset ?? 0) + lastPage.operations.length
      );
    },
    placeholderData: (previousData) => previousData,
    enabled: Boolean(selectedCategoryId)
  });

  const allCategoryOperationsQueries = useQueries({
    queries: categories.map((category) => ({
      queryKey: [
        ...categoryOperationsQueryKey(category.id),
        'page-size',
        apiDocsOperationsPageSize
      ],
      queryFn: () =>
        fetchCategoryOperations(category.id, {
          offset: 0,
          limit: apiDocsOperationsPageSize
        }),
      enabled: showAllOperationsWhenNoCategory && !selectedCategoryId
    }))
  });
  const allCategoryOperationsLoading =
    showAllOperationsWhenNoCategory &&
    !selectedCategoryId &&
    allCategoryOperationsQueries.some((query) => query.isLoading);
  const allCategoryOperationsError =
    showAllOperationsWhenNoCategory &&
    !selectedCategoryId &&
    allCategoryOperationsQueries.some((query) => query.isError);
  const allCategoryOperations = useMemo(
    () =>
      allCategoryOperationsQueries.flatMap((query, index) => {
        const category = categories[index];

        return (query.data?.operations ?? []).map((operation) => ({
          ...operation,
          categoryId: category?.id ?? null,
          categoryLabel: category?.label ?? null
        }));
      }),
    [allCategoryOperationsQueries, categories]
  );
  const selectedCategoryOperations = useMemo(
    () =>
      (
        categoryOperationsQuery.data?.pages.flatMap(
          (page) => page.operations
        ) ?? []
      ).map((operation) => ({
        ...operation,
        categoryId: selectedCategoryId,
        categoryLabel: selectedCategory?.label ?? null
      })),
    [
      categoryOperationsQuery.data?.pages,
      selectedCategory?.label,
      selectedCategoryId
    ]
  );

  useEffect(() => {
    if (!toolbarPortalId) {
      setToolbarPortalElement(null);
      return;
    }

    setToolbarPortalElement(document.getElementById(toolbarPortalId));
  }, [toolbarPortalId]);
  const operations: ApiDocsOperationWithCategory[] =
    showAllOperationsWhenNoCategory && !selectedCategoryId
      ? allCategoryOperations
      : selectedCategoryOperations;
  const selectedOperationId =
    operations.find((operation) => operation.id === queryState.operationId)
      ?.id ?? null;
  const selectedOperation =
    operations.find((operation) => operation.id === selectedOperationId) ??
    null;
  const selectedCategoryOperationTotal =
    categoryOperationsQuery.data?.pages[0]?.total ??
    selectedCategory?.operation_count ??
    selectedCategoryOperations.length;
  const {
    isLoading: isCategoryOperationsLoading,
    isFetchingNextPage: isFetchingCategoryOperationsNextPage,
    hasNextPage: hasNextCategoryOperationsPage,
    fetchNextPage: fetchNextCategoryOperationsPage
  } = categoryOperationsQuery;

  const filteredOperations = useMemo(() => {
    const normalizedQuery = normalizeSearchText(operationSearch);

    if (!normalizedQuery) {
      return operations;
    }

    return operations.filter((operation) =>
      buildOperationSearchText(operation).includes(normalizedQuery)
    );
  }, [operationSearch, operations]);

  useEffect(() => {
    if (
      !selectedCategoryId ||
      isCategoryOperationsLoading ||
      isFetchingCategoryOperationsNextPage ||
      !queryState.operationId ||
      selectedOperationId
    ) {
      return;
    }

    if (hasNextCategoryOperationsPage) {
      void fetchNextCategoryOperationsPage();
      return;
    }

    onQueryStateChange(
      { categoryId: selectedCategoryId, operationId: null },
      'replace'
    );
  }, [
    fetchNextCategoryOperationsPage,
    hasNextCategoryOperationsPage,
    isCategoryOperationsLoading,
    isFetchingCategoryOperationsNextPage,
    onQueryStateChange,
    queryState.operationId,
    selectedCategoryId,
    selectedOperationId
  ]);

  useEffect(() => {
    if (
      !showAllOperationsWhenNoCategory ||
      selectedCategoryId ||
      allCategoryOperationsLoading ||
      !queryState.operationId ||
      selectedOperationId
    ) {
      return;
    }

    onQueryStateChange({ categoryId: null, operationId: null }, 'replace');
  }, [
    allCategoryOperationsLoading,
    onQueryStateChange,
    queryState.operationId,
    selectedCategoryId,
    selectedOperationId,
    showAllOperationsWhenNoCategory
  ]);

  const operationSpecQuery = useQuery({
    queryKey: operationSpecQueryKey(selectedOperationId ?? ''),
    queryFn: () => fetchOperationSpec(selectedOperationId!),
    enabled: Boolean(selectedOperationId)
  });
  const authenticationQuery = useQuery({
    queryKey: authentication?.queryKey ?? [
      'api-docs',
      'authentication-disabled'
    ],
    queryFn: () => authentication!.queryFn(),
    enabled: Boolean(authentication && selectedOperationId)
  });

  function handleOperationPaneScroll(event: UIEvent<HTMLDivElement>) {
    if (
      !selectedCategoryId ||
      !categoryOperationsQuery.hasNextPage ||
      categoryOperationsQuery.isFetchingNextPage
    ) {
      return;
    }

    const target = event.currentTarget;
    const distanceToBottom =
      target.scrollHeight - target.scrollTop - target.clientHeight;

    if (distanceToBottom <= 160) {
      void categoryOperationsQuery.fetchNextPage();
    }
  }

  function handleScalarOperationLinkClick(event: MouseEvent<HTMLDivElement>) {
    const target = event.target;

    if (!(target instanceof Element)) {
      return;
    }

    const anchor = target.closest<HTMLAnchorElement>(
      'a[href*="#"], a.endpoint'
    );

    if (!anchor || !event.currentTarget.contains(anchor)) {
      return;
    }

    const copyText = buildScalarOperationCopyText(anchor);

    if (!copyText) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();

    void copyTextToClipboard(copyText)
      .then(() => {
        message.success(
          i18nText('sharedUi', 'auto.copied', {
            defaultValue: 'Copied'
          })
        );
      })
      .catch(() => {
        message.error(
          i18nText('sharedUi', 'auto.copy_failed', {
            defaultValue: 'Copy failed'
          })
        );
      });
  }

  function renderCategorySelector() {
    return (
      <section
        className="api-docs-panel__toolbar"
        aria-label={i18nText('sharedUi', 'auto.document_filter')}
      >
        <div className="api-docs-panel__header-control">
          <Select
            aria-label={i18nText('sharedUi', 'auto.interface_category')}
            className="api-docs-panel__category-select"
            showSearch
            allowClear
            disabled={categories.length === 0}
            value={selectedCategoryId ?? undefined}
            options={categoryOptions}
            placeholder={
              categories.length === 0
                ? i18nText('sharedUi', 'auto.no_interface_categories')
                : showAllOperationsWhenNoCategory
                  ? i18nText('sharedUi', 'auto.all_interfaces')
                  : i18nText('sharedUi', 'auto.select_interface_category')
            }
            optionRender={(option) => {
              const category = option.data as CategorySelectOption;

              return (
                <div className="api-docs-panel__category-option">
                  <div className="api-docs-panel__category-option-copy">
                    <span className="api-docs-panel__category-option-label">
                      {category.label}
                    </span>
                    <span
                      className="api-docs-panel__category-option-id"
                      aria-hidden="true"
                    >
                      {category.categoryId}
                    </span>
                  </div>
                  <span
                    className="api-docs-panel__category-option-count"
                    aria-hidden="true"
                  >
                    {category.operationCount}{' '}
                    {i18nText('sharedUi', 'auto.interface_count_suffix')}
                  </span>
                </div>
              );
            }}
            filterOption={(input, option) =>
              String(
                (option as CategorySelectOption | undefined)?.searchText ?? ''
              ).includes(normalizeSearchText(input))
            }
            onChange={(nextCategoryId) =>
              onQueryStateChange({
                categoryId: nextCategoryId ?? null,
                operationId: null
              })
            }
            notFoundContent={i18nText('sharedUi', 'auto.no_matching_category')}
          />
        </div>
        <Typography.Text className="api-docs-panel__count">
          {i18nText('sharedUi', 'auto.total')}
          {totalOperations}{' '}
          {i18nText('sharedUi', 'auto.interface_count_suffix')}
        </Typography.Text>
      </section>
    );
  }

  function renderOperationPane() {
    return (
      <ApiDocsOperationListPane
        categoriesLength={categories.length}
        selectedCategoryId={selectedCategoryId}
        selectedCategoryLabel={selectedCategory?.label ?? null}
        showAllOperationsWhenNoCategory={showAllOperationsWhenNoCategory}
        loading={
          selectedCategoryId
            ? categoryOperationsQuery.isLoading
            : allCategoryOperationsLoading
        }
        error={
          selectedCategoryId
            ? categoryOperationsQuery.isError
            : allCategoryOperationsError
        }
        operations={operations}
        filteredOperations={filteredOperations}
        selectedOperationId={selectedOperationId}
        operationSearch={operationSearch}
        selectedCategoryOperationTotal={selectedCategoryOperationTotal}
        fetchingNextPage={categoryOperationsQuery.isFetchingNextPage}
        onOperationSearchChange={setOperationSearch}
        onOperationScroll={handleOperationPaneScroll}
        onQueryStateChange={onQueryStateChange}
      />
    );
  }

  function renderDetailPane() {
    if (
      (!selectedCategoryId && !showAllOperationsWhenNoCategory) ||
      !selectedOperationId
    ) {
      return (
        <div className="api-docs-panel__detail-state">
          <Result
            status="info"
            title={i18nText(
              'sharedUi',
              'auto.view_details_after_selecting_interface'
            )}
            subTitle={
              showAllOperationsWhenNoCategory
                ? i18nText('sharedUi', 'auto.open_interface_from_left')
                : i18nText(
                    'sharedUi',
                    'auto.select_category_then_open_interface'
                  )
            }
          />
        </div>
      );
    }

    if (operationSpecQuery.isLoading) {
      return (
        <div className="api-docs-panel__detail-state">
          <Spin size="large" />
          <Typography.Text type="secondary">
            {i18nText('sharedUi', 'auto.loading')}
            {selectedOperation?.path ??
              i18nText('sharedUi', 'auto.current_interface')}{' '}
            {i18nText('sharedUi', 'auto.details_suffix')}
          </Typography.Text>
        </div>
      );
    }

    if (operationSpecQuery.isError) {
      return (
        <div className="api-docs-panel__detail-state">
          <Result
            status="error"
            title={i18nText('sharedUi', 'auto.interface_details_load_failed')}
            subTitle={i18nText(
              'sharedUi',
              'auto.interface_document_return_failed'
            )}
          />
        </div>
      );
    }

    return (
      <div
        className="api-docs-panel__detail-viewer"
        onClickCapture={handleScalarOperationLinkClick}
      >
        <ApiReferenceReact
          configuration={{
            authentication: authentication?.buildConfig(
              operationSpecQuery.data,
              authenticationQuery.data
            ),
            baseServerURL:
              typeof baseServerUrl === 'function'
                ? baseServerUrl()
                : baseServerUrl,
            content: operationSpecQuery.data,
            showSidebar: false
          }}
        />
      </div>
    );
  }

  let content = null;

  if (catalogQuery.isLoading) {
    content = (
      <div className="api-docs-panel__detail-state">
        <Spin size="large" />
        <Typography.Text type="secondary">
          {i18nText('sharedUi', 'auto.loading_interface_directory')}
        </Typography.Text>
      </div>
    );
  } else if (catalogQuery.isError) {
    content = (
      <Result
        status="error"
        title={i18nText('sharedUi', 'auto.interface_directory_load_failed')}
        subTitle={i18nText('sharedUi', 'auto.api_doc_permission_retry')}
      />
    );
  } else {
    content = (
      <>
        {toolbarPortalElement ? null : renderCategorySelector()}
        <div className="api-docs-panel__workspace">
          {renderOperationPane()}
          <section
            className="api-docs-panel__detail"
            aria-label={i18nText('sharedUi', 'auto.api_documentation_details')}
          >
            {renderDetailPane()}
          </section>
        </div>
      </>
    );
  }

  return (
    <div
      className={
        toolbarPortalElement
          ? 'api-docs-panel api-docs-panel--external-toolbar'
          : 'api-docs-panel'
      }
    >
      {toolbarPortalElement && !catalogQuery.isLoading && !catalogQuery.isError
        ? createPortal(renderCategorySelector(), toolbarPortalElement)
        : null}
      {content}
    </div>
  );
}
