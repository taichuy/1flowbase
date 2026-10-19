import { ConfigProvider } from 'antd';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { PublicAuthNativeFixture } from './public-auth-native/PublicAuthNativeFixture';

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <ConfigProvider>
      <PublicAuthNativeFixture />
    </ConfigProvider>
  </StrictMode>
);
