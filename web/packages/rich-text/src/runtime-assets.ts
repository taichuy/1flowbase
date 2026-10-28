import type Vditor from 'vditor';
import { englishEditorMessages } from './locales/en_US';
import { chineseEditorMessages } from './locales/zh_Hans';

import { VDITOR_ASSET_BASE } from './asset-contract';
export { VDITOR_ASSET_BASE } from './asset-contract';

export function getBundledEditorMessages(locale: 'zh_Hans' | 'en_US') {
  return locale === 'en_US' ? englishEditorMessages : chineseEditorMessages;
}

const runtimes = new WeakMap<Document, Promise<typeof Vditor>>();
const codeLanguages = new WeakMap<Document, string[]>();

function loadScript(document: Document, relative: string, id: string) {
  return new Promise<void>((resolve, reject) => {
    const script = document.createElement('script');
    script.src = `${VDITOR_ASSET_BASE}/dist/${relative}`;
    script.onload = () => {
      script.id = id;
      resolve();
    };
    script.onerror = () =>
      reject(new Error(`Vditor resource could not be loaded: ${relative}`));
    document.head.appendChild(script);
  });
}

async function ensureIconSprite(document: Document) {
  if (document.getElementById('vditorIconScript')) return;
  if (document.getElementById('vditor-icon-bold')) {
    // Vditor.destroy removes its script marker but leaves the loaded sprite.
    const marker = document.createElement('script');
    marker.id = 'vditorIconScript';
    marker.type = 'application/x-1flowbase-bundled-support';
    document.head.appendChild(marker);
    return;
  }
  await loadScript(document, 'js/icons/ant.js', 'vditorIconScript');
}

/** Load the constructor in the frame's realm, not in its Shadow DOM host. */
export function loadFrameRuntime(document: Document): Promise<typeof Vditor> {
  const existing = runtimes.get(document);
  if (existing)
    return existing.then(async (constructor) => {
      await ensureIconSprite(document);
      return constructor;
    });
  const runtime = new Promise<typeof Vditor>((resolve, reject) => {
    const style = document.createElement('link');
    style.rel = 'stylesheet';
    style.href = `${VDITOR_ASSET_BASE}/dist/index.css`;
    document.head.appendChild(style);
    // Start independent resources together; only third-languages depends on highlight.
    const resources = Promise.all([
      loadScript(document, 'js/lute/lute.min.js', 'vditorLuteScript'),
      ensureIconSprite(document),
      loadScript(
        document,
        'js/highlight.js/highlight.min.js',
        'vditorHljsScript'
      ).then(() =>
        loadScript(
          document,
          'js/highlight.js/third-languages.js',
          'vditorHljsThirdScript'
        )
      )
    ]);
    resources.catch(reject);
    const script = document.createElement('script');
    script.src = `${VDITOR_ASSET_BASE}/dist/index.min.js`;
    script.onload = () => {
      const view = document.defaultView as
        | (Window & {
            Vditor?: typeof Vditor;
            hljs?: {
              listLanguages(): string[];
              getLanguage(name: string): { aliases?: string[] };
            };
          })
        | null;
      if (!view?.Vditor) {
        reject(new Error('Vditor runtime is unavailable.'));
        return;
      }
      const constructor = view.Vditor;
      void resources
        .then(() => {
          const languages = view
            .hljs!.listLanguages()
            .flatMap((name) => [
              name,
              ...((view.hljs!.getLanguage(name) as { aliases?: string[] })
                .aliases ?? [])
            ]);
          // Only advertise diagram renderers included in the frontend asset contract.
          const unsupported = new Set([
            'abc',
            'echarts',
            'flowchart',
            'graphviz',
            'markmap',
            'mindmap',
            'plantuml',
            'smiles'
          ]);
          codeLanguages.set(
            document,
            [...new Set([...languages, 'mermaid', 'math'])]
              .filter((name) => !unsupported.has(name))
              .sort()
          );
          resolve(constructor);
        })
        .catch(reject);
    };
    script.onerror = () =>
      reject(new Error('Vditor runtime could not be loaded.'));
    document.head.appendChild(script);
  });
  runtimes.set(document, runtime);
  return runtime;
}

export function sharedPreviewOptions(
  locale: 'zh_Hans' | 'en_US',
  dark: boolean,
  document: Document
): IPreviewOptions {
  return {
    cdn: VDITOR_ASSET_BASE,
    i18n: getBundledEditorMessages(locale),
    lang: locale === 'en_US' ? 'en_US' : 'zh_CN',
    mode: dark ? 'dark' : 'light',
    hljs: {
      enable: true,
      style: dark ? 'github-dark' : 'github',
      langs: codeLanguages.get(document)
    },
    markdown: {
      codeBlockPreview: true,
      mathBlockPreview: true,
      sanitize: false
    },
    math: { engine: 'KaTeX' },
    theme: {
      current: dark ? 'dark' : 'ant-design',
      path: `${VDITOR_ASSET_BASE}/dist/css/content-theme`
    }
  };
}
