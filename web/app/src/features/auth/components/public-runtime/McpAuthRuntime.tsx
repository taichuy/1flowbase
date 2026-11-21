import { PublicAuthProviders } from '../PublicAuthProviders';
import { McpAuthorizePage } from '../../pages/mcp-authorization/McpAuthorizePage';
import { PublicAuthRouter } from './PublicAuthRouter';

export function McpAuthRuntime({ requestId }: { requestId?: string }) {
  return (
    <PublicAuthProviders>
      <PublicAuthRouter>
        <McpAuthorizePage requestId={requestId} />
      </PublicAuthRouter>
    </PublicAuthProviders>
  );
}
