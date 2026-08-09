import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

import { ApplicationEnvironmentVariablesPanel } from '../../components/editor/ApplicationEnvironmentVariablesPanel';
import { EnvironmentVariableValueEditor } from '../../components/editor/environment-variables/EnvironmentVariableValueEditor';

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
