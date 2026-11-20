import { fetchPublicSystemMetadata } from '@1flowbase/api-client/auth';
import { getInitialAppLocale } from '../../../../shared/i18n/initial-locale';
import { auto as enBrand, sign_in as enPage } from '../../i18n/en_US.json';
import { auto as zhBrand, sign_in as zhPage } from '../../i18n/zh_Hans.json';
import {
  findDocumentArtwork,
  setDocumentBrand,
  documentSceneId
} from './document-motion';
import { startLeafMotion } from './leaf-motion';
const art = findDocumentArtwork();
if (art) {
  art.stopMotion = startLeafMotion(
    art.querySelector('canvas')!,
    art.querySelector('.auth-sign-in-hero')!
  );

  const chinese = getInitialAppLocale() === 'zh_Hans';
  const slogan = chinese ? zhBrand.product_tagline : enBrand.product_tagline;
  const scene = document.getElementById(documentSceneId)!;
  scene.querySelector('.auth-sign-in-footer a')!.textContent = chinese
    ? zhPage.footer
    : enPage.footer;
  // The public response, not the core bundle or auth decision, owns this reveal.
  art.metadataFlight = fetchPublicSystemMetadata();
  void art.metadataFlight
    .then((metadata) => {
      art.metadata = metadata;
      if (!scene.isConnected) return;
      setDocumentBrand(
        art,
        metadata.hide_login_tagline === false ? slogan : null
      );
      if (metadata.filing_html) {
        const filing = scene.querySelector<HTMLElement>(
          '[data-opening-filing]'
        )!;
        filing.className = 'auth-sign-in-filing';
        filing.innerHTML = metadata.filing_html;
        filing.hidden = false;
      }
    })
    .catch(() => {
      delete art.metadataFlight;
    });
}
