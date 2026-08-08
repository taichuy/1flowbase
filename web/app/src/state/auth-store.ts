import { create } from 'zustand';

import type {
  ConsoleAvailableRole,
  ConsoleMe,
  ConsoleSessionActor
} from '@1flowbase/api-client';

export interface AuthSnapshot {
  csrfToken: string;
  actor: ConsoleSessionActor;
  me: ConsoleMe | null;
  availableRoles?: ConsoleAvailableRole[];
}

interface AuthState {
  sessionStatus: 'unknown' | 'authenticated' | 'anonymous';
  csrfToken: string | null;
  actor: ConsoleSessionActor | null;
  me: ConsoleMe | null;
  availableRoles: ConsoleAvailableRole[];
  setAuthenticated: (payload: AuthSnapshot) => void;
  setAnonymous: () => void;
  setMe: (me: ConsoleMe) => void;
}

const initialState = {
  sessionStatus: 'unknown' as const,
  csrfToken: null,
  actor: null,
  me: null,
  availableRoles: []
};

export const useAuthStore = create<AuthState>((set) => ({
  ...initialState,
  setAuthenticated: ({ csrfToken, actor, me, availableRoles }) =>
    set({
      sessionStatus: 'authenticated',
      csrfToken,
      actor,
      me,
      availableRoles: availableRoles ?? []
    }),
  setAnonymous: () =>
    set({
      sessionStatus: 'anonymous',
      csrfToken: null,
      actor: null,
      me: null,
      availableRoles: []
    }),
  setMe: (me) =>
    set((state) => ({
      me,
      sessionStatus: state.actor ? 'authenticated' : state.sessionStatus
    }))
}));

export function resetAuthStore() {
  useAuthStore.setState(initialState);
}
