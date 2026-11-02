import { fireEvent, render, screen } from '@testing-library/react';
import { beforeAll, describe, expect, test, vi } from 'vitest';

vi.mock('../BlockSourceEditor', () => ({
  BlockSourceEditor: ({
    value,
    readOnly,
    onChange
  }: {
    value: string;
    readOnly: boolean;
    onChange(value: string): void;
  }) => (
    <textarea
      aria-label="studio source"
      value={value}
      readOnly={readOnly}
      onChange={(event) => onChange(event.target.value)}
    />
  )
}));

import { loadApplicationI18nResources } from '../../i18n/app-i18n';
import { i18nText } from '../../i18n/text';
import { BlockSourceStudio } from '../BlockSourceStudio';

beforeAll(() => loadApplicationI18nResources());

describe('BlockSourceStudio permissions', () => {
  test.each([false, true])(
    'keeps source readable while independent execution authority is %s',
    async (canRun) => {
      const onRun = vi.fn();
      const onChange = vi.fn();
      const onReset = vi.fn();
      const onSave = vi.fn();
      render(
        <BlockSourceStudio
          contextComment="context"
          dirty
          initialSection="code"
          loading={false}
          open
          owner="permission-test"
          path="permission.tsx"
          readOnly
          canRun={canRun}
          saving={false}
          sections={['code', 'run']}
          source="readable source"
          testId="permission-studio"
          windowId="permission-studio"
          onChange={onChange}
          onClose={vi.fn()}
          onInjectContext={(source) => `${source} context`}
          onReset={onReset}
          onRun={onRun}
          onSave={onSave}
          renderResource={() => null}
        />
      );
      const editor = await screen.findByRole('textbox', {
        name: 'studio source'
      });
      expect(editor).toHaveValue('readable source');
      expect(editor).toHaveAttribute('readonly');
      for (const key of ['auto.inject_context', 'auto.reset', 'auto.save']) {
        const button = screen.getByRole('button', {
          name: new RegExp(`^${i18nText('frontstage', key).split('').join('\\s*')}$`)
        });
        expect(button).toBeDisabled();
        fireEvent.click(button);
      }
      const run = screen.getByRole('button', {
        name: new RegExp(`^${i18nText('frontstage', 'auto.run').split('').join('\\s*')}$`)
      });
      expect(run).toHaveProperty('disabled', !canRun);
      fireEvent.click(run);
      expect(onRun).toHaveBeenCalledTimes(canRun ? 1 : 0);
      expect(onChange).not.toHaveBeenCalled();
      expect(onReset).not.toHaveBeenCalled();
      expect(onSave).not.toHaveBeenCalled();
    }
  );
});
