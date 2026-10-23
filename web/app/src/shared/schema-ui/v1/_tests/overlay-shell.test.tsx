import { render, screen } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

import { SchemaDockPanel } from '../overlay-shell/SchemaDockPanel';

describe('overlay shell runtime', () => {
  test('keeps dock panel width controlled by the caller and avoids internal resize chrome', () => {
    const onClose = vi.fn();

    const { rerender } = render(
      <SchemaDockPanel
        schema={{
          schemaVersion: '1.0.0',
          shellType: 'dock_panel',
          title: '节点详情'
        }}
        width={520}
        footer={<button type="button">保存</button>}
        onClose={onClose}
      >
        <div>dock body</div>
      </SchemaDockPanel>
    );

    expect(screen.getByRole('button', { name: '保存' })).toBeInTheDocument();
    expect(screen.getByLabelText('节点详情')).toHaveStyle({ width: '520px' });
    expect(screen.queryByRole('separator', { name: '调整节点详情宽度' })).not.toBeInTheDocument();

    rerender(
      <SchemaDockPanel
        schema={{
          schemaVersion: '1.0.0',
          shellType: 'dock_panel',
          title: '节点详情'
        }}
        width={640}
        footer={<button type="button">保存</button>}
        onClose={onClose}
      >
        <div>dock body</div>
      </SchemaDockPanel>
    );

    expect(screen.getByLabelText('节点详情')).toHaveStyle({ width: '640px' });
  });
});
