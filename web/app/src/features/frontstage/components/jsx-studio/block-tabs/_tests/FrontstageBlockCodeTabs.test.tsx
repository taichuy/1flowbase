import { fireEvent, render, screen, within } from '@testing-library/react';
import { App } from 'antd';
import { describe, expect, test, vi } from 'vitest';

import { FrontstageBlockCodeTabs } from '../FrontstageBlockCodeTabs';
import type { FrontstageBlockCodeTabState } from '../use-frontstage-block-tabs';

function tab(
  blockId: string,
  draft = `source:${blockId}`
): FrontstageBlockCodeTabState {
  return {
    block_id: blockId,
    detail: null,
    base_source: `source:${blockId}`,
    draft,
    source_sha256: `hash:${blockId}`,
    loading: false,
    saving: false,
    error: null
  };
}

describe('FrontstageBlockCodeTabs', () => {
  test('AC-005 keeps the initial root open and activates real block IDs', () => {
    const onActivate = vi.fn();
    const onClose = vi.fn();
    const view = render(
      <App>
        <FrontstageBlockCodeTabs
          activeBlockId="root"
          initialBlockId="root"
          tabs={[tab('root'), tab('child')]}
          onActivate={onActivate}
          onClose={onClose}
        />
      </App>
    );

    expect(view.container.querySelectorAll('.ant-tabs-tab-remove')).toHaveLength(
      1
    );
    fireEvent.click(screen.getByText('child'));
    expect(onActivate).toHaveBeenCalledWith('child');
  });

  test('AC-005 cancels or confirms closing a dirty secondary tab', async () => {
    const onClose = vi.fn();
    const view = render(
      <App>
        <FrontstageBlockCodeTabs
          activeBlockId="child"
          initialBlockId="root"
          tabs={[tab('root'), tab('child', 'dirty child')]}
          onActivate={vi.fn()}
          onClose={onClose}
        />
      </App>
    );
    const close = view.container.querySelector('.ant-tabs-tab-remove');
    fireEvent.click(close as Element);
    const cancelDialog = await screen.findByRole('dialog');
    expect(cancelDialog).toHaveAccessibleName(
      /关闭未保存的区块|Close this unsaved block/u
    );
    fireEvent.click(
      within(cancelDialog).getByRole('button', { name: /取\s*消|Cancel/u })
    );
    expect(onClose).not.toHaveBeenCalled();

    fireEvent.click(close as Element);
    const confirmDialog = await screen.findByRole('dialog');
    fireEvent.click(
      within(confirmDialog).getByRole('button', { name: /关\s*闭|Close/u })
    );
    expect(onClose).toHaveBeenCalledWith('child');
  });
});
