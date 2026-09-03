import { AppProviders } from './AppProviders';
import { AppRouterProvider } from './router';
import { WebMcpRegistrationLifecycle } from '../features/webmcp/components/WebMcpRegistrationLifecycle';

export function AuthenticatedAppRuntime() {
  return (
    <AppProviders>
      <WebMcpRegistrationLifecycle />
      <AppRouterProvider />
    </AppProviders>
  );
}
