import { lazy, Suspense, useLayoutEffect, useRef, useState } from 'react';
import { useAuthTranslation } from '../lib/auth-i18n';
import {
  findDocumentArtwork,
  discardDocumentScene,
  setDocumentBrand
} from './hero/document-motion';
const SignInFallbackHero = lazy(() => import('./hero/SignInFallbackHero'));

export function HeroAnimation({
  hide_login_tagline,
  login_hero_title,
  login_hero_tagline
}: {
  hide_login_tagline?: boolean;
  login_hero_title?: string;
  login_hero_tagline?: string | null;
}) {
  const { t } = useAuthTranslation();
  const host = useRef<HTMLDivElement>(null);
  const [opening] = useState(findDocumentArtwork);
  const title = login_hero_title?.trim() || '1flowbase';
  const slogan = hide_login_tagline
    ? null
    : (login_hero_tagline?.trim() || t('auto.product_tagline'));
  useLayoutEffect(() => {
    if (opening) setDocumentBrand(opening, slogan, title);
  }, [opening, slogan, title]);
  useLayoutEffect(() => {
    if (opening) {
      const target = host.current!;
      // Move the live canvas, its pointer target and its simulation together. Do not restart on handoff.
      target.appendChild(opening);
      discardDocumentScene();
      return () => {
        queueMicrotask(() => {
          if (!opening.isConnected) {
            opening.stopMotion?.();
            delete opening.stopMotion;
          }
        });
      };
    }
  }, [opening]);
  // An intact document opening never requests the fallback graph. Recovery also
  // goes through Suspense: an abandoned mount cannot start a late simulation.
  const fallback = (!opening || !opening.stopMotion) && (
    <Suspense fallback={null}>
      <SignInFallbackHero opening={opening} title={title} slogan={slogan} />
    </Suspense>
  );
  return opening ? (
    <div ref={host} style={{ display: 'contents' }}>
      {fallback}
    </div>
  ) : (
    fallback
  );
}
