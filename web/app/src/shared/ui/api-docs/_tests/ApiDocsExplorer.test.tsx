import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

vi.mock('@scalar/api-reference-react', () => ({
  ApiReferenceReact: ({ configuration }: { configuration: unknown }) => (
    <div data-testid="scalar-viewer">
      {JSON.stringify(configuration)}
      <a
        className="endpoint"
        onClick={() => {
          window.location.hash = 'operation-create-chat-completion';
        }}
      >
        <span className="endpoint-method">POST</span>
        <span className="endpoint-path">/v1/chat/completions</span>
      </a>
    </div>
  )
}));

vi.mock('antd', async () => {
  const actual = await vi.importActual<typeof import('antd')>('antd');

  return {
    ...actual,
    message: {
      ...actual.message,
      error: vi.fn(),
      success: vi.fn()
    }
  };
});

vi.mock('../../clipboard/copy-text', () => ({
  copyTextToClipboard: vi.fn(() => Promise.resolve())
}));

import { AppProviders } from '../../../../app/AppProviders';
import { appI18n } from '../../../i18n/app-i18n';
import { ApiDocsExplorer } from '../ApiDocsExplorer';
import { copyTextToClipboard } from '../../clipboard/copy-text';

const catalog = {
  title: 'Application API',
  version: '1.0.0',
  categories: [
    {
      id: 'application-native-api',
      label: 'Application Native API',
      operation_count: 2
    },
    {
      id: 'openai-compatible-api',
      label: 'OpenAI Compatible',
      operation_count: 1
    }
  ]
};

const operationsByCategory = {
  'application-native-api': {
    id: 'application-native-api',
    label: 'Application Native API',
    operations: [
      {
        id: 'create_native_run',
        method: 'POST',
        path: '/api/agent/v1/runs',
        summary: 'Create Native public run',
        description: 'Create Native public run',
        tags: ['native'],
        group: 'application-native-api',
        deprecated: false
      },
      {
        id: 'get_native_run',
        method: 'GET',
        path: '/api/agent/v1/runs/{run_id}',
        summary: 'Get Native public run',
        description: 'Get Native public run',
        tags: ['native'],
        group: 'application-native-api',
        deprecated: false
      }
    ]
  },
  'openai-compatible-api': {
    id: 'openai-compatible-api',
    label: 'OpenAI Compatible',
    operations: [
      {
        id: 'create_chat_completion',
        method: 'POST',
        path: '/v1/chat/completions',
        summary: 'Create chat completion',
        description: 'Create chat completion',
        tags: ['openai'],
        group: 'openai-compatible-api',
        deprecated: false
      }
    ]
  }
};

