import { useAuthStore } from '../../../../state/auth-store';

/** Authoring scenarios state their grants explicitly; permission scenarios override them. */
export function authenticateEditor(
  permissions: string[] = [],
  role: 'root' | 'member' = 'root'
) {
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-token',
    actor: {
      id: 'user-1',
      account: role,
      effective_display_role: role,
      current_workspace_id: 'workspace-1'
    },
    me: {
      id: 'user-1',
      account: role,
      email: 'member@example.com',
      phone: null,
      nickname: role,
      name: role,
      avatar_url: null,
      introduction: '',
      effective_display_role: role,
      permissions
    }
  });
}
