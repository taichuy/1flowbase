import 'vditor/dist/js/lute/lute.min.js';
import 'vditor/dist/js/i18n/zh_CN.js';
import vditorAntIconsSource from 'vditor/dist/js/icons/ant.js?raw';

const VDITOR_RUNTIME_MARKERS = [
  'vditorLuteScript',
  'vditorIconScript'
] as const;

let runtimeConsumers = 0;
const iconSpriteConsumers = new WeakMap<
  Document | ShadowRoot,
  { count: number; ownedSprite: SVGSVGElement | null }
>();

const ICON_SOURCE_PREFIX = "document.body.insertAdjacentHTML('afterbegin', `";
const ICON_SOURCE_SUFFIX = '`)';
const VDITOR_ICON_SPRITE = extractVditorIconSprite(vditorAntIconsSource);

export function acquireBundledVditorRuntime(
  root: Document | ShadowRoot
): () => void {
  runtimeConsumers += 1;
  acquireIconSprite(root);
  for (const id of VDITOR_RUNTIME_MARKERS) {
    if (document.getElementById(id)) continue;
    const marker = document.createElement('script');
    marker.id = id;
    marker.type = 'application/x-1flowbase-bundled-support';
    document.head.appendChild(marker);
  }
  let released = false;
  return () => {
    if (released) return;
    released = true;
    releaseIconSprite(root);
    runtimeConsumers = Math.max(0, runtimeConsumers - 1);
    if (runtimeConsumers === 0) {
      for (const id of VDITOR_RUNTIME_MARKERS) {
        document.getElementById(id)?.remove();
      }
    }
  };
}

function extractVditorIconSprite(source: string): string {
  const normalized = source.trim();
  if (
    !normalized.startsWith(ICON_SOURCE_PREFIX) ||
    !normalized.endsWith(ICON_SOURCE_SUFFIX)
  ) {
    throw new Error('The bundled Vditor Ant icon asset has an unknown format.');
  }
  return normalized.slice(ICON_SOURCE_PREFIX.length, -ICON_SOURCE_SUFFIX.length);
}

function acquireIconSprite(root: Document | ShadowRoot) {
  const current = iconSpriteConsumers.get(root);
  if (current) {
    current.count += 1;
    return;
  }

  const existingIcon = root.getElementById('vditor-icon-headings');
  if (existingIcon) {
    iconSpriteConsumers.set(root, { count: 1, ownedSprite: null });
    return;
  }

  const ownerDocument =
    root.nodeType === Node.DOCUMENT_NODE
      ? (root as Document)
      : (root as ShadowRoot).ownerDocument;
  const template = ownerDocument.createElement('template');
  template.innerHTML = VDITOR_ICON_SPRITE;
  const sprite = template.content.firstElementChild;
  if (!(sprite instanceof SVGSVGElement)) {
    throw new Error('The bundled Vditor Ant icon asset is not an SVG sprite.');
  }
  sprite.setAttribute('data-1flowbase-vditor-icons', '');
  if (root.nodeType === Node.DOCUMENT_NODE) {
    (root as Document).body.prepend(sprite);
  } else {
    (root as ShadowRoot).prepend(sprite);
  }
  iconSpriteConsumers.set(root, { count: 1, ownedSprite: sprite });
}

function releaseIconSprite(root: Document | ShadowRoot) {
  const current = iconSpriteConsumers.get(root);
  if (!current) return;
  current.count -= 1;
  if (current.count > 0) return;
  current.ownedSprite?.remove();
  iconSpriteConsumers.delete(root);
}
