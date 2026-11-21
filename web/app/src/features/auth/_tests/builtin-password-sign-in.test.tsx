import { StrictMode } from 'react';
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import { beforeEach, expect, test, vi } from 'vitest';
import { BuiltinPasswordSignIn } from '../components/BuiltinPasswordSignIn';
import { signInWithPassword } from '../api/session';

vi.mock('../api/session', () => ({ signInWithPassword: vi.fn() }));
beforeEach(() => {
  vi.mocked(signInWithPassword).mockReset();
  localStorage.clear();
  history.replaceState({}, '', '/sign-in?language=en');
});
const session = {
  csrf_token: 'csrf',
  effective_display_role: 'member',
  current_workspace_id: 'workspace'
};
function submit() {
  fireEvent.change(screen.getByLabelText('Account or email'), {
    target: { value: 'account' }
  });
  fireEvent.change(screen.getByLabelText('Password'), {
    target: { value: 'secret' }
  });
  fireEvent.submit(
    screen.getByRole('form', { name: 'Escape password sign in' })
  );
}
test('fallback preserves API payload, awaits auth callback, and prevents duplicate pending submissions', async () => {
  let finish!: (value: typeof session) => void;
  vi.mocked(signInWithPassword).mockReturnValue(
    new Promise((resolve) => {
      finish = resolve;
    })
  );
  const authenticated = vi.fn();
  render(
    <StrictMode>
      <BuiltinPasswordSignIn
        loginEntryId="legacy"
        onAuthenticated={authenticated}
      />
    </StrictMode>
  );
  submit();
  submit();
  expect(screen.getByRole('button', { name: 'Sign in' })).toBeDisabled();
  expect(signInWithPassword).toHaveBeenCalledExactlyOnceWith({
    login_entry_id: 'legacy',
    identifier: 'account',
    password: 'secret'
  });
  await act(async () => finish(session));
  expect(authenticated).toHaveBeenCalledExactlyOnceWith(session);
  expect(screen.getByRole('button', { name: 'Sign in' })).toBeEnabled();
});
test('failed fallback exposes translated safe feedback and permits retry', async () => {
  vi.mocked(signInWithPassword)
    .mockRejectedValueOnce(new Error('internal details'))
    .mockResolvedValueOnce(session);
  const authenticated = vi.fn();
  render(<BuiltinPasswordSignIn onAuthenticated={authenticated} />);
  submit();
  expect(await screen.findByRole('alert')).toHaveTextContent(
    'Authentication failed.'
  );
  expect(document.body.innerHTML).not.toContain('internal details');
  submit();
  await waitFor(() => expect(authenticated).toHaveBeenCalledWith(session));
  expect(screen.queryByRole('alert')).not.toBeInTheDocument();
});
test('unmounted fallback ignores the late response and never calls its old auth owner', async () => {
  let finish!: (value: typeof session) => void;
  vi.mocked(signInWithPassword).mockReturnValue(
    new Promise((resolve) => {
      finish = resolve;
    })
  );
  const authenticated = vi.fn();
  const view = render(
    <BuiltinPasswordSignIn onAuthenticated={authenticated} />
  );
  submit();
  view.unmount();
  await act(async () => finish(session));
  expect(authenticated).not.toHaveBeenCalled();
});
