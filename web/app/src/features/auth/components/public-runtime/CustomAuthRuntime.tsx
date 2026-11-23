import { diagnoseLegacyBlockModuleSource } from '@1flowbase/page-runtime/source-contract';
import { BuiltinPasswordSignIn } from '../BuiltinPasswordSignIn';
import { Suspense } from 'react';
import { AnonymousLoadingState } from '../anonymous-loading/AnonymousLoadingState';
import type { PublicAuthBlockProps } from '../PublicAuthBlock';
import { PublicAuthBlock } from '../PublicAuthBlock';
import { PublicAuthProviders } from '../PublicAuthProviders';
import { PublicAuthRouter } from './PublicAuthRouter';

export function CustomAuthRuntime(props: PublicAuthBlockProps) {
  if (diagnoseLegacyBlockModuleSource(props.instance.public_ui_block)) {
    return (
      <BuiltinPasswordSignIn
        loginEntryId={props.instance.id}
        loginEntrySelector={props.loginEntrySelector}
        onAuthenticated={props.onAuthenticated}
        invitationToken={props.invitationToken}
      />
    );
  }
  if (!props.nativeModuleRegistryFactory) {
    void import('../../../frontstage/lib/native-modules/registry').catch(
      () => undefined
    );
  }
  return (
    <PublicAuthProviders>
      <PublicAuthRouter>
        <Suspense fallback={<AnonymousLoadingState compact />}>
          <PublicAuthBlock {...props} />
        </Suspense>
      </PublicAuthRouter>
    </PublicAuthProviders>
  );
}
