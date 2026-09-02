import AntdApp from 'antd/es/app';
import ConfigProvider from 'antd/es/config-provider';
import type { PropsWithChildren } from 'react';

import { emeraldLightTheme } from './theme';

export function AppThemeProvider({ children }: PropsWithChildren) {
  return (
    <ConfigProvider theme={emeraldLightTheme}>
      <AntdApp>{children}</AntdApp>
    </ConfigProvider>
  );
}
