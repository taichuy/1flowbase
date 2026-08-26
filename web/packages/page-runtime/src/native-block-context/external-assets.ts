import type {
  BlockContextAssets,
  BlockExternalAssetHandle
} from '@1flowbase/page-protocol';

export interface NativeBlockExternalAssetScope {
  readonly assets: BlockContextAssets;
  dispose(): void;
}

export interface CreateNativeBlockExternalAssetScopeOptions {
  root: ShadowRoot;
  fetchText?(url: string): Promise<string>;
  importModule?(url: string): Promise<unknown>;
}

export function createNativeBlockExternalAssetScope({
  root,
  fetchText = fetchExternalText,
  importModule = importExternalModule
}: CreateNativeBlockExternalAssetScopeOptions): NativeBlockExternalAssetScope {
  const handles = new Set<BlockExternalAssetHandle>();
  const pendingElementLoads = new Set<() => void>();
  const moduleFlights = new Map<string, Promise<unknown>>();
  let disposed = false;

  const register = (element: Element): BlockExternalAssetHandle => {
    requireActive();
    let handleDisposed = false;
    const handle: BlockExternalAssetHandle = {
      dispose() {
        if (handleDisposed) return;
        handleDisposed = true;
        handles.delete(handle);
        element.remove();
      }
    };
    handles.add(handle);
    return handle;
  };

  const loadElement = <TElement extends HTMLElement>(
    element: TElement
  ): Promise<BlockExternalAssetHandle> => {
    requireActive();
    return new Promise((resolve, reject) => {
      let settled = false;
      const cleanupListeners = () => {
        element.removeEventListener('load', loaded);
        element.removeEventListener('error', failed);
        pendingElementLoads.delete(cancel);
      };
      const loaded = () => {
        if (settled) return;
        settled = true;
        cleanupListeners();
        try {
          resolve(register(element));
        } catch (error) {
          element.remove();
          reject(error);
        }
      };
      const failed = () => {
        if (settled) return;
        settled = true;
        cleanupListeners();
        element.remove();
        reject(
          new Error(
            `Native Block external asset failed to load: ${element.getAttribute('src') ?? element.getAttribute('href') ?? 'unknown'}.`
          )
        );
      };
      const cancel = () => {
        if (settled) return;
        settled = true;
        cleanupListeners();
        element.remove();
        reject(new Error('Native Block external asset scope is disposed.'));
      };
      element.addEventListener('load', loaded, { once: true });
      element.addEventListener('error', failed, { once: true });
      pendingElementLoads.add(cancel);
      root.append(element);
    });
  };

  const requireActive = () => {
    if (disposed) {
      throw new Error('Native Block external asset scope is disposed.');
    }
  };

  const assets: BlockContextAssets = {
    async importModule<TModule>(urlValue: string): Promise<TModule> {
      requireActive();
      const url = normalizeExternalUrl(root, urlValue);
      let flight = moduleFlights.get(url);
      if (!flight) {
        flight = importModule(url).catch((error) => {
          moduleFlights.delete(url);
          throw error;
        });
        moduleFlights.set(url, flight);
      }
      return (await flight) as TModule;
    },

    loadStyle(urlValue) {
      const url = normalizeExternalUrl(root, urlValue);
      const link = root.ownerDocument.createElement('link');
      link.rel = 'stylesheet';
      link.href = url;
      link.dataset.flowbaseExternalAsset = 'style';
      return loadElement(link);
    },

    loadScript(urlValue) {
      const url = normalizeExternalUrl(root, urlValue);
      const script = root.ownerDocument.createElement('script');
      script.src = url;
      script.async = true;
      script.dataset.flowbaseExternalAsset = 'script';
      return loadElement(script);
    },

    async loadSvgSprite(urlValue) {
      requireActive();
      const url = normalizeExternalUrl(root, urlValue);
      const source = await fetchText(url);
      requireActive();
      const sprite = createSvgSpriteElement(root.ownerDocument, source);
      sprite.dataset.flowbaseExternalAsset = 'svg-sprite';
      root.prepend(sprite);
      return register(sprite);
    }
  };

  return {
    assets,
    dispose() {
      if (disposed) return;
      disposed = true;
      [...pendingElementLoads].forEach((cancel) => cancel());
      [...handles].forEach((handle) => handle.dispose());
      moduleFlights.clear();
    }
  };
}

