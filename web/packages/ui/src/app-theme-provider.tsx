import { App as AntdApp, ConfigProvider } from 'antd';
import type { PropsWithChildren } from 'react';

import { emeraldLightTheme } from './theme';

export function AppThemeProvider({ children }: PropsWithChildren) {
  return (
    <ConfigProvider theme={emeraldLightTheme}>
      <AntdApp>{children}</AntdApp>
    </ConfigProvider>
  );
}
