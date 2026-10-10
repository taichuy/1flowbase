import {
  requestManagedService,
  type ManagedServiceRequest
} from '@1flowbase/api-client';
import { useAuthStore } from '../../../../../state/auth-store';

/** Read the current session at call time; never freeze credentials into compiled TSX. */
export function request<T>(options: ManagedServiceRequest): Promise<T> {
  return requestManagedService<T>(options, useAuthStore.getState().csrfToken);
}
