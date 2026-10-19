import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { useState } from 'react';
import { describe, expect, test, vi } from 'vitest';

import { TemplatedTextField } from '../../components/bindings/TemplatedTextField';
import type { FlowSelectorOption } from '../../lib/selector-options';

const startQueryOption: FlowSelectorOption = {
  nodeId: 'node-start',
  nodeLabel: 'Start',
  outputKey: 'query',
  outputLabel: 'query',
  valueType: 'string',
  value: ['node-start', 'query'],
  displayLabel: 'Start/query'
};

function PromptPairHarness() {
  const [systemValue, setSystemValue] = useState('');
  const [userValue, setUserValue] = useState('');

  return (
    <>
      <TemplatedTextField
        label="System Prompt"
        ariaLabel="System Prompt"
        options={[startQueryOption]}
        value={systemValue}
        onChange={setSystemValue}
      />
      <TemplatedTextField
        label="User Prompt"
        ariaLabel="User Prompt"
        options={[startQueryOption]}
        value={userValue}
        onChange={setUserValue}
      />
      <output data-testid="system-value">{systemValue}</output>
      <output data-testid="user-value">{userValue}</output>
    </>
  );
}

function PromptWithPlainInputHarness() {
  const [systemValue, setSystemValue] = useState('');
  const [plainValue, setPlainValue] = useState('');

  return (
    <>
      <TemplatedTextField
        label="System Prompt"
        ariaLabel="System Prompt"
        options={[startQueryOption]}
        value={systemValue}
        onChange={setSystemValue}
      />
      <input
        aria-label="Plain Input"
        value={plainValue}
        onChange={(event) => {
          setPlainValue(event.target.value);
          setSystemValue('Synced prompt');
        }}
      />
      <output data-testid="system-value">{systemValue}</output>
      <output data-testid="plain-value">{plainValue}</output>
    </>
  );
}

