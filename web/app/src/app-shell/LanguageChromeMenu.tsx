import TranslationOutlined from '@ant-design/icons/es/icons/TranslationOutlined';
import type { MenuProps } from 'antd';
import { Menu } from 'antd';
import { useTranslation } from 'react-i18next';

import { useAuthStore } from '../state/auth-store';
import { patchUserPreferences } from '../shared/user-preferences/user-preferences';
import {
  mergeLocalePreferenceMeta,
  writeLocalePreferenceToStorage,
  type ProfileLocalePreference
} from '../shared/user-preferences/locale-preference';

const PROFILE_LOCALE_BY_MENU_KEY = {
  zh_Hans: 'zh_Hans',
  en_US: 'en_US'
} as const;

export function LanguageChromeMenu() {
  const { t } = useTranslation('appShell');
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const me = useAuthStore((state) => state.me);
  const setMe = useAuthStore((state) => state.setMe);

  const handleClick: MenuProps['onClick'] = ({ key }) => {
    if (!(key in PROFILE_LOCALE_BY_MENU_KEY)) {
      return;
    }

    const nextPreferredLocale =
      PROFILE_LOCALE_BY_MENU_KEY[
        key as keyof typeof PROFILE_LOCALE_BY_MENU_KEY
      ];

    writeLocalePreferenceToStorage(nextPreferredLocale);

    if (!me) {
      return;
    }

    const optimisticMeta = mergeLocalePreferenceMeta(
      me.meta,
      nextPreferredLocale
    );

    setMe({
      ...me,
      preferred_locale: nextPreferredLocale,
      meta: optimisticMeta
    });

    if (!csrfToken) {
      return;
    }

    void patchUserPreferences(optimisticMeta, csrfToken)
      .then((updatedMe) => {
        setMe({
          ...updatedMe,
          preferred_locale: nextPreferredLocale as ProfileLocalePreference,
          meta: mergeLocalePreferenceMeta(updatedMe.meta, nextPreferredLocale)
        });
      })
      .catch(() => {
        // Locale preference persistence should not block the current UI choice.
      });
  };

  return (
    <Menu
      className="app-shell-language-menu"
      mode="horizontal"
      selectable={false}
      onClick={handleClick}
      items={[
        {
          key: 'language',
          label: (
            <span
              className="app-shell-language-block"
              aria-label={t('language.trigger')}
            >
              <TranslationOutlined />
              <span className="app-shell-language-label">
                {t('language.current')}
              </span>
            </span>
          ),
          popupClassName: 'app-shell-language-popup',
          children: [
            {
              key: 'zh_Hans',
              label: t('language.zh_hans')
            },
            {
              key: 'en_US',
              label: t('language.en_us')
            }
          ]
        }
      ]}
      disabledOverflow
    />
  );
}
