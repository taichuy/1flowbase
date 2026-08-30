import type { ThemeConfig } from 'antd';
import defaultAlgorithm from 'antd/es/theme/themes/default';

export const emeraldLightTheme: ThemeConfig = {
  algorithm: defaultAlgorithm,
  token: {
    colorPrimary: '#00ab73',
    colorSuccess: '#19b36b',
    colorWarning: '#ffba00',
    colorError: '#fb565b',
    colorInfo: '#2bb9b1',
    colorBgBase: '#f4f8f5',
    colorBgContainer: 'rgba(255, 255, 255, 0.9)',
    colorBgElevated: '#fcfffd',
    colorText: '#16211d',
    colorTextSecondary: '#55645d',
    colorTextTertiary: '#7b8982',
    colorBorder: '#d5ddd8',
    colorBorderSecondary: '#e7ede9',
    colorFillSecondary: '#f2f6f3',
    borderRadius: 8,
    borderRadiusLG: 12,
    controlHeight: 32,
    fontFamily: '"IBM Plex Sans", "Segoe UI", sans-serif',
    boxShadowSecondary: '0 18px 50px rgba(15, 23, 20, 0.08)'
  },
  components: {
    Layout: {
      headerBg: 'transparent',
      bodyBg: 'transparent',
      headerColor: '#16211d'
    },
    Card: {
      headerBg: 'transparent'
    },
    Button: {
      fontWeight: 600,
      defaultBg: 'rgba(255, 255, 255, 0.88)',
      defaultBorderColor: '#bcc8c1',
      defaultColor: '#16211d',
      defaultHoverBg: 'rgba(255, 255, 255, 0.88)',
      defaultHoverColor: '#16211d',
      defaultHoverBorderColor: '#00ab73',
      colorPrimary: 'rgba(0, 171, 115, 0.06)',
      colorPrimaryHover: 'rgba(0, 171, 115, 0.12)',
      colorPrimaryActive: 'rgba(0, 171, 115, 0.18)',
      primaryColor: '#008f5f',
      primaryShadow: 'none'
    },
    Input: {
      hoverBorderColor: '#bcc8c1',
      activeBorderColor: '#00ab73',
      activeShadow: '0 0 0 3px rgba(0, 171, 115, 0.12)'
    },
    Select: {
      hoverBorderColor: '#bcc8c1',
      activeBorderColor: '#00ab73',
      activeOutlineColor: 'rgba(0, 171, 115, 0.12)'
    },
    Tabs: {
      itemColor: '#55645d',
      itemHoverColor: '#16211d',
      itemSelectedColor: '#16211d',
      inkBarColor: '#00ab73'
    },
    Table: {
      headerBg: '#f8fcf9',
      headerColor: '#55645d',
      borderColor: '#e8edea',
      headerSplitColor: '#e8edea'
    },
    Descriptions: {
      labelBg: '#f8fcf9',
      labelColor: '#55645d',
      contentColor: '#16211d'
    }
  }
};
