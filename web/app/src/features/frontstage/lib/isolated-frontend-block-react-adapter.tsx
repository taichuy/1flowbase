import { useEffect, useRef, type ReactNode } from 'react';

import {
  prepareIsolatedFrontendBlockRealm,
  type IsolatedFrontendBlockCapabilityHandlers,
  type IsolatedFrontendBlockRealmHandle
} from '@1flowbase/page-runtime';

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
  const realmRef = useRef<IsolatedFrontendBlockRealmHandle | null>(null);
  const propsRef = useRef(preparation.program.props);
  propsRef.current = preparation.program.props;

  useEffect(() => {
    let active = true;
    const mountedProps = preparation.program.props;
    const realm = prepareIsolatedFrontendBlockRealm(preparation.program, {
      capabilityHandlers,
      onError: onRuntimeError
    });
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
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    root,
    capabilityHandlers,
    preparation.contributionId,
    preparation.program.source
  ]);

  useEffect(() => {
    const realm = realmRef.current;
    if (realm?.state === 'mounted' || realm?.state === 'updated') {
      realm.update(preparation.program.props);
    }
  }, [preparation.program.props]);

  return null;
}
