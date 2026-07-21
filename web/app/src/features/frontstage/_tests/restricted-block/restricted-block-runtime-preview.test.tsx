import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

import { RestrictedBlockRuntimePreview } from '../../components/RestrictedBlockRuntimePreview';
import type { RestrictedBlockRuntimeHostSnapshot } from '../../lib/restricted-block-runtime-host';

function createSnapshot(
  overrides: Partial<RestrictedBlockRuntimeHostSnapshot> = {}
): RestrictedBlockRuntimeHostSnapshot {
  return {
    status: 'idle',
    requestId: 'restricted-block:block-1:code-1',
    blockId: 'block-1',
    schemaValidationOptions: {
      maxDepth: 8,
      maxNodes: 250,
      allowedActions: ['record.save'],
      allowedEvents: ['record.saved'],
      allowedDataPermissions: ['query']
    },
    logs: [],
    effects: [],
    rejections: [],
    ...overrides
  };
}

describe('RestrictedBlockRuntimePreview', () => {
  test('renders a ready snapshot with BlockUiRenderer and relays renderer actions through the injected callback', () => {
    const onAction = vi.fn();

    render(
      <RestrictedBlockRuntimePreview
        snapshot={createSnapshot({
          status: 'ready',
          view: {
            primitive: 'Stack',
            children: [
              { primitive: 'Title', props: { children: 'Runtime Result' } },
              {
                primitive: 'Button',
                key: 'save-button',
                props: {
                  children: 'Save record',
                  actionId: 'record.save',
                  actionPayload: { id: 'record-1' }
                },
                permissions: { actions: ['record.save'] }
              }
            ]
          },
          logs: [
            {
              requestId: 'restricted-block:block-1:code-1',
              level: 'info',
              message: 'rendered',
              data: { hidden: 'raw-log-value' }
            }
          ],
          effects: [
            {
              type: 'interface',
              requestId: 'restricted-block:block-1:code-1',
              effectId: 'effect-1',
              interfaceId: 'save_record',
              schemaDigest: 'digest-save-record',
              request: { hidden: 'raw-effect-value' }
            }
          ],
          rejections: [
            {
              code: 'invalid_message',
              path: 'worker.message',
              message: 'Ignored malformed worker message.',
              requestId: 'restricted-block:block-1:code-1'
            }
          ]
        })}
        onAction={onAction}
      />
    );

    expect(screen.queryByText('运行结果')).not.toBeInTheDocument();
    expect(
      screen.getByRole('heading', { name: 'Runtime Result' })
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Save record' }));
    expect(onAction).toHaveBeenCalledWith({
      type: 'action',
      primitive: 'Button',
      key: 'save-button',
      actionId: 'record.save',
      payload: { id: 'record-1' }
    });

    expect(screen.queryByText('日志')).not.toBeInTheDocument();
    expect(screen.queryByText('1 条')).not.toBeInTheDocument();
    expect(screen.queryByText('效果')).not.toBeInTheDocument();
    expect(screen.queryByText('接口: save_record')).not.toBeInTheDocument();
    expect(screen.queryByText('拒绝项')).not.toBeInTheDocument();
    expect(screen.queryByText('invalid_message')).not.toBeInTheDocument();
    expect(screen.queryByText(/raw-log-value/)).not.toBeInTheDocument();
    expect(screen.queryByText(/raw-effect-value/)).not.toBeInTheDocument();
    expect(screen.queryByText(/\{"hidden"/)).not.toBeInTheDocument();
  });

  test('renders failed and timed out snapshots as controlled error summaries', () => {
    const { rerender } = render(
      <RestrictedBlockRuntimePreview
        snapshot={createSnapshot({
          status: 'failed',
          error: {
            kind: 'runtime_error',
            message: 'Worker crashed while rendering.',
            errors: [
              {
                code: 'runtime_error',
                path: 'runtime.render',
                message: 'Worker crashed while rendering.'
              }
            ]
          }
        })}
      />
    );

    expect(screen.getByText('运行失败')).toBeInTheDocument();
    expect(
      screen.getAllByText('Worker crashed while rendering.').length
    ).toBeGreaterThan(0);
    expect(screen.queryByText('runtime_error')).not.toBeInTheDocument();
    expect(screen.queryByText('runtime.render')).not.toBeInTheDocument();
    expect(screen.queryByText(/errors/)).not.toBeInTheDocument();

    rerender(
      <RestrictedBlockRuntimePreview
        snapshot={createSnapshot({
          status: 'timed_out',
          error: {
            kind: 'runtime_timeout',
            message: 'JS block runtime timed out.',
            errors: [
              {
                code: 'runtime_timeout',
                path: 'runtime.timeout',
                message: 'JS block runtime timed out.'
              }
            ]
          }
        })}
      />
    );

    expect(screen.getByText('运行超时')).toBeInTheDocument();
    expect(screen.queryByText('runtime_timeout')).not.toBeInTheDocument();
    expect(screen.queryByText('runtime.timeout')).not.toBeInTheDocument();
  });

  test('renders idle and running as local loading shells while keeping disposed explicit', () => {
    const { rerender } = render(
      <RestrictedBlockRuntimePreview
        snapshot={createSnapshot({ status: 'idle' })}
      />
    );

    expect(screen.getByTestId('block-ui-loading-shell')).toHaveAttribute(
      'aria-busy',
      'true'
    );
    expect(screen.queryByText('尚未运行')).not.toBeInTheDocument();
    expect(
      screen.queryByText(/restricted-block:block-1:code-1/)
    ).not.toBeInTheDocument();

    rerender(
      <RestrictedBlockRuntimePreview
        snapshot={createSnapshot({
          status: 'running',
          logs: [
            {
              requestId: 'restricted-block:block-1:code-1',
              level: 'info',
              message: 'booting'
            }
          ]
        })}
      />
    );
    expect(screen.getByTestId('block-ui-loading-shell')).toBeInTheDocument();
    expect(screen.queryByText('运行中')).not.toBeInTheDocument();
    expect(screen.queryByText('booting')).not.toBeInTheDocument();
    expect(screen.queryByText('日志')).not.toBeInTheDocument();

    rerender(
      <RestrictedBlockRuntimePreview
        snapshot={createSnapshot({ status: 'disposed' })}
      />
    );
    expect(
      screen.queryByTestId('block-ui-loading-shell')
    ).not.toBeInTheDocument();
    expect(screen.getByText('已释放')).toBeInTheDocument();
  });
});
