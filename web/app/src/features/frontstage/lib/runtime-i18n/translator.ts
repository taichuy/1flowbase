import i18next from 'i18next';
import type {
  BlockContextI18n,
  BlockTranslationOptions
} from '@1flowbase/page-protocol';

export function createBlockI18n({
  locale,
  status,
  messages = {}
}: {
  locale: string;
  status: BlockContextI18n['status'];
  messages?: Readonly<Record<string, string>>;
}): BlockContextI18n {
  const instance = i18next.createInstance();
  void instance.init({
    lng: locale,
    resources: { [locale]: { translation: messages } },
    initAsync: false,
    fallbackLng: false,
    keySeparator: false,
    nsSeparator: false,
    interpolation: { escapeValue: false }
  });
  return Object.freeze({
    locale,
    status,
    t(key: string, options?: BlockTranslationOptions) {
      return instance.t(key, {
        replace: options?.values,
        defaultValue: options?.defaultValue ?? key
      });
    }
  });
}
