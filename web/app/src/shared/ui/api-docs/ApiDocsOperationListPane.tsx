import type { UIEvent } from 'react';

import { Empty, Input, Result, Spin, Typography } from 'antd';

import type {
  ApiDocsExplorerQueryState,
  ApiDocsOperationWithCategory
} from './ApiDocsExplorer';
import { i18nText } from '../../i18n/text';

export interface ApiDocsOperationListPaneProps {
  categoriesLength: number;
  selectedCategoryId: string | null;
  selectedCategoryLabel: string | null;
  showAllOperationsWhenNoCategory: boolean;
  loading: boolean;
  error: boolean;
  operations: ApiDocsOperationWithCategory[];
  filteredOperations: ApiDocsOperationWithCategory[];
  selectedOperationId: string | null;
  operationSearch: string;
  selectedCategoryOperationTotal: number;
  fetchingNextPage: boolean;
  onOperationSearchChange: (value: string) => void;
  onOperationScroll: (event: UIEvent<HTMLDivElement>) => void;
  onQueryStateChange: (
    nextState: ApiDocsExplorerQueryState,
    mode?: 'push' | 'replace'
  ) => void;
}

export function ApiDocsOperationListPane({
  categoriesLength,
  selectedCategoryId,
  selectedCategoryLabel,
  showAllOperationsWhenNoCategory,
  loading,
  error,
  operations,
  filteredOperations,
  selectedOperationId,
  operationSearch,
  selectedCategoryOperationTotal,
  fetchingNextPage,
  onOperationSearchChange,
  onOperationScroll,
  onQueryStateChange
}: ApiDocsOperationListPaneProps) {
  const hasOperationSearch = operationSearch.trim().length > 0;

  if (categoriesLength === 0) {
    return (
      <section
        className="api-docs-panel__pane"
        aria-label={i18nText('sharedUi', 'auto.interface_list')}
      >
        <div className="api-docs-panel__pane-header">
          <div className="api-docs-panel__pane-copy">
            <Typography.Text strong>
              {i18nText('sharedUi', 'auto.interface_list')}
            </Typography.Text>
            <Typography.Text type="secondary">
              {i18nText('sharedUi', 'auto.no_accessible_categories')}
            </Typography.Text>
          </div>
        </div>
        <div className="api-docs-panel__pane-body">
          <Empty
            description={i18nText('sharedUi', 'auto.no_interface_categories')}
            image={Empty.PRESENTED_IMAGE_SIMPLE}
          />
        </div>
      </section>
    );
  }

  if (!selectedCategoryId && !showAllOperationsWhenNoCategory) {
    return (
      <section
        className="api-docs-panel__pane"
        aria-label={i18nText('sharedUi', 'auto.interface_list')}
      >
        <div className="api-docs-panel__pane-header">
          <div className="api-docs-panel__pane-copy">
            <Typography.Text strong>
              {i18nText('sharedUi', 'auto.interface_list')}
            </Typography.Text>
            <Typography.Text type="secondary">
              {i18nText(
                'sharedUi',
                'auto.select_category_then_display_interfaces'
              )}
            </Typography.Text>
          </div>
        </div>
        <div className="api-docs-panel__pane-body">
          <Result
            status="info"
            title={i18nText(
              'sharedUi',
              'auto.select_category_to_view_interface_list'
            )}
            subTitle={i18nText('sharedUi', 'auto.category_header_hint')}
          />
        </div>
      </section>
    );
  }

  if (loading && !hasOperationSearch) {
    return (
      <section
        className="api-docs-panel__pane"
        aria-label={i18nText('sharedUi', 'auto.interface_list')}
      >
        <div className="api-docs-panel__pane-header">
          <div className="api-docs-panel__pane-copy">
            <Typography.Text strong>
              {i18nText('sharedUi', 'auto.interface_list')}
            </Typography.Text>
            <Typography.Text type="secondary">
              {i18nText('sharedUi', 'auto.loading')}
              {selectedCategoryLabel ??
                i18nText('sharedUi', 'auto.all_categories')}{' '}
              {i18nText('sharedUi', 'auto.interfaces_in_suffix')}
            </Typography.Text>
          </div>
        </div>
        <div className="api-docs-panel__pane-state">
          <Spin size="large" />
        </div>
      </section>
    );
  }

  if (error) {
    return (
      <section
        className="api-docs-panel__pane"
        aria-label={i18nText('sharedUi', 'auto.interface_list')}
      >
        <div className="api-docs-panel__pane-header">
          <div className="api-docs-panel__pane-copy">
            <Typography.Text strong>
              {i18nText('sharedUi', 'auto.interface_list')}
            </Typography.Text>
            <Typography.Text type="secondary">
              {selectedCategoryId
                ? i18nText('sharedUi', 'auto.current_category')
                : i18nText('sharedUi', 'auto.all_categories')}
              {i18nText('sharedUi', 'auto.interface_load_failed')}
            </Typography.Text>
          </div>
        </div>
        <div className="api-docs-panel__pane-body">
          <Result
            status="error"
            title={i18nText('sharedUi', 'auto.interface_list_load_failed')}
            subTitle={i18nText('sharedUi', 'auto.refresh_or_switch_category')}
          />
        </div>
      </section>
    );
  }

  return (
    <section
      className="api-docs-panel__pane"
      aria-label={i18nText('sharedUi', 'auto.interface_list')}
    >
      <div className="api-docs-panel__pane-header">
        <div className="api-docs-panel__pane-copy">
          <Typography.Text strong>
            {i18nText('sharedUi', 'auto.interface_list')}
          </Typography.Text>
          <Typography.Text type="secondary">
            {selectedCategoryLabel ??
              i18nText('sharedUi', 'auto.all_categories')}{' '}
            {i18nText('sharedUi', 'auto.total')}{' '}
            {selectedCategoryId
              ? selectedCategoryOperationTotal
              : operations.length}{' '}
            {i18nText('sharedUi', 'auto.interface_count_suffix')}
          </Typography.Text>
        </div>
      </div>
      <div className="api-docs-panel__pane-toolbar">
        <Input
          aria-label={i18nText('sharedUi', 'auto.search_interface')}
          allowClear
          placeholder={i18nText('sharedUi', 'auto.search_interface')}
          value={operationSearch}
          onChange={(event) => onOperationSearchChange(event.target.value)}
        />
      </div>
      <div
        className="api-docs-panel__pane-body"
        data-testid="api-docs-operation-list-scroll-area"
        onScroll={onOperationScroll}
      >
        {loading ? (
          <div className="api-docs-panel__pane-state">
            <Spin size="large" />
          </div>
        ) : !operations.length ? (
          <Empty
            description={
              hasOperationSearch
                ? i18nText('sharedUi', 'auto.no_matching_interface')
                : selectedCategoryId
                  ? i18nText(
                      'sharedUi',
                      'auto.no_interfaces_in_current_category'
                    )
                  : i18nText('sharedUi', 'auto.no_interfaces')
            }
            image={Empty.PRESENTED_IMAGE_SIMPLE}
          />
        ) : filteredOperations.length === 0 ? (
          <Empty
            description={i18nText('sharedUi', 'auto.no_matching_interface')}
            image={Empty.PRESENTED_IMAGE_SIMPLE}
          />
        ) : (
          <div className="api-docs-panel__list">
            {filteredOperations.map((operation) => (
              <button
                key={operation.id}
                type="button"
                className="api-docs-panel__list-button api-docs-panel__list-button--operation"
                aria-pressed={selectedOperationId === operation.id}
                onClick={() =>
                  onQueryStateChange({
                    categoryId: selectedCategoryId,
                    operationId: operation.id
                  })
                }
              >
                <span className="api-docs-panel__list-button-main">
                  <span className="api-docs-panel__operation-heading">
                    <span
                      className={`api-docs-panel__operation-method api-docs-panel__operation-method--${operation.method.toLowerCase()}`}
                    >
                      {operation.method}
                    </span>
                    <span className="api-docs-panel__operation-path">
                      {operation.path}
                    </span>
                  </span>
                  <span className="api-docs-panel__list-button-subtitle">
                    {operation.summary ?? operation.description ?? operation.id}
                  </span>
                </span>
              </button>
            ))}
            {selectedCategoryId && fetchingNextPage ? (
              <div className="api-docs-panel__list-loading">
                <Spin size="small" />
              </div>
            ) : null}
          </div>
        )}
      </div>
    </section>
  );
}
