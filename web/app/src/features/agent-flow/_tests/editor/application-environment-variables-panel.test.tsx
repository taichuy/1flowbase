import { authenticateEditor } from '../fixtures/editor-auth';
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { Profiler } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { ApplicationEnvironmentVariablesPanel } from '../../components/editor/ApplicationEnvironmentVariablesPanel';
import { EnvironmentVariableValueEditor } from '../../components/editor/environment-variables/EnvironmentVariableValueEditor';
import type { AgentFlowEnvironmentVariable } from '../../lib/variables/application-environment-variables';
import { i18nText } from '../../../../shared/i18n/text';

beforeEach(() => authenticateEditor());

describe('ApplicationEnvironmentVariablesPanel', () => {
  test('switches the value editor when the variable type changes', async () => {
    render(
      <ApplicationEnvironmentVariablesPanel
        variables={[]}
        onClose={vi.fn()}
        onSave={vi.fn()}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: /添加环境变量/ }));

    expect(screen.getByPlaceholderText('请输入变量值')).toBeInTheDocument();

    fireEvent.mouseDown(screen.getByRole('combobox', { name: '类型' }));
    fireEvent.click(await screen.findByTitle('number'));

    expect(await screen.findByRole('spinbutton')).toBeInTheDocument();

    fireEvent.mouseDown(screen.getByRole('combobox', { name: '类型' }));
    fireEvent.click(await screen.findByTitle('boolean'));

    await waitFor(() => {
      expect(screen.getByText('true')).toBeInTheDocument();
      expect(screen.getByText('false')).toBeInTheDocument();
    });
  }, 10000);

  test('edits object values as field rows', () => {
    const onChange = vi.fn();

    render(
      <EnvironmentVariableValueEditor
        value={{ ApiBaseUrl: '' }}
        valueType="object"
        onChange={onChange}
      />
    );

    fireEvent.change(screen.getByLabelText('对象值 1'), {
      target: { value: 'https://api.example.com' }
    });

    expect(onChange).toHaveBeenLastCalledWith({
      ApiBaseUrl: 'https://api.example.com'
    });
  });

  test('keeps a newly added object field editable before it has a key', () => {
    const onChange = vi.fn();

    render(
      <EnvironmentVariableValueEditor
        value={{}}
        valueType="object"
        onChange={onChange}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: '添加字段' }));

    expect(screen.getByLabelText('对象键 2')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('对象键 2'), {
      target: { value: 'ApiBaseUrl' }
    });
    fireEvent.change(screen.getByLabelText('对象值 2'), {
      target: { value: 'https://api.example.com' }
    });

    expect(onChange).toHaveBeenLastCalledWith({
      ApiBaseUrl: 'https://api.example.com'
    });
  });

  test('edits array string values as item rows', () => {
    const onChange = vi.fn();

    render(
      <EnvironmentVariableValueEditor
        value={['first', '']}
        valueType="array[string]"
        onChange={onChange}
      />
    );

    fireEvent.change(screen.getByLabelText('数组值 2'), {
      target: { value: 'second' }
    });

    expect(onChange).toHaveBeenLastCalledWith(['first', 'second']);
  });

  test('edits array object values as object field rows', () => {
    const onChange = vi.fn();

    render(
      <EnvironmentVariableValueEditor
        value={[{}]}
        valueType="array[object]"
        onChange={onChange}
      />
    );

    expect(screen.getByLabelText('数组对象 1')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('数组对象 1 字段键 1'), {
      target: { value: 'ApiBaseUrl' }
    });
    fireEvent.change(screen.getByLabelText('数组对象 1 字段值 1'), {
      target: { value: 'https://api.example.com' }
    });

    expect(onChange).toHaveBeenLastCalledWith([
      { ApiBaseUrl: 'https://api.example.com' }
    ]);
  });
});

interface VariableRow {
  name: string | null;
  value: string | null;
}

function variable(name: string, value: string): AgentFlowEnvironmentVariable {
  return { name, value_type: 'string', value, description: '' };
}

