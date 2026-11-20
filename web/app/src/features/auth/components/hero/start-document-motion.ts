import {
  fetchPublicSystemMetadata,
  type SystemMetadata
} from '@1flowbase/api-client/auth';
import { getInitialAppLocale } from '../../../../shared/i18n/initial-locale';
import { auto as enBrand, sign_in as enPage } from '../../i18n/en_US.json';
import { auto as zhBrand, sign_in as zhPage } from '../../i18n/zh_Hans.json';
import {
  findDocumentArtwork,
  setDocumentBrand,
  documentSceneId
} from './document-motion';
import { startLeafMotion } from './leaf-motion';
// Same public API envelope as the normal request. Missing SSI, a timed-out
// subrequest or an invalid response all take the normal API-client path.
function includedMetadata(): SystemMetadata | undefined {
  const template =
    document.querySelector<HTMLTemplateElement>('#sign-in-metadata');
  const text = template?.content.textContent;
  template?.remove();
  if (!text) return;
  try {
    const data = JSON.parse(text)?.data;
    if (
      typeof data?.hide_login_tagline === 'boolean' &&
      typeof data.filing_html === 'string' &&
      typeof data.logo_url === 'string' &&
      typeof data.system_description === 'string' &&
      typeof data.system_name === 'string'
    )
      return data;
  } catch {
    /* e.g. failed upstream; never guess the public configuration */
  }
}
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
  const included = includedMetadata();
  art.metadataFlight = included
    ? Promise.resolve(included)
    : fetchPublicSystemMetadata();
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
