import LoadingOutlined from '@ant-design/icons/es/icons/LoadingOutlined';
import Skeleton from 'antd/es/skeleton';
import Spin from 'antd/es/spin';

import './loading-state.css';

import { createLoadingState } from './create-loading-state';
export type { LoadingStateProps } from './create-loading-state';
export const LoadingState = createLoadingState({
  Spin,
  Skeleton,
  LoadingOutlined
});
