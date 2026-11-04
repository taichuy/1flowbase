import { resetAuthStore, useAuthStore } from '../../state/auth-store';

export type PreferenceActor = { role: string; permissions: string[] };

export const preferenceWriteCases: Array<{
  name: string;
  actor: PreferenceActor | null;
  canPersist: boolean;
}> = [
  { name: 'missing identity', actor: null, canPersist: false },
  { name: 'reader', actor: { role: 'member', permissions: [] }, canPersist: false },
  { name: 'profile only', actor: { role: 'member', permissions: ['patch_me'] }, canPersist: false },
  { name: 'assistant only', actor: { role: 'member', permissions: ['assistant_update_settings'] }, canPersist: false },
  { name: 'metadata only', actor: { role: 'member', permissions: ['patch_me_meta'] }, canPersist: true },
  { name: 'root', actor: { role: 'root', permissions: [] }, canPersist: true }
];

export function setPreferenceActor(actor: PreferenceActor | null) {
  resetAuthStore();
  if (!actor) return;
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'preference-csrf',
    actor: {
      id: 'preference-user', account: actor.role,
      effective_display_role: actor.role, current_workspace_id: 'workspace-1'
    },
    me: {
      id: 'preference-user', account: actor.role, email: 'preferences@example.com',
      phone: null, nickname: 'Preferences', name: 'Preferences', avatar_url: null,
      introduction: '', effective_display_role: actor.role, permissions: actor.permissions
    }
  });
}