async function fetchExternalText(url: string): Promise<string> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(
      `Native Block external asset request failed: ${response.status} ${url}.`
    );
  }
  return response.text();
}

function importExternalModule(url: string): Promise<unknown> {
  return import(/* @vite-ignore */ url);
}

function normalizeExternalUrl(root: ShadowRoot, value: string): string {
  if (typeof value !== 'string' || value.trim() === '') {
    throw new Error('Native Block external asset URL must not be empty.');
  }
  const normalizedValue = value.startsWith('//') ? `https:${value}` : value;
  const url = new URL(normalizedValue, root.ownerDocument.baseURI);
  if (url.protocol !== 'https:') {
    throw new Error(
      `Native Block external asset URL must use HTTPS: ${url.href}.`
    );
  }
  return url.href;
}

function createSvgSpriteElement(
  ownerDocument: Document,
  source: string
): SVGSVGElement {
  const svgSource = source.trimStart().startsWith('<svg')
    ? source
    : extractIconfontSvg(source);
  const template = ownerDocument.createElement('template');
  template.innerHTML = svgSource.trim();
  const svg = template.content.querySelector('svg');
  if (!(svg instanceof ownerDocument.defaultView!.SVGSVGElement)) {
    throw new Error('Native Block SVG Sprite response does not contain an SVG root.');
  }
  svg.setAttribute('aria-hidden', 'true');
  Object.assign(svg.style, {
    position: 'absolute',
    width: '0',
    height: '0',
    overflow: 'hidden'
  });
  return svg;
}

function extractIconfontSvg(source: string): string {
  const assignment = 'var svgSprite=';
  const start = source.indexOf(assignment);
  const end = source.indexOf(';var script=', start + assignment.length);
  if (start < 0 || end < 0) {
    throw new Error(
      'Native Block SVG Sprite response is neither SVG nor a supported IconFont script.'
    );
  }
  return decodeStringConcatenation(
    source.slice(start + assignment.length, end)
  );
}

function decodeStringConcatenation(expression: string): string {
  const pieces: string[] = [];
  let index = 0;
  while (index < expression.length) {
    index = skipWhitespace(expression, index);
    const quote = expression[index];
    if (quote !== '"' && quote !== "'") {
      throw new Error('Native Block IconFont Sprite expression is invalid.');
    }
    const literalStart = index;
    index += 1;
    while (index < expression.length) {
      if (expression[index] === '\\') {
        index += 2;
        continue;
      }
      if (expression[index] === quote) break;
      index += 1;
    }
    if (expression[index] !== quote) {
      throw new Error('Native Block IconFont Sprite string is unterminated.');
    }
    const literal = expression.slice(literalStart, ++index);
    pieces.push(decodeJavaScriptStringLiteral(literal));
    index = skipWhitespace(expression, index);
    if (index === expression.length) break;
    if (expression[index] !== '+') {
      throw new Error('Native Block IconFont Sprite expression is invalid.');
    }
    index += 1;
  }
  const svg = pieces.join('');
  if (!svg.trimStart().startsWith('<svg')) {
    throw new Error('Native Block IconFont Sprite did not produce SVG.');
  }
  return svg;
}

function decodeJavaScriptStringLiteral(literal: string): string {
  return Function(`"use strict"; return (${literal});`)() as string;
}

function skipWhitespace(value: string, start: number): number {
  let index = start;
  while (/\s/u.test(value[index] ?? '')) index += 1;
  return index;
}
