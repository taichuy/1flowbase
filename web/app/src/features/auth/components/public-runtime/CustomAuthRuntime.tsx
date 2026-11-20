import type { PublicAuthBlockProps } from '../PublicAuthBlock';
import { PublicAuthBlock } from '../PublicAuthBlock';
import { PublicAuthProviders } from '../PublicAuthProviders';
import { PublicAuthRouter } from './PublicAuthRouter';

export function CustomAuthRuntime(props: PublicAuthBlockProps) {
  return (
    <PublicAuthProviders>
      <PublicAuthRouter>
        <PublicAuthBlock {...props} />
      </PublicAuthRouter>
    </PublicAuthProviders>
  );
}
