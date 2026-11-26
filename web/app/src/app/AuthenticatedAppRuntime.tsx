import type { QueryClient } from '@tanstack/react-query';
import { AppProviders } from './AppProviders';
import { AppRouterProvider } from './router';
import { WebMcpRegistrationLifecycle } from '../features/webmcp/components/WebMcpRegistrationLifecycle';

export function AuthenticatedAppRuntime({
  queryClient
}: {
  queryClient: QueryClient;
}) {
  return (
    <AppProviders queryClient={queryClient}>
      <WebMcpRegistrationLifecycle />
      <AppRouterProvider />
    </AppProviders>
  );
}
