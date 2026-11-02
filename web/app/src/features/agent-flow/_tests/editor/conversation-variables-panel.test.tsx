import { authenticateEditor } from '../fixtures/editor-auth';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { ConversationVariablesPanel } from '../../components/editor/ConversationVariablesPanel';
import { appI18n } from '../../../../shared/i18n/app-i18n';

describe('ConversationVariablesPanel', () => {
  beforeEach(async () => {
    authenticateEditor();
    await appI18n.changeLanguage('zh_Hans');
  });

  test('adds a conversation variable definition without an initial value', async () => {
    const onSave = vi.fn();

    render(
      <ConversationVariablesPanel
        variables={[]}
        onClose={vi.fn()}
        onSave={onSave}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: /添加会话变量/ }));
    fireEvent.change(screen.getByPlaceholderText('ApiBaseUrl'), {
      target: { value: 'ApiBaseUrl' }
    });
    fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));

    await waitFor(() => {
      expect(onSave).toHaveBeenCalledWith([
        {
          name: 'ApiBaseUrl',
          valueType: 'string',
          description: ''
        }
      ]);
    });
  });
});
