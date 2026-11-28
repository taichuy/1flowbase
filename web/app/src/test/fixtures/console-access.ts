import { useAuthStore } from '../../state/auth-store';
import {
  resetConsoleOperationAccessCache,
  seedConsoleOperationAccessForTests
} from '../../state/console-operation-access';

export function seedAccessFixture(codes: readonly string[]) {
  const { actor, csrfToken, sessionStatus } = useAuthStore.getState();
  if (actor && csrfToken && sessionStatus !== 'authenticated') {
    useAuthStore.setState({ sessionStatus: 'authenticated' });
  }
  resetConsoleOperationAccessCache();
  seedConsoleOperationAccessForTests(
    Object.fromEntries(codes.map((code) => [code, true]))
  );
}
