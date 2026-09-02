import { AppProviders } from './AppProviders';
import { AppRouterProvider } from './router';

export function AuthenticatedAppRuntime() {
  return (
    <AppProviders>
      <AppRouterProvider />
    </AppProviders>
  );
}
