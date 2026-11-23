import type { MenuProps } from 'antd';
import type { ConsoleMe, ConsoleSessionActor } from '@1flowbase/api-client';

import { signOut } from '../features/auth/api/session-actions';
import { i18nText } from '../shared/i18n/text';

interface AccountMenuClickHandlerOptions {
  csrfToken: string | null;
  setAnonymous: () => void;
  navigateTo: (path: '/me' | '/sign-in') => Promise<void> | void;
}

interface AccountLabelSnapshot {
  me: Pick<ConsoleMe, 'name' | 'nickname'> | null;
  actor: Pick<ConsoleSessionActor, 'account'> | null;
}

export function createAccountMenuClickHandler({
  csrfToken,
  setAnonymous,
  navigateTo
}: AccountMenuClickHandlerOptions): MenuProps['onClick'] {
  return ({ key }) => {
    if (key === 'profile') {
      void navigateTo('/me');
      return;
    }

    if (key === 'sign-out') {
      void (async () => {
        try {
          if (csrfToken) {
            await signOut(csrfToken);
          }
        } finally {
          setAnonymous();
          await navigateTo('/sign-in');
        }
      })();
    }
  };
}

export function selectAccountLabel({ me, actor }: AccountLabelSnapshot) {
  return me?.nickname || me?.name || actor?.account || i18nText("appShell", "auto.user");
}
