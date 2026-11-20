import { lazy, Suspense, useEffect } from 'react';
import { SignInPage } from '../features/auth/pages/SignInPage';
import { AnonymousLoadingState } from '../features/auth/components/anonymous-loading/AnonymousLoadingState';
import { discardDocumentScene } from '../features/auth/components/hero/document-motion';
import {
  navigatePublicRoute,
  usePublicLocation
} from '../features/auth/lib/anonymous-navigation';

const McpAuthRuntime = lazy(() =>
  import('../features/auth/components/public-runtime/McpAuthRuntime').then(
    (module) => ({ default: module.McpAuthRuntime })
  )
);

export function PublicRouterProvider() {
  const location = usePublicLocation();
  const url = new URL(location, window.location.origin);
  const signIn = /^\/sign-in\/?$/.test(url.pathname);
  const mcp = /^\/mcp\/authorize\/?$/.test(url.pathname);
  const loginEntryId = url.searchParams.get('login_entry_id') ?? undefined;
  const requestId = url.searchParams.get('request_id') ?? undefined;
  useEffect(() => {
    if (mcp) discardDocumentScene();
    else if (!signIn) navigatePublicRoute({ to: '/sign-in', replace: true });
  }, [mcp, signIn]);
  if (mcp)
    return (
      <Suspense fallback={<AnonymousLoadingState fullscreen />}>
        <McpAuthRuntime requestId={requestId} />
      </Suspense>
    );
  if (!signIn) return <AnonymousLoadingState fullscreen />;
  return (
    <SignInPage
      loginEntryId={loginEntryId?.trim() ? loginEntryId : undefined}
    />
  );
}
