import { describe, test, expect, vi } from 'vitest';
import {
  SelectionSeed,
  createInitialState,
  mockElementRect,
  openModelSettings,
  renderWithProviders
} from './llm-model-provider-field.fixtures';
import { AgentFlowEditorStoreProvider } from '../../store/editor/AgentFlowEditorStoreProvider';
import { NodeConfigTab } from '../../components/detail/tabs/NodeConfigTab';
import { act, fireEvent, screen, waitFor } from '@testing-library/react';

describe('LlmModelField', () => {
  test('renders the model settings inside the canvas body with minimum width and half-height', async () => {
    const { container } = renderWithProviders(
      <div className="agent-flow-editor__body">
        <AgentFlowEditorStoreProvider initialState={createInitialState()}>
          <SelectionSeed nodeId="node-llm" />
          <NodeConfigTab />
        </AgentFlowEditorStoreProvider>
      </div>
    );

    const editorBody = container.querySelector('.agent-flow-editor__body');
    const trigger = await screen.findByRole('button', { name: '模型' });

    if (!editorBody) {
      throw new Error('expected editor body container');
    }

    mockElementRect(editorBody, {
      left: 0,
      top: 0,
      width: 1200,
      height: 800
    });
    mockElementRect(trigger, {
      left: 980,
      top: 160,
      width: 240,
      height: 40
    });

    fireEvent.click(trigger);

    const dialog = await screen.findByRole('dialog', { name: '模型设置' });

    expect(dialog).toHaveClass('agent-flow-model-settings__panel');
    expect(editorBody.contains(dialog)).toBe(true);
    expect(dialog).toHaveStyle({
      width: '320px',
      height: '400px'
    });
  });

  test('opens the model settings panel in the blank area left of the node detail panel', async () => {
    const { container } = renderWithProviders(
      <div className="agent-flow-editor__body">
        <div className="agent-flow-node-detail">
          <AgentFlowEditorStoreProvider initialState={createInitialState()}>
            <SelectionSeed nodeId="node-llm" />
            <NodeConfigTab />
          </AgentFlowEditorStoreProvider>
        </div>
      </div>
    );

    const editorBody = container.querySelector('.agent-flow-editor__body');
    const nodeDetail = container.querySelector('.agent-flow-node-detail');
    const trigger = await screen.findByRole('button', { name: '模型' });

    if (!editorBody || !nodeDetail) {
      throw new Error('expected editor body and node detail container');
    }

    mockElementRect(editorBody, {
      left: 0,
      top: 0,
      width: 1200,
      height: 800
    });
    mockElementRect(nodeDetail, {
      left: 760,
      top: 40,
      width: 420,
      height: 720
    });
    mockElementRect(trigger, {
      left: 980,
      top: 180,
      width: 180,
      height: 40
    });

    fireEvent.click(trigger);

    const dialog = await screen.findByRole('dialog', { name: '模型设置' });

    expect(dialog).toHaveStyle({
      left: '416px',
      top: '180px'
    });
  });

  test('closes the model settings panel two seconds after the mouse leaves it', async () => {
    renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        <SelectionSeed nodeId="node-llm" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    await openModelSettings();

    vi.useFakeTimers();
    try {
      const dialog = screen.getByRole('dialog', { name: '模型设置' });

      fireEvent.mouseLeave(dialog);
      act(() => {
        vi.advanceTimersByTime(1999);
      });

      expect(
        screen.getByRole('dialog', { name: '模型设置' })
      ).toBeInTheDocument();

      act(() => {
        vi.advanceTimersByTime(1);
      });

      expect(
        screen.queryByRole('dialog', { name: '模型设置' })
      ).not.toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });

  test('keeps the model settings panel open when the mouse returns before auto close', async () => {
    renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        <SelectionSeed nodeId="node-llm" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    await openModelSettings();

    vi.useFakeTimers();
    try {
      const dialog = screen.getByRole('dialog', { name: '模型设置' });

      fireEvent.mouseLeave(dialog);
      act(() => {
        vi.advanceTimersByTime(1500);
      });
      fireEvent.mouseEnter(dialog);
      act(() => {
        vi.advanceTimersByTime(500);
      });

      expect(
        screen.getByRole('dialog', { name: '模型设置' })
      ).toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });

  test('resizes the model settings panel width from both sides while keeping the 320px minimum', async () => {
    const { container } = renderWithProviders(
      <div className="agent-flow-editor__body">
        <AgentFlowEditorStoreProvider initialState={createInitialState()}>
          <SelectionSeed nodeId="node-llm" />
          <NodeConfigTab />
        </AgentFlowEditorStoreProvider>
      </div>
    );

    const editorBody = container.querySelector('.agent-flow-editor__body');
    const trigger = await screen.findByRole('button', { name: '模型' });

    if (!editorBody) {
      throw new Error('expected editor body container');
    }

    mockElementRect(editorBody, {
      left: 0,
      top: 0,
      width: 1200,
      height: 800
    });
    mockElementRect(trigger, {
      left: 980,
      top: 160,
      width: 240,
      height: 40
    });

    fireEvent.click(trigger);

    const dialog = await screen.findByRole('dialog', { name: '模型设置' });
    const leftResizeHandle = screen.getByTestId(
      'agent-flow-model-settings-resize-handle-left'
    );
    const rightResizeHandle = screen.getByTestId(
      'agent-flow-model-settings-resize-handle'
    );

    fireEvent.mouseDown(rightResizeHandle, {
      clientX: 320,
      clientY: 180
    });
    fireEvent.mouseMove(window, {
      clientX: 460,
      clientY: 180
    });
    fireEvent.mouseUp(window);

    await waitFor(() => {
      expect(dialog).toHaveStyle({
        width: '460px'
      });
    });

    fireEvent.mouseDown(rightResizeHandle, {
      clientX: 460,
      clientY: 180
    });
    fireEvent.mouseMove(window, {
      clientX: 120,
      clientY: 180
    });
    fireEvent.mouseUp(window);

    await waitFor(() => {
      expect(dialog).toHaveStyle({
        width: '320px'
      });
    });

    fireEvent.mouseDown(leftResizeHandle, {
      clientX: 636,
      clientY: 180
    });
    fireEvent.mouseMove(window, {
      clientX: 496,
      clientY: 180
    });
    fireEvent.mouseUp(window);

    await waitFor(() => {
      expect(dialog).toHaveStyle({
        left: '496px',
        width: '460px'
      });
    });

    fireEvent.mouseDown(leftResizeHandle, {
      clientX: 496,
      clientY: 180
    });
    fireEvent.mouseMove(window, {
      clientX: 760,
      clientY: 180
    });
    fireEvent.mouseUp(window);

    await waitFor(() => {
      expect(dialog).toHaveStyle({
        left: '636px',
        width: '320px'
      });
    });
  });

  test('resizes the model settings panel height from the bottom while keeping the minimum height', async () => {
    const { container } = renderWithProviders(
      <div className="agent-flow-editor__body">
        <AgentFlowEditorStoreProvider initialState={createInitialState()}>
          <SelectionSeed nodeId="node-llm" />
          <NodeConfigTab />
        </AgentFlowEditorStoreProvider>
      </div>
    );

    const editorBody = container.querySelector('.agent-flow-editor__body');
    const trigger = await screen.findByRole('button', { name: '模型' });

    if (!editorBody) {
      throw new Error('expected editor body container');
    }

    mockElementRect(editorBody, {
      left: 0,
      top: 0,
      width: 1200,
      height: 800
    });
    mockElementRect(trigger, {
      left: 980,
      top: 160,
      width: 240,
      height: 40
    });

    fireEvent.click(trigger);

    const dialog = await screen.findByRole('dialog', { name: '模型设置' });
    const bottomResizeHandle = screen.getByRole('separator', {
      name: '向下调整模型设置高度'
    });

    expect(dialog).toHaveStyle({
      height: '400px'
    });

    fireEvent.mouseDown(bottomResizeHandle, {
      clientX: 520,
      clientY: 560
    });
    fireEvent.mouseMove(window, {
      clientX: 520,
      clientY: 660
    });
    fireEvent.mouseUp(window);

    await waitFor(() => {
      expect(dialog).toHaveStyle({
        height: '500px'
      });
    });

    fireEvent.mouseDown(bottomResizeHandle, {
      clientX: 520,
      clientY: 660
    });
    fireEvent.mouseMove(window, {
      clientX: 520,
      clientY: 260
    });
    fireEvent.mouseUp(window);

    await waitFor(() => {
      expect(dialog).toHaveStyle({
        height: '240px'
      });
    });
  });

  test('supports dragging the model settings panel within the canvas bounds', async () => {
    const { container } = renderWithProviders(
      <div className="agent-flow-editor__body">
        <AgentFlowEditorStoreProvider initialState={createInitialState()}>
          <SelectionSeed nodeId="node-llm" />
          <NodeConfigTab />
        </AgentFlowEditorStoreProvider>
      </div>
    );

    const editorBody = container.querySelector('.agent-flow-editor__body');
    const trigger = await screen.findByRole('button', { name: '模型' });

    if (!editorBody) {
      throw new Error('expected editor body container');
    }

    mockElementRect(editorBody, {
      left: 0,
      top: 0,
      width: 1200,
      height: 800
    });
    mockElementRect(trigger, {
      left: 980,
      top: 160,
      width: 240,
      height: 40
    });

    fireEvent.click(trigger);

    const dialog = await screen.findByRole('dialog', { name: '模型设置' });
    const dragHandle = screen.getByTestId(
      'agent-flow-model-settings-drag-handle'
    );

    fireEvent.mouseDown(dragHandle, {
      clientX: 680,
      clientY: 180
    });
    fireEvent.mouseMove(window, {
      clientX: -120,
      clientY: -80
    });
    fireEvent.mouseUp(window);

    await waitFor(() => {
      expect(dialog).toHaveStyle({
        left: '16px',
        top: '16px'
      });
    });
  });
});
