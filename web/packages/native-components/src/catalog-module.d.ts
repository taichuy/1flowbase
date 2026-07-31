declare module '@1flowbase/native-components' {
  import type {
    ComponentType,
    ElementType,
    HTMLAttributes,
    ReactNode
  } from 'react';

  export interface SurfaceProps extends HTMLAttributes<HTMLElement> {
    readonly as?: ElementType;
    readonly children?: ReactNode;
  }
  export type ScrollableSurfaceProps = SurfaceProps;
  export const Surface: ComponentType<SurfaceProps>;
  export const ScrollableSurface: ComponentType<ScrollableSurfaceProps>;
}