function visibleVariableRows(): VariableRow[] {
  const panel = screen.getByRole('region', {
    name: i18nText('agentFlow', 'auto.environment_variables')
  });
  return Array.from(panel.querySelectorAll('.agent-flow-editor__environment-variable-row'))
    .map((row) => ({
      name: row.querySelector('.agent-flow-editor__environment-variable-title strong')?.textContent ?? null,
      value: row.querySelector('.agent-flow-editor__environment-variable-value')?.textContent ?? null
    }));
}

function mountVariableSource(initialSource: AgentFlowEnvironmentVariable[]) {
  const commits: Array<{ action: string; rows: VariableRow[] }> = [];
  const onSave = vi.fn();
  const onClose = vi.fn();
  let source = initialSource;
  let loading = false;
  let action = 'mount';
  const scene = () => (
    <Profiler id="environment-variable-source-commits" onRender={() => {
      // Observe actual rendered list rows, including descendant Form commits.
      commits.push({ action, rows: visibleVariableRows() });
    }}>
      <ApplicationEnvironmentVariablesPanel
        variables={source} loading={loading} onClose={onClose} onSave={onSave}
      />
    </Profiler>
  );
  const view = render(scene());
  return {
    onSave,
    replace(nextSource: AgentFlowEnvironmentVariable[], nextAction: string, pending = false) {
      source = nextSource;
      action = nextAction;
      loading = pending;
      view.rerender(scene());
    },
    firstRows(expectedAction: string) {
      const matching = commits.filter((commit) => commit.action === expectedAction);
      expect(matching.length).toBeGreaterThan(0);
      return matching[0].rows;
    }
  };
}

async function fillNewVariable(name: string, value: string) {
  fireEvent.click(screen.getByRole('button', {
    name: (accessibleName) => accessibleName.endsWith(
      i18nText('agentFlow', 'auto.add_environment_variables')
    )
  }));
  const dialog = await screen.findByRole('dialog');
  fireEvent.change(within(dialog).getByRole('textbox', {
    name: i18nText('agentFlow', 'auto.name')
  }), { target: { value: name } });
  fireEvent.change(within(dialog).getByPlaceholderText(
    i18nText('agentFlow', 'auto.enter_variable_value')
  ), { target: { value } });
  return dialog;
}

function textButtonName(key: string) {
  const text = i18nText('agentFlow', key).replace(/\s+/g, '');
  return (accessibleName: string) => accessibleName.replace(/\s+/g, '') === text;
}

function clickSave(dialog: HTMLElement) {
  fireEvent.click(within(dialog).getByRole('button', {
    name: textButtonName('auto.save')
  }));
}

