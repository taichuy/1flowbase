import type { ThemeConfig } from 'antd';
import { FRONTSTAGE_DESIGN_BLUE } from '../../../lib/design-mode-theme';

/** Override the brand theme's explicit component accents only while designing. */
export const DATA_BLOCK_DESIGN_THEME: ThemeConfig = {
  token: {
    colorPrimary: FRONTSTAGE_DESIGN_BLUE.primary,
    colorPrimaryHover: FRONTSTAGE_DESIGN_BLUE.primaryStrong,
    colorPrimaryActive: FRONTSTAGE_DESIGN_BLUE.primaryStrong
  },
  components: {
    Button: {
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