describe('ApiDocsExplorer', () => {
  beforeEach(async () => {
    window.history.replaceState(null, '', '/?language=zh-Hans');
    await appI18n.changeLanguage('zh_Hans');
  });

  afterEach(async () => {
    window.history.replaceState(null, '', '/');
    await appI18n.changeLanguage('en_US');
  });

  test('loads the next operations page when the selected category list scrolls near the bottom', async () => {
    const pagedOperations = Array.from({ length: 25 }, (_, index) => ({
      id: `op_${index}`,
      method: 'GET',
      path: `/api/v1/items/${index}`,
      summary: `Operation ${index}`,
      description: `Operation ${index}`,
      tags: ['native'],
      group: 'application-native-api',
      deprecated: false
    }));
    const fetchCategoryOperations = vi.fn(
      (
        categoryId: string,
        request?: { offset?: number; limit?: number; q?: string | null }
      ) => {
        const offset = request?.offset ?? 0;
        const limit = request?.limit ?? 20;
        const operations = pagedOperations.slice(offset, offset + limit);

        return Promise.resolve({
          id: categoryId,
          label: 'Application Native API',
          operations,
          total: pagedOperations.length,
          offset,
          limit,
          has_more: offset + operations.length < pagedOperations.length,
          next_offset:
            offset + operations.length < pagedOperations.length
              ? offset + operations.length
              : null
        });
      }
    );

    render(
      <AppProviders>
        <ApiDocsExplorer
          queryState={{
            categoryId: 'application-native-api',
            operationId: null
          }}
          onQueryStateChange={vi.fn()}
          catalogQueryKey={['api-docs', 'catalog']}
          fetchCatalog={() => Promise.resolve(catalog)}
          categoryOperationsQueryKey={(categoryId) => [
            'api-docs',
            'category',
            categoryId
          ]}
          fetchCategoryOperations={fetchCategoryOperations}
          operationSpecQueryKey={(operationId) => [
            'api-docs',
            'operation',
            operationId
          ]}
          fetchOperationSpec={() => Promise.resolve({})}
          baseServerUrl="http://127.0.0.1:3100"
        />
      </AppProviders>
    );

    expect(
      await screen.findByRole('button', { name: /get \/api\/v1\/items\/0/i })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: /get \/api\/v1\/items\/24/i })
    ).not.toBeInTheDocument();
    expect(fetchCategoryOperations).toHaveBeenCalledWith(
      'application-native-api',
      { offset: 0, limit: 20, q: null }
    );

    const scrollArea = screen.getByTestId(
      'api-docs-operation-list-scroll-area'
    );
    Object.defineProperty(scrollArea, 'scrollHeight', {
      value: 1200,
      configurable: true
    });
    Object.defineProperty(scrollArea, 'clientHeight', {
      value: 400,
      configurable: true
    });
    Object.defineProperty(scrollArea, 'scrollTop', {
      value: 700,
      configurable: true
    });
    fireEvent.scroll(scrollArea);

    expect(
      await screen.findByRole('button', { name: /get \/api\/v1\/items\/24/i })
    ).toBeInTheDocument();
    expect(fetchCategoryOperations).toHaveBeenCalledWith(
      'application-native-api',
      { offset: 20, limit: 20, q: null }
    );
  });

  test('keeps the operation search input focused while the filtered list reloads', async () => {
    const fetchCategoryOperations = vi.fn(
      (
        categoryId: string,
        request?: { offset?: number; limit?: number; q?: string | null }
      ) =>
        Promise.resolve({
          ...operationsByCategory[
            categoryId as keyof typeof operationsByCategory
          ],
          total: operationsByCategory[
            categoryId as keyof typeof operationsByCategory
          ].operations.length,
          offset: request?.offset ?? 0,
          limit: request?.limit ?? 20,
          has_more: false,
          next_offset: null
        })
    );

    render(
      <AppProviders>
        <ApiDocsExplorer
          queryState={{
            categoryId: 'application-native-api',
            operationId: null
          }}
          onQueryStateChange={vi.fn()}
          catalogQueryKey={['api-docs', 'catalog', 'search-focus']}
          fetchCatalog={() => Promise.resolve(catalog)}
          categoryOperationsQueryKey={(categoryId) => [
            'api-docs',
            'category',
            categoryId,
            'search-focus'
          ]}
          fetchCategoryOperations={fetchCategoryOperations}
          operationSpecQueryKey={(operationId) => [
            'api-docs',
            'operation',
            operationId,
            'search-focus'
          ]}
          fetchOperationSpec={() => Promise.resolve({})}
          baseServerUrl="http://127.0.0.1:3100"
        />
      </AppProviders>
    );

    const searchInput = await screen.findByRole('textbox', {
      name: '搜索接口'
    });
    searchInput.focus();
    fireEvent.change(searchInput, { target: { value: 's' } });

    expect(searchInput).toHaveFocus();
    await waitFor(() => {
      expect(fetchCategoryOperations).toHaveBeenCalledWith(
        'application-native-api',
        { offset: 0, limit: 20, q: 's' }
      );
    });
    expect(searchInput).toHaveFocus();
  });

  test('shows all operations by default when no category is selected and aggregation is enabled', async () => {
    const fetchCategoryOperations = vi.fn((categoryId: string) =>
      Promise.resolve(
        operationsByCategory[categoryId as keyof typeof operationsByCategory]
      )
    );

    render(
      <AppProviders>
        <ApiDocsExplorer
          queryState={{ categoryId: null, operationId: null }}
          onQueryStateChange={vi.fn()}
          catalogQueryKey={['api-docs', 'catalog']}
          fetchCatalog={() => Promise.resolve(catalog)}
          categoryOperationsQueryKey={(categoryId) => [
            'api-docs',
            'category',
            categoryId
          ]}
          fetchCategoryOperations={fetchCategoryOperations}
          operationSpecQueryKey={(operationId) => [
            'api-docs',
            'operation',
            operationId
          ]}
          fetchOperationSpec={() => Promise.resolve({})}
          baseServerUrl="http://127.0.0.1:3100"
          showAllOperationsWhenNoCategory
        />
      </AppProviders>
    );

    expect(
      await screen.findByRole('combobox', { name: '接口分类' })
    ).toBeInTheDocument();
    await waitFor(() => {
      expect(fetchCategoryOperations).toHaveBeenCalledWith(
        'application-native-api',
        { offset: 0, limit: 20 }
      );
    });
    await waitFor(() => {
      expect(fetchCategoryOperations).toHaveBeenCalledWith(
        'openai-compatible-api',
        { offset: 0, limit: 20 }
      );
    });
    expect(await screen.findByText('全部分类 共 3 个接口')).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: /post \/api\/agent\/v1\/runs/i })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', {
        name: /get \/api\/agent\/v1\/runs\/\{run_id\}/i
      })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: /post \/v1\/chat\/completions/i })
    ).toBeInTheDocument();
    expect(
      screen.queryByText('选择一个分类后查看接口列表')
    ).not.toBeInTheDocument();
  });

  test('copies a Scalar operation link instead of navigating to its hash target', async () => {
    const fetchOperationSpec = vi.fn(() =>
      Promise.resolve({
        openapi: '3.1.0',
        paths: {
          '/v1/chat/completions': {
            post: {
              operationId: 'create_chat_completion'
            }
          }
        }
      })
    );
    const onQueryStateChange = vi.fn();

    render(
      <AppProviders>
        <ApiDocsExplorer
          queryState={{
            categoryId: 'openai-compatible-api',
            operationId: 'create_chat_completion'
          }}
          onQueryStateChange={onQueryStateChange}
          catalogQueryKey={['api-docs', 'catalog', 'copy-operation-link']}
          fetchCatalog={() => Promise.resolve(catalog)}
          categoryOperationsQueryKey={(categoryId) => [
            'api-docs',
            'category',
            categoryId,
            'copy-operation-link'
          ]}
          fetchCategoryOperations={(categoryId) =>
            Promise.resolve(
              operationsByCategory[
                categoryId as keyof typeof operationsByCategory
              ]
            )
          }
          operationSpecQueryKey={(operationId) => [
            'api-docs',
            'operation',
            operationId,
            'copy-operation-link'
          ]}
          fetchOperationSpec={fetchOperationSpec}
          baseServerUrl="http://127.0.0.1:3100"
        />
      </AppProviders>
    );

    const scalarViewer = await screen.findByTestId('scalar-viewer');
    const operationLink = await within(scalarViewer).findByText(
      '/v1/chat/completions'
    );
    fireEvent.click(operationLink);

    expect(copyTextToClipboard).toHaveBeenCalledWith(
      'POST /v1/chat/completions'
    );
    expect(window.location.hash).toBe('');
  });
});
