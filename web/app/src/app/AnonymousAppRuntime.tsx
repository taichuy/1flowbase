import { PublicRouterProvider } from './public-router';
import { PublicAuthProviders } from '../features/auth/components/PublicAuthProviders';

export function AnonymousAppRuntime() {
  return (
    <PublicAuthProviders>
      <PublicRouterProvider />
    </PublicAuthProviders>
  );
}
