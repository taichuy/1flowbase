import { StyleProvider, createCache } from '@ant-design/cssinjs';
import { Affix as AntdAffix, type AffixProps, type AffixRef } from 'antd';
import {
  forwardRef,
  useImperativeHandle,
  useLayoutEffect,
  useMemo,
  useRef
} from 'react';
import { createPortal } from 'react-dom';

import {
  attachNativeAffixLayer,
  createNativeAffixPortal,
  type NativeAffixLayer
} from './native-affix-layer';
import { useNativeBlockSurface } from './native-block-surface-context';

export const NativeBlockAffix = forwardRef<AffixRef, AffixProps>(
  function NativeBlockAffixComponent(
    {
      children,
      className,
      offsetBottom,
      offsetTop,
      onChange,
      prefixCls,
      rootClassName,
      style,
      target
    },
    ref
  ) {
    const surface = useNativeBlockSurface();
    const surfaceScrollOwner = surface?.scrollOwner;
    const targetRoot = surface?.targetRoot;
    const sentinelRef = useRef<HTMLSpanElement | null>(null);
    const placeholderRef = useRef<HTMLDivElement | null>(null);
    const onChangeRef = useRef(onChange);
    const optionsRef = useRef({
      offset: 0,
      placement: 'top' as 'top' | 'bottom'
    });
    const layerRef = useRef<NativeAffixLayer | null>(null);
    const styleCache = useMemo(() => createCache(), []);
    const portal = useMemo(() => {
      if (!targetRoot) return null;
      const blockId =
        targetRoot.host.getAttribute('data-flowbase-native-trusted-block-id') ??
        'native-affix';
      return createNativeAffixPortal(blockId, targetRoot.ownerDocument);
    }, [targetRoot]);
    const updatePosition = useMemo(
      () =>
        Object.assign(() => layerRef.current?.refresh(), {
          cancel: () => undefined
        }),
      []
    );
    onChangeRef.current = onChange;
    optionsRef.current = {
      offset: offsetBottom ?? offsetTop ?? 0,
      placement: offsetBottom === undefined ? 'top' : 'bottom'
    };

    useImperativeHandle(ref, () => ({ updatePosition }), [updatePosition]);

    useLayoutEffect(() => {
      const placeholder = placeholderRef.current;
      const sentinel = sentinelRef.current;
      const scrollOwner = target?.() ?? surfaceScrollOwner;
      if (!targetRoot || !scrollOwner || !placeholder || !portal || !sentinel) {
        return;
      }
      const blockId =
        targetRoot.host.getAttribute('data-flowbase-native-trusted-block-id') ??
        'native-affix';
      const nextLayer = attachNativeAffixLayer({
        blockId,
        onPinnedChange: (pinned) => onChangeRef.current?.(pinned),
        options: () => optionsRef.current,
        placeholder,
        portal,
        scrollOwner,
        sentinel,
        surfaceHost: targetRoot.host as HTMLElement
      });
      layerRef.current = nextLayer;
      const sync = () => {
        const height = nextLayer.mountElement.getBoundingClientRect().height;
        if (height > 0) placeholder.style.height = `${height}px`;
        nextLayer.refresh();
      };
      sync();
      const observer =
        typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(sync);
      observer?.observe(nextLayer.mountElement);
      return () => {
        observer?.disconnect();
        layerRef.current = null;
        nextLayer.dispose();
      };
    }, [portal, surfaceScrollOwner, target, targetRoot]);

    if (!targetRoot || !surfaceScrollOwner) {
      return (
        <AntdAffix
          className={className}
          offsetBottom={offsetBottom}
          offsetTop={offsetTop}
          onChange={onChange}
          prefixCls={prefixCls}
          rootClassName={rootClassName}
          style={style}
          target={target}
        >
          {children}
        </AntdAffix>
      );
    }

    const mergedClassName = [rootClassName, prefixCls, className]
      .filter(Boolean)
      .join(' ');
    return (
      <>
        <span
          ref={sentinelRef}
          data-flowbase-native-affix-sentinel=""
          aria-hidden="true"
          style={{ display: 'block', height: 0, pointerEvents: 'none' }}
        />
        <div
          ref={placeholderRef}
          data-flowbase-native-affix-placeholder=""
          style={{ display: 'flow-root', width: '100%' }}
        />
        {portal
          ? createPortal(
              <StyleProvider cache={styleCache} container={portal.shadowRoot}>
                <div className={mergedClassName || undefined} style={style}>
                  {children}
                </div>
              </StyleProvider>,
              portal.mountElement
            )
          : null}
      </>
    );
  }
) as typeof AntdAffix;
