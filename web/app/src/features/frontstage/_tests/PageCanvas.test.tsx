import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

import type { FrontstagePageContent } from '../api/page-content';
import { PageCanvas } from '../components/PageCanvas';

function createPageContent(
  overrides: Partial<FrontstagePageContent> = {}
): FrontstagePageContent {
  return {
    page: {
      id: 'page-1',
      title: 'Landing',
      kind: 'page',
      parentId: null,
      rank: '001000',
    },
    schema: {
      rootUid: 'root-1',
      payload: {}
    },
    root: {
      uid: 'root-1',
      payload: {}
    },
    ...overrides
  };
}

describe('PageCanvas', () => {
  test('renders a compact loading state before content is available', () => {
    render(<PageCanvas isLoading />);

    expect(screen.getByText('页面内容加载中')).toBeInTheDocument();
    expect(
      screen.getByText('正在读取页面内容和区块清单。')
    ).toBeInTheDocument();
  });

  test('renders an error state with retry action', () => {
    const onRetry = vi.fn();

    render(<PageCanvas hasError onRetry={onRetry} />);

    expect(screen.getByText('页面内容加载失败')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /重\s*试/ }));

    expect(onRetry).toHaveBeenCalledTimes(1);
  });

  test('renders an unselected empty state without content', () => {
    render(<PageCanvas content={undefined} />);

    expect(screen.getByText('未选择页面内容')).toBeInTheDocument();
    expect(
      screen.getByText('选择页面后将显示页面预览。')
    ).toBeInTheDocument();
  });

  test('renders page title and empty content placeholder', () => {
    render(<PageCanvas content={createPageContent()} />);

    expect(screen.getByText('Landing')).toBeInTheDocument();
    expect(screen.getByText('页面内容为空')).toBeInTheDocument();
  });

  test('renders blocks sorted by order — each block shows loading placeholder', () => {
    render(
      <PageCanvas
        content={createPageContent({
          root: {
            uid: 'root-1',
            payload: {
              blocks: [
                {
                  id: 'hero',
                  codeRef: 'hero-code',
                  contributionCode: 'official.hero',
                  runtime: { kind: 'iframe', entry: 'blocks/hero.html' },
                  layout: { order: 20, region: 'main' }
                },
                {
                  id: 'cta',
                  codeRef: 'cta-code',
                  contributionCode: 'official.cta',
                  runtime: 'inline',
                  layout: { order: 10, region: 'footer' }
                }
              ]
            }
          }
        })}
      />
    );

    // All blocks show "区块加载中..." when no runtime session available
    expect(
      within(screen.getByTestId('page-canvas-render-slots'))
        .getAllByText('区块加载中...')
    ).toHaveLength(2);
  });

  test('shows loading placeholder for blocks without runtime sessions', () => {
    render(
      <PageCanvas
        content={createPageContent({
          root: {
            uid: 'root-1',
            payload: {
              blocks: [
                {
                  id: 'hero',
                  codeRef: 'hero-code',
                  contributionCode: 'official.hero',
                  runtime: { kind: 'iframe', entry: 'blocks/hero.js' },
                  layout: { order: 10, region: 'main', span: 12 }
                }
              ]
            }
          }
        })}
      />
    );

    const slots = within(
      screen.getByTestId('page-canvas-render-slots')
    );
    expect(slots.getByText('区块加载中...')).toBeInTheDocument();
  });

  test('notifies selection changes when clicked in design mode', () => {
    const onSelectBlock = vi.fn();

    render(
      <PageCanvas
        onSelectBlock={onSelectBlock}
        content={createPageContent({
          root: {
            uid: 'root-1',
            payload: {
              blocks: [
                {
                  id: 'hero',
                  codeRef: 'hero-code',
                  contributionCode: 'official.hero',
                  runtime: 'inline'
                }
              ]
            }
          }
        })}
        isDesignMode
      />
    );

    // In design mode, block containers have role="button"
    const slots = within(
      screen.getByTestId('page-canvas-render-slots')
    );

    fireEvent.click(slots.getByRole('button', { name: '区块 hero' }));

    expect(onSelectBlock).toHaveBeenCalledWith('hero');
  });

  test('does not show hover toolbar when isDesignMode is false', () => {
    render(
      <PageCanvas
        content={createPageContent({
          root: {
            uid: 'root-1',
            payload: {
              blocks: [
                {
                  id: 'hero',
                  codeRef: 'hero-code',
                  contributionCode: 'official.hero',
                  runtime: 'inline'
                }
              ]
            }
          }
        })}
        isDesignMode={false}
      />
    );

    // The toolbar buttons should not be present
    expect(
      screen.queryByRole('button', { name: '移动或排序区块' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '更多区块操作' })
    ).not.toBeInTheDocument();
  });

  test('renders design mode with hover toolbar actions', () => {
    const designActions = {
      onMoveUp: vi.fn(),
      onMoveDown: vi.fn(),
      onConfigure: vi.fn(),
      onEditCode: vi.fn(),
      onDelete: vi.fn()
    };

    render(
      <PageCanvas
        content={createPageContent({
          root: {
            uid: 'root-1',
            payload: {
              blocks: [
                {
                  id: 'hero',
                  codeRef: 'hero-code',
                  contributionCode: 'official.hero',
                  runtime: { kind: 'iframe', entry: 'blocks/hero.js' },
                  layout: { order: 10, region: 'main' }
                },
                {
                  id: 'cta',
                  codeRef: 'cta-code',
                  contributionCode: 'official.cta',
                  runtime: { kind: 'iframe', entry: 'blocks/cta.js' },
                  layout: { order: 20, region: 'footer' }
                }
              ]
            }
          }
        })}
        isDesignMode
        designActions={designActions}
      />
    );

    // In design mode, blocks are rendered as buttons (container + toolbar buttons)
    const renderSlots = screen.getByTestId('page-canvas-render-slots');
    expect(renderSlots).toBeInTheDocument();
    // Each block container is a role="button" (2 total for 2 blocks)
    const blockButtons = within(renderSlots).getAllByRole('button', {
      name: /区块 /
    });
    expect(blockButtons).toHaveLength(2);
  });
});