describe('ApplicationEnvironmentVariablesPanel source and optimistic submission ownership', () => {
  test('retains a submitted list for the same source and resets it on the first new-source commit without reviving it later', async () => {
    const sourceA = [variable('SourceA', 'original-a')];
    const sourceB = [variable('SourceB', 'replacement-b')];
    const owner = mountVariableSource(sourceA);
    const dialog = await fillNewVariable('Optimistic', 'submitted-a');
    clickSave(dialog);
    await waitFor(() => expect(owner.onSave).toHaveBeenCalledExactlyOnceWith([
      ...sourceA, variable('Optimistic', 'submitted-a')
    ]));
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(visibleVariableRows()).toEqual([
      { name: 'env.SourceA', value: 'original-a' },
      { name: 'env.Optimistic', value: 'submitted-a' }
    ]);

    owner.replace(sourceA, 'same-source-pending', true);
    owner.replace(sourceA, 'same-source-no-replacement');
    // The public callback is void: absent replacement props, a failed save has
    // the same current contract as any other same-source rerender, with no rollback.
    expect(visibleVariableRows()).toEqual([
      { name: 'env.SourceA', value: 'original-a' },
      { name: 'env.Optimistic', value: 'submitted-a' }
    ]);

    owner.replace(sourceB, 'replace-source-B');
    expect(owner.firstRows('replace-source-B')).toEqual([
      { name: 'env.SourceB', value: 'replacement-b' }
    ]);
    owner.replace(sourceA, 'return-original-source-A');
    expect(owner.firstRows('return-original-source-A')).toEqual([
      { name: 'env.SourceA', value: 'original-a' }
    ]);
    expect(owner.onSave).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole('button', {
      name: i18nText('agentFlow', 'auto.delete_item', { value1: 'SourceA' })
    }));
    fireEvent.click(await screen.findByRole('button', {
      name: textButtonName('auto.delete')
    }));
    expect(owner.onSave).toHaveBeenLastCalledWith([]);
    expect(visibleVariableRows()).toEqual([]);
    owner.replace(sourceA, 'same-source-after-delete');
    expect(visibleVariableRows()).toEqual([]);
  });

  test('uses a replacement source immediately while preserving the independent open Form buffer and cancel semantics', async () => {
    const sourceA = [variable('SourceA', 'original-a')];
    const sourceB = [variable('SourceB', 'replacement-b')];
    const owner = mountVariableSource(sourceA);
    fireEvent.click(screen.getByRole('button', {
      name: i18nText('agentFlow', 'auto.edit', { value1: 'SourceA' })
    }));
    const dialog = await screen.findByRole('dialog');
    fireEvent.change(within(dialog).getByRole('textbox', {
      name: i18nText('agentFlow', 'auto.name')
    }), { target: { value: 'UnsavedForm' } });
    fireEvent.change(within(dialog).getByPlaceholderText(
      i18nText('agentFlow', 'auto.enter_variable_value')
    ), { target: { value: 'independent-form-buffer' } });

    owner.replace(sourceB, 'replace-with-open-form');

    expect(owner.firstRows('replace-with-open-form')).toEqual([
      { name: 'env.SourceB', value: 'replacement-b' }
    ]);
    expect(screen.getByRole('dialog')).toBe(dialog);
    expect(within(dialog).getByRole('textbox', {
      name: i18nText('agentFlow', 'auto.name')
    })).toHaveValue('UnsavedForm');
    expect(within(dialog).getByPlaceholderText(
      i18nText('agentFlow', 'auto.enter_variable_value')
    )).toHaveValue('independent-form-buffer');
    fireEvent.click(within(dialog).getByRole('button', {
      name: textButtonName('auto.cancel')
    }));
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(owner.onSave).not.toHaveBeenCalled();
    expect(visibleVariableRows()).toEqual([
      { name: 'env.SourceB', value: 'replacement-b' }
    ]);
  });

  test('discards validation from an older source before it can save or change the newer list and open Modal', async () => {
    const sourceA = [variable('SourceA', 'original-a')];
    const sourceB = [variable('SourceB', 'replacement-b')];
    const owner = mountVariableSource(sourceA);
    const dialog = await fillNewVariable('OldSubmission', 'submitted-from-a');
    clickSave(dialog);
    // submitModal awaits the real Form validation before its local write/onSave.
    // Do not invent an asynchronous callback contract: onSave returns void.
    expect(owner.onSave).not.toHaveBeenCalled();
    owner.replace(sourceB, 'replace-during-validation');
    expect(visibleVariableRows()).toEqual([
      { name: 'env.SourceB', value: 'replacement-b' }
    ]);

    // Drain the real Form/async-validator Promise chain without waiting for a
    // stale save callback or replacing validation with a synthetic deferred.
    await act(async () => { await Promise.resolve(); });
    // A failure displays the actual outgoing payload in mock.calls, including
    // whether it captured A or silently retargeted B. Neither write is allowed.
    expect(owner.onSave.mock.calls).toEqual([]);
    expect(visibleVariableRows()).toEqual([
      { name: 'env.SourceB', value: 'replacement-b' }
    ]);
    expect(screen.getByRole('dialog')).toBe(dialog);
    expect(within(dialog).getByRole('textbox', {
      name: i18nText('agentFlow', 'auto.name')
    })).toHaveValue('OldSubmission');
    expect(within(dialog).getByPlaceholderText(
      i18nText('agentFlow', 'auto.enter_variable_value')
    )).toHaveValue('submitted-from-a');
  });
});
