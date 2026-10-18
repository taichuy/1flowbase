import { ConfigProvider } from 'antd';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { NativeReactTrialFixture } from './native-react-trial/NativeReactTrialFixture';

const root = document.getElementById('root');
if (!root) throw new Error('Native Frontstage fixture root is missing.');
createRoot(root).render(
  <StrictMode>
    <ConfigProvider>
      <NativeReactTrialFixture />
    </ConfigProvider>
  </StrictMode>
);
