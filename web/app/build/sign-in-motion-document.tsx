import { AnonymousLoadingState } from '../src/features/auth/components/anonymous-loading/AnonymousLoadingView';
import { SignInPanel } from '../src/features/auth/components/hero/SignInPanel';
import en from '../src/features/auth/i18n/en_US.json';
import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { build } from 'vite';
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
  const scene = renderToStaticMarkup(
    <div id={documentSceneId}>
      <div className="auth-sign-in-page">
        <div data-hero-artwork="" style={{ display: 'contents' }}>
          <HeroArtwork>
            <div data-opening-brand="" hidden>
              <HeroTagline slogan="" />
            </div>
          </HeroArtwork>
        </div>
        <SignInPanel footer={en.sign_in.footer}>
          <AnonymousLoadingState compact />
        </SignInPanel>
      </div>
    </div>
  );
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
      `<style>${frameCSS}\n${loadingCSS}\n#${documentSceneId} * {box-sizing:border-box}\n#${documentSceneId} ~ #root {display:none}\n[data-opening-brand]:not([hidden]){display:contents}</style></head>`
    )
    .replace(
      '<body>',
      () =>
        `<body>${scene}<template id="sign-in-next-resources">${[...deferred, ...hints].join('\n')}</template><script>${output.output[0].code.replace(/<\/script/gi, '<\\/script')};document.head.append(document.getElementById('sign-in-next-resources').content)</script>`
    );
}
