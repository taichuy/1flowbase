import { useState } from 'react';
import { jsx, jsxs, Fragment } from 'react/jsx-runtime';
import Alert from 'antd/es/alert';
import Button from 'antd/es/button';
import Input from 'antd/es/input';
import Space from 'antd/es/space';
import ArrowLeftOutlined from '@ant-design/icons/es/icons/ArrowLeftOutlined';
import { createNativeReactModuleRegistry } from '@1flowbase/page-runtime/browser';
import { defaultLoginImports } from './default-login-imports';

// Scoped to the exact shipped source. Use the same components as the full registry,
// including the leaf behind its lazy icon. None require host wrappers or styles.
const modules: Record<string, Record<string, unknown>> = {
  react: { useState },
  'react/jsx-runtime': { jsx, jsxs, Fragment },
  antd: { Alert, Button, Input, Space },
  '@ant-design/icons': { ArrowLeftOutlined }
};
export function createDefaultLoginRegistry() {
  return createNativeReactModuleRegistry(
    defaultLoginImports.map((entry) => ({
      ...entry,
      load: async () => ({ module: modules[entry.module_source] })
    }))
  );
}