describe('TemplatedTextField focus and layout', () => {
  test('aligns the empty placeholder with the first editor line', async () => {
    const cssSource = await readFile(
      path.resolve(
        process.cwd(),
        'src/features/agent-flow/components/editor/styles/inspector.css'
      ),
      'utf8'
    );

    expect(cssSource).toContain(
      '.agent-flow-templated-text-field__placeholder {\n  position: absolute;\n  top: 4px;'
    );
  });

  test('keeps inline variable chips within the editor text line height', async () => {
    const cssSource = await readFile(
      path.resolve(
        process.cwd(),
        'src/features/agent-flow/components/editor/styles/inspector.css'
      ),
      'utf8'
    );

    expect(cssSource).toContain(
      '.agent-flow-templated-text-field__chip {\n  display: inline-flex;\n  align-items: center;\n  box-sizing: border-box;\n  height: 20px;'
    );
    expect(cssSource).toContain('line-height: 18px;');
  });

  test('supports a single-line input mode for compact variable bindings', async () => {
    render(
      <TemplatedTextField
        ariaLabel="Code Arg"
        displayMode="input"
        label="Code Arg"
        options={[startQueryOption]}
        value="{{node-start.query}}"
        onChange={vi.fn()}
      />
    );

    const codeArg = screen.getByLabelText('Code Arg');

    expect(codeArg).toHaveAttribute('aria-multiline', 'false');
    expect(
      screen.queryByRole('button', { name: '复制Code Arg' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '放大编辑Code Arg' })
    ).not.toBeInTheDocument();
    expect(
      await screen.findByTestId('templated-text-inline-chip')
    ).toBeInTheDocument();
  });

  test('focuses the matching editor when clicking an embedded field label', async () => {
    render(
      <>
        <TemplatedTextField
          label="System Prompt"
          ariaLabel="System Prompt"
          options={[startQueryOption]}
          value=""
          onChange={vi.fn()}
        />
        <TemplatedTextField
          label="User Prompt"
          ariaLabel="User Prompt"
          options={[startQueryOption]}
          value=""
          onChange={vi.fn()}
        />
      </>
    );

    const systemEditor = screen.getByLabelText('System Prompt');
    const userEditor = screen.getByLabelText('User Prompt');

    userEditor.focus();
    expect(userEditor).toHaveFocus();

    fireEvent.mouseDown(screen.getByText('System Prompt'));

    await waitFor(() => {
      expect(systemEditor).toHaveFocus();
    });
  });

  test('focuses the matching editor when clicking the editor box itself', async () => {
    render(
      <>
        <TemplatedTextField
          label="System Prompt"
          ariaLabel="System Prompt"
          options={[startQueryOption]}
          value=""
          onChange={vi.fn()}
        />
        <TemplatedTextField
          label="User Prompt"
          ariaLabel="User Prompt"
          options={[startQueryOption]}
          value=""
          onChange={vi.fn()}
        />
      </>
    );

    const systemEditor = screen.getByLabelText('System Prompt');
    const userEditor = screen.getByLabelText('User Prompt');

    userEditor.focus();
    expect(userEditor).toHaveFocus();

    fireEvent.mouseDown(systemEditor);
    fireEvent.click(systemEditor);

    await waitFor(() => {
      expect(systemEditor).toHaveFocus();
    });
  });

  test('keeps focus on system prompt after visiting user prompt then returning to system prompt', async () => {
    render(<PromptPairHarness />);

    const systemEditor = screen.getByLabelText('System Prompt');
    const userEditor = screen.getByLabelText('User Prompt');

    fireEvent.mouseDown(systemEditor);
    fireEvent.click(systemEditor);

    await waitFor(() => {
      expect(systemEditor).toHaveFocus();
    });

    fireEvent.mouseDown(userEditor);
    fireEvent.click(userEditor);

    await waitFor(() => {
      expect(userEditor).toHaveFocus();
    });

    fireEvent.mouseDown(systemEditor);
    fireEvent.click(systemEditor);

    await waitFor(() => {
      expect(systemEditor).toHaveFocus();
    });
  });

  test('does not emit content changes when only switching prompt focus', async () => {
    const handleSystemChange = vi.fn();
    const handleUserChange = vi.fn();

    render(
      <>
        <TemplatedTextField
          label="System Prompt"
          ariaLabel="System Prompt"
          options={[startQueryOption]}
          value=""
          onChange={handleSystemChange}
        />
        <TemplatedTextField
          label="User Prompt"
          ariaLabel="User Prompt"
          options={[startQueryOption]}
          value=""
          onChange={handleUserChange}
        />
      </>
    );

    const systemEditor = screen.getByLabelText('System Prompt');
    const userEditor = screen.getByLabelText('User Prompt');

    fireEvent.mouseDown(systemEditor);
    fireEvent.click(systemEditor);
    fireEvent.focus(systemEditor);

    fireEvent.mouseDown(userEditor);
    fireEvent.click(userEditor);
    fireEvent.focus(userEditor);

    fireEvent.mouseDown(systemEditor);
    fireEvent.click(systemEditor);
    fireEvent.focus(systemEditor);

    await waitFor(() => {
      expect(systemEditor).toHaveFocus();
    });
    expect(handleSystemChange).not.toHaveBeenCalled();
    expect(handleUserChange).not.toHaveBeenCalled();
  });

  test('keeps focus on a plain input when the templated value syncs from state', async () => {
    render(<PromptWithPlainInputHarness />);

    const plainInput = screen.getByLabelText('Plain Input');

    plainInput.focus();
    expect(plainInput).toHaveFocus();
    fireEvent.change(plainInput, {
      target: { value: 'Next field' }
    });

    await waitFor(() => {
      expect(screen.getByTestId('system-value')).toHaveTextContent(
        'Synced prompt'
      );
    });
    expect(plainInput).toHaveFocus();
    expect(screen.getByTestId('plain-value')).toHaveTextContent('Next field');
  });

  test('focuses the owning prompt editor when clicking an inline variable chip', async () => {
    render(
      <>
        <TemplatedTextField
          label="System Prompt"
          ariaLabel="System Prompt"
          options={[startQueryOption]}
          value=""
          onChange={vi.fn()}
        />
        <TemplatedTextField
          label="User Prompt"
          ariaLabel="User Prompt"
          options={[startQueryOption]}
          value="{{node-start.query}}"
          onChange={vi.fn()}
        />
      </>
    );

    const systemEditor = screen.getByLabelText('System Prompt');
    const userEditor = screen.getByLabelText('User Prompt');

    systemEditor.focus();
    expect(systemEditor).toHaveFocus();

    fireEvent.mouseDown(
      await screen.findByTestId('templated-text-inline-chip')
    );

    await waitFor(() => {
      expect(userEditor).toHaveFocus();
    });
  });
});
