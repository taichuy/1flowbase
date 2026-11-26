import { StrictMode } from 'react';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, expect, test } from 'vitest';
import {
  navigatePublicRoute,
  usePublicLocation
} from '../anonymous-navigation';

function Location() {
  const location = usePublicLocation();
  return (
    <>
      <output>{location}</output>
      <button
        onClick={() =>
          navigatePublicRoute({
            to: '/sign-in',
            search: { login_entry_id: 'password' }
          })
        }
      >
        Choose
      </button>
    </>
  );
}
beforeEach(() =>
  window.history.replaceState(
    {},
    '',
    '/sign-in?language=zh&auth_error=cancelled'
  )
);
test('native selection preserves language/error and removes only the selected entry', () => {
  render(
    <StrictMode>
      <Location />
    </StrictMode>
  );
  fireEvent.click(screen.getByText('Choose'));
  expect(screen.getByRole('status')).toHaveTextContent(
    '/sign-in?language=zh&auth_error=cancelled&login_entry_id=password'
  );
  act(() =>
    navigatePublicRoute({
      to: '/sign-in',
      search: { login_entry_id: undefined },
      replace: true
    })
  );
  expect(location.search).toBe('?language=zh&auth_error=cancelled');
});
test('popstate follows Back/Forward snapshots including MCP request and origin', () => {
  const view = render(
    <StrictMode>
      <Location />
    </StrictMode>
  );
  act(() => {
    history.replaceState(
      {},
      '',
      '/mcp/authorize?request_id=req&origin=https%3A%2F%2Fclient.example'
    );
    dispatchEvent(new PopStateEvent('popstate'));
  });
  expect(screen.getByRole('status')).toHaveTextContent(
    '/mcp/authorize?request_id=req&origin=https%3A%2F%2Fclient.example'
  );
  act(() => {
    history.replaceState({}, '', '/sign-in?login_entry_id=other');
    dispatchEvent(new PopStateEvent('popstate'));
  });
  expect(screen.getByRole('status')).toHaveTextContent(
    '/sign-in?login_entry_id=other'
  );
  view.unmount();
});
test('authentication navigation clears public query credentials', () => {
  navigatePublicRoute({ to: '/' });
  expect(location.pathname + location.search).toBe('/');
});
