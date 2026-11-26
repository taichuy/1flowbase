import { AnonymousLoadingState } from '../src/features/auth/components/anonymous-loading/AnonymousLoadingView';
import { SignInPanel } from '../src/features/auth/components/hero/SignInPanel';
import en from '../src/features/auth/i18n/en_US.json';
import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { build } from 'vite';
import { publicSignInPlugin } from './public-sign-in';
import {
  HeroArtwork,
  HeroTagline
} from '../src/features/auth/components/hero/HeroArtwork';
import { documentSceneId } from '../src/features/auth/components/hero/document-motion';

// Build the real scene's input/drawing loop as a closed graph: no React, theme,
// router or deferred UI dependency. Auth discovery starts in the same HTML wave.
export async function motionDocument(
  html: string,
  root: string,
  hints: string[]
) {
  const result = await build({
    configFile: false,
    publicDir: false,
    root,
    logLevel: 'silent',
    plugins: [publicSignInPlugin(root)],
    build: {
      write: false,
      copyPublicDir: false,
      lib: {
        entry: path.join(
          root,
          'src/features/auth/components/hero/start-document-motion.ts'
        ),
        formats: ['iife'],
        name: 'SignInDocumentMotion'
      }
    }
  });
  const output = Array.isArray(result) ? result[0] : result;
  if (
    !('output' in output) ||
    output.output.length !== 1 ||
    output.output[0].type !== 'chunk' ||
    output.output[0].imports.length ||
    output.output[0].dynamicImports.length
  )
    throw new Error('Document motion must be one self-contained script');
  const frameCSS = readFileSync(
    path.join(root, 'src/features/auth/pages/sign-in-frame.css'),
    'utf8'
  );
  const loadingCSS = readFileSync(
    path.join(
      root,
      'src/features/auth/components/anonymous-loading/anonymous-loading.css'
    ),
    'utf8'
  );
  // preprocessCSS is deliberately non-minifying in Vite 8.3. Use the public
  // build pipeline so inline CSS receives the same target-aware minifier as
  // emitted styles, without importing a transitive dependency or trimming CSS.
  const styleId = path.join(root, 'sign-in-opening.css');
  const minifyStyle = async (css: string) => {
    const result = await build({
      configFile: false,
      root,
      publicDir: false,
      logLevel: 'silent',
      plugins: [
        {
          name: '1flowbase-opening-css',
          resolveId: (id) => (id === styleId ? styleId : undefined),
          load: (id) => (id === styleId ? css : undefined)
        }
      ],
      build: {
        write: false,
        copyPublicDir: false,
        cssMinify: 'lightningcss',
        rolldownOptions: { input: styleId }
      }
    });
    const output = Array.isArray(result) ? result[0] : result;
    if (!('output' in output))
      throw new Error('Opening CSS build missing output');
    const style = output.output.find(
      (item) => item.type === 'asset' && item.fileName.endsWith('.css')
    );
    if (!style || style.type !== 'asset')
      throw new Error('Opening CSS build missing stylesheet');
    return { code: String(style.source).trimEnd() };
  };
  const styles = await minifyStyle(`${frameCSS}\n${loadingCSS}`);
  let scene = renderToStaticMarkup(
    <div id={documentSceneId}>
      <div className="auth-sign-in-page">
        <div data-hero-artwork="" style={{ display: 'contents' }}>
          <HeroArtwork>
            <div data-opening-brand="" hidden>
              <HeroTagline slogan="" />
            </div>
          </HeroArtwork>
        </div>
        <SignInPanel footer={en.sign_in.footer} openingFooterHidden>
          <AnonymousLoadingState compact />
        </SignInPanel>
      </div>
    </div>
  );
  // Hero's existing inline style literal has the same CSS-parser minification;
  // markup, geometry, keyframes and animation ownership remain unchanged.
  for (const match of [...scene.matchAll(/<style>([\s\S]*?)<\/style>/gu)]) {
    scene = scene.replace(
      match[0],
      `<style>${(await minifyStyle(match[1])).code}</style>`
    );
  }
  // The opening owns the loading skeleton; the hidden generic entry skeleton
  // is redundant. Keep base/reset and failure/retry styles for load failures.
  html = html
    .replace(
      /<style\s+data-document-skeleton(?:="")?[^>]*>[\s\S]*?<\/style>/gu,
      ''
    )
    .replace(
      /<!-- document-skeleton:start -->[\s\S]*?<!-- document-skeleton:end -->/gu,
      ''
    );
  for (const match of [...html.matchAll(/<style>([\s\S]*?)<\/style>/gu)]) {
    html = html.replace(
      match[0],
      `<style>${(await minifyStyle(match[1])).code}</style>`
    );
  }
  // Vite's generated head hints belong to stage two; the static scene carries its own CSS.
  const deferred: string[] = [];
  const document = html.replace(
    /<link rel="(?:modulepreload|stylesheet)"[^>]*>/g,
    (link) => {
      deferred.push(link);
      return '';
    }
  );
  return document
    .replace(
      '</head>',
      `<style>${styles.code}\n#${documentSceneId}{color:#16211d;font:14px/1.5714285714285714 'IBM Plex Sans','Segoe UI',sans-serif}
#${documentSceneId} * {box-sizing:border-box}\n#${documentSceneId} ~ #root {display:none}\n[data-opening-brand]:not([hidden]){display:contents}</style></head>`
    )
    .replace(
      '<body>',
      () =>
        `<body>${scene}<template id="sign-in-next-resources">${[...deferred, ...hints].join('\n')}</template><script>${output.output[0].code.replace(/<\/script/gi, '<\\/script')};document.head.append(document.getElementById('sign-in-next-resources').content)</script>`
    );
}
