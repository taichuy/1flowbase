import { useEffect, useLayoutEffect, useRef, type ReactNode } from 'react';

import {
  prepareIsolatedFrontendBlockRealm,
  type IsolatedFrontendBlockCapabilityHandlers,
  type IsolatedFrontendBlockRealmHandle
} from '@1flowbase/page-runtime/browser';

import type { PreparedFrontstageIsolatedContribution } from './isolated-frontend-block-contribution';

export interface FrontstageIsolatedFrontendBlockHostProps {
  root: Element;
  preparation: PreparedFrontstageIsolatedContribution;
  capabilityHandlers?: IsolatedFrontendBlockCapabilityHandlers;
  onRuntimeError?(error: Error): void;
}

export function FrontstageIsolatedFrontendBlockHost({
  root,
  preparation,
  capabilityHandlers,
  onRuntimeError
}: FrontstageIsolatedFrontendBlockHostProps): ReactNode {
  const {
    contributionId,
    program: { source, props }
  } = preparation;
  const realmRef = useRef<IsolatedFrontendBlockRealmHandle | null>(null);
  const propsRef = useRef(props);
  const runtimeErrorRef = useRef(onRuntimeError);

  useLayoutEffect(() => {
    propsRef.current = props;
    runtimeErrorRef.current = onRuntimeError;
  }, [props, onRuntimeError]);

  useEffect(() => {
    let active = true;
    const mountedProps = propsRef.current;
    const realm = prepareIsolatedFrontendBlockRealm(
      { source, props: mountedProps },
      {
        capabilityHandlers,
        onError: (error) => runtimeErrorRef.current?.(error)
      }
    );
    realmRef.current = realm;
    void realm.mount(root).then(
      () => {
        if (!active || propsRef.current === mountedProps) return;
        realm.update(propsRef.current);
      },
      () => undefined
    );
    return () => {
      active = false;
      realmRef.current = null;
      realm.terminate();
    };
    // Contribution identity/source and host authority own a realm; props update over its port.
  }, [root, capabilityHandlers, contributionId, source]);

  useEffect(() => {
    const realm = realmRef.current;
    if (realm?.state === 'mounted' || realm?.state === 'updated') {
      realm.update(props);
    }
  }, [props]);

  return null;
}
