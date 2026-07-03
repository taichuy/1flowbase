import '../features/workflow/register';

import { AppProviders } from './AppProviders';
import { AppRouterProvider } from './router';
import { AuthBootstrap } from '../features/auth/components/AuthBootstrap';

export function App() {
  return (
    <AppProviders>
      <AuthBootstrap>
        <AppRouterProvider />
      </AuthBootstrap>
    </AppProviders>
  );
}
