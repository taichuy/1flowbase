import type { ThemeConfig } from 'antd';
import { FRONTSTAGE_DESIGN_BLUE } from '../../design-mode-theme';

/** Measured from the local NocoBase configuration and table-action buttons. */
export const BLOCK_BUTTON_METRICS = {
  fontSize: 14,
  fontWeight: 400,
  height: 28,
  paddingInline: 11,
  gap: 4,
  borderRadius: 6
} as const;

export const BLOCK_BUTTON_THEME: ThemeConfig = {
  components: {
    Button: {
      fontWeight: BLOCK_BUTTON_METRICS.fontWeight,
      contentFontSizeSM: BLOCK_BUTTON_METRICS.fontSize,
      controlHeightSM: BLOCK_BUTTON_METRICS.height,
      paddingInlineSM: BLOCK_BUTTON_METRICS.paddingInline,
      iconGap: BLOCK_BUTTON_METRICS.gap,
      borderRadiusSM: BLOCK_BUTTON_METRICS.borderRadius
    }
  }
};

/** Override the brand theme's explicit component accents only while designing. */
export const BLOCK_DESIGN_THEME: ThemeConfig = {
  token: {
    colorLink: FRONTSTAGE_DESIGN_BLUE.primary,
    colorLinkHover: FRONTSTAGE_DESIGN_BLUE.primaryStrong,
    colorPrimary: FRONTSTAGE_DESIGN_BLUE.primary,
    colorPrimaryHover: FRONTSTAGE_DESIGN_BLUE.primaryStrong,
    colorPrimaryActive: FRONTSTAGE_DESIGN_BLUE.primaryStrong
  },
  components: {
    Button: {
      ...BLOCK_BUTTON_THEME.components?.Button,
      colorPrimary: FRONTSTAGE_DESIGN_BLUE.primary,
      colorPrimaryHover: FRONTSTAGE_DESIGN_BLUE.primaryStrong,
      colorPrimaryActive: FRONTSTAGE_DESIGN_BLUE.primaryStrong,
      primaryColor: '#fff',
      defaultColor: FRONTSTAGE_DESIGN_BLUE.primary,
      defaultBorderColor: FRONTSTAGE_DESIGN_BLUE.borderHover,
      defaultHoverColor: FRONTSTAGE_DESIGN_BLUE.primaryStrong,
      defaultHoverBorderColor: FRONTSTAGE_DESIGN_BLUE.primaryStrong,
      defaultHoverBg: FRONTSTAGE_DESIGN_BLUE.bgHover,
      defaultActiveColor: FRONTSTAGE_DESIGN_BLUE.primaryStrong,
      defaultActiveBorderColor: FRONTSTAGE_DESIGN_BLUE.primaryStrong,
      defaultActiveBg: FRONTSTAGE_DESIGN_BLUE.bgSelected
    },
    Input: {
      hoverBorderColor: FRONTSTAGE_DESIGN_BLUE.primary,
      activeBorderColor: FRONTSTAGE_DESIGN_BLUE.primary,
      activeShadow: FRONTSTAGE_DESIGN_BLUE.halo
    },
    Select: {
      hoverBorderColor: FRONTSTAGE_DESIGN_BLUE.primary,
      activeBorderColor: FRONTSTAGE_DESIGN_BLUE.primary,
      activeOutlineColor: FRONTSTAGE_DESIGN_BLUE.borderSelected
    }
  }
};
