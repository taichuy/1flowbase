import { apiFetch } from '@1flowbase/api-client/auth';
import {
  getAuthApiBaseUrl,
  type PasswordSignInResponse
} from '../../api/session';
import type { PublicAuthBlockProps } from '../PublicAuthBlock';

export type DefaultPasswordSignInProps = Pick<
  PublicAuthBlockProps,
  'instance' | 'loginEntrySelector' | 'onAuthenticated'
>;

/** Only the capabilities exercised by the unchanged official password source.
 * Keep request/session/event semantics aligned with public-auth-block-host and
 * PublicAuthBlock's interface handler; do not import their framework runtime.
 */
export function createDefaultPasswordContext(
  { instance, loginEntrySelector, onAuthenticated }: DefaultPasswordSignInProps,
  isCurrentInstance: () => boolean
) {
  const requireCurrentInstance = () => {
    if (!isCurrentInstance()) {
      throw new Error('Native Block capability belongs to a stale instance.');
    }
  };
  return {
    inputs: {
      login_entry_id: instance.id,
      authenticator_selection_available: Boolean(loginEntrySelector),
      public_variables: instance.public_variables
    },
    api: {
      async post<TResponse = unknown>(
        path: string,
        request?: { body?: unknown }
      ): Promise<TResponse> {
        requireCurrentInstance();
        const url = new URL(path, 'http://public-auth.local');
        if (
          url.origin !== 'http://public-auth.local' ||
          !url.pathname.startsWith('/api/public/')
        ) {
          throw new Error(
            'Public authentication Block requested a forbidden API path.'
          );
        }
        const response = await apiFetch<TResponse>({
          method: 'POST',
          path: `${url.pathname}${url.search}`,
          body: request?.body,
          baseUrl: getAuthApiBaseUrl()
        });
        // The existing host awaits its captured callback INSIDE the interface
        // handler, before the capability's post-response stale check. Keep that
        // ordering (including navigation/unmount during callback) unchanged.
        if (
          (path === '/api/public/auth/sign-in' ||
            path === '/api/public/auth/sign-up') &&
          isPublicAuthSession(response)
        ) {
          await onAuthenticated(response);
        }
        requireCurrentInstance();
        return response;
      }
    },
    events: {
      emit(name: string) {
        requireCurrentInstance();
        if (name === 'authenticator_selector_requested')
          loginEntrySelector?.request();
      }
    }
  };
}

export type DefaultPasswordContext = ReturnType<
  typeof createDefaultPasswordContext
>;

// Same canonical response contract as PublicAuthBlock's completion handler.
function isPublicAuthSession(value: unknown): value is PasswordSignInResponse {
  return (
    typeof value === 'object' &&
    value !== null &&
    !Array.isArray(value) &&
    'csrf_token' in value &&
    typeof value.csrf_token === 'string' &&
    'effective_display_role' in value &&
    typeof value.effective_display_role === 'string' &&
    'current_workspace_id' in value &&
    typeof value.current_workspace_id === 'string'
  );
}
