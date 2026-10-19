import { fireEvent, render, screen } from '@testing-library/react';
import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { DebugVariablesPane } from '../../components/debug-console/variables/DebugVariablesPane';
import { AgentFlowSideDock } from '../../components/editor/AgentFlowSideDock';

describe('variable selection and dock keyboard surfaces', () => {
  it('exposes selectable variables as focusable native buttons', () => {
    render(
      <DebugVariablesPane
        groups={[
          {
            title: 'Values',
            items: [
              { key: 'first', label: 'First variable', value: 'first value' },
              { key: 'second', label: 'Second variable', value: 'second value' }
            ]
          }
        ]}
      />
    );
    const second = screen.getByRole('button', { name: 'Second variable' });
    expect(second.tagName).toBe('BUTTON');
    second.focus();
    expect(second).toHaveFocus();
    fireEvent.click(second);
    expect(second).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByRole('textbox')).toHaveValue('second value');
  });

  it('forwards sidebar resize keys while exposing the current value and bounds', () => {
    const onSidebarResizeKeyDown = vi.fn();
    render(
      <DebugVariablesPane
        groups={[
          {
            title: 'Values',
            items: [{ key: 'first', label: 'First', value: 'value' }]
          }
        ]}
        sidebarWidth={260}
        sidebarMinWidth={140}
        sidebarMaxWidth={400}
        onSidebarResizeKeyDown={onSidebarResizeKeyDown}
      />
    );
    const handle = screen.getByRole('separator');
    handle.focus();
    expect(handle).toHaveFocus();
    expect(handle).toHaveAttribute('aria-valuenow', '260');
    expect(handle).toHaveAttribute('aria-valuemin', '140');
    expect(handle).toHaveAttribute('aria-valuemax', '400');
    fireEvent.keyDown(handle, { key: 'ArrowRight' });
    expect(onSidebarResizeKeyDown).toHaveBeenCalledOnce();
  });

  it('keeps the dock separator focused as its owner changes the width', () => {
    function Dock() {
      const [width, setWidth] = useState(300);
      return (
        <AgentFlowSideDock
          width={width}
          minWidth={200}
          maxWidth={500}
          resizeLabel="Resize dock"
          onResizeStart={() => {}}
          onResizeKeyDown={(event) => {
            if (event.key === 'ArrowLeft') setWidth((current) => current + 24);
          }}
        >
          <span>Dock content</span>
        </AgentFlowSideDock>
      );
    }
    render(<Dock />);
    const handle = screen.getByRole('separator', { name: 'Resize dock' });
    handle.focus();
    fireEvent.keyDown(handle, { key: 'ArrowLeft' });
    expect(handle).toHaveAttribute('aria-valuenow', '324');
    expect(handle).toHaveFocus();
  });
});
