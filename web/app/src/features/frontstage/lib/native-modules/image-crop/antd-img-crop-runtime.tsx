import type { ComponentPropsWithoutRef, ComponentRef } from 'react';
import { forwardRef, useContext } from 'react';
import reactEasyCropStyles from 'react-easy-crop/react-easy-crop.css?raw';
import { ConfigContext } from 'antd/es/config-provider/context';

import type { NativeReactFrontendModuleLoadResult } from '@1flowbase/page-runtime';

import { useNativeBlockSurface } from '../native-block-surface-context';

type ImgCropComponent = typeof import('antd-img-crop').default;
type ImgCropProps = ComponentPropsWithoutRef<ImgCropComponent>;
type ImgCropperProps = NonNullable<ImgCropProps['cropperProps']>;

const ANTD_IMG_CROP_STYLE_MARKERS = [
  '/*! tailwindcss',
  '.\\[height\\:40vh\\]'
] as const;

let loadFlight: Promise<NativeReactFrontendModuleLoadResult> | undefined;

export function loadAntdImgCropModule(): Promise<NativeReactFrontendModuleLoadResult> {
  loadFlight ??= loadAntdImgCropModuleOnce().catch((error) => {
    loadFlight = undefined;
    throw error;
  });
  return loadFlight;
}

async function loadAntdImgCropModuleOnce(): Promise<NativeReactFrontendModuleLoadResult> {
  if (typeof document === 'undefined') {
    throw new Error('antd-img-crop requires a browser document.');
  }

  const existingStyles = new Set(document.head.querySelectorAll('style'));
  const module = await import('antd-img-crop');
  const packageStyles = [...document.head.querySelectorAll('style')].filter(
    (style) =>
      !existingStyles.has(style) &&
      ANTD_IMG_CROP_STYLE_MARKERS.every((marker) =>
        style.textContent?.includes(marker)
      )
  );
  if (packageStyles.length !== 1) {
    throw new Error(
      `antd-img-crop injected ${packageStyles.length} recognized global style assets; expected exactly one.`
    );
  }

  const packageCss = packageStyles[0]!.textContent ?? '';
  packageStyles[0]!.remove();
  const NativeImgCrop = createNativeImgCropComponent(module.default);

  return {
    module: { default: NativeImgCrop },
    styles: [{ css: `${packageCss}\n${reactEasyCropStyles}` }]
  };
}

function createNativeImgCropComponent(
  ImgCrop: ImgCropComponent
): ImgCropComponent {
  const NativeImgCrop = forwardRef<
    ComponentRef<ImgCropComponent>,
    ComponentPropsWithoutRef<ImgCropComponent>
  >(function NativeImgCrop(
    { cropperProps, modalProps, ...props },
    ref
  ) {
    const { getPopupContainer } = useContext(ConfigContext);
    const surface = useNativeBlockSurface();
    const isolatedCropperProps = {
      ...cropperProps,
      disableAutomaticStylesInjection: true
    } as ImgCropperProps;
    return (
      <ImgCrop
        {...props}
        ref={ref}
        modalProps={{
          getContainer: surface?.overlayHost.container ?? getPopupContainer,
          ...modalProps
        }}
        cropperProps={isolatedCropperProps}
      />
    );
  });
  NativeImgCrop.displayName = 'NativeImgCrop';
  return NativeImgCrop as ImgCropComponent;
}
