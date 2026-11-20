// Only plain, unwrapped host exports used by the shipped password template.
// Any other binding keeps the full configured block runtime.
export const defaultLoginImports = [
  { module_source: 'react', exports: ['useState'] },
  { module_source: 'react/jsx-runtime', exports: ['jsx', 'jsxs', 'Fragment'] },
  { module_source: 'antd', exports: ['Alert', 'Button', 'Input', 'Space'] },
  { module_source: '@ant-design/icons', exports: ['ArrowLeftOutlined'] }
];
