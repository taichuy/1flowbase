import { ApiClientError } from '@1flowbase/api-client';
import { describe, expect, test } from 'vitest';

import { getPluginUploadErrorMessage } from '../shared';

describe('getPluginUploadErrorMessage', () => {
  test('replaces a browser transport failure with actionable Chinese guidance', () => {
    expect(getPluginUploadErrorMessage(new TypeError('Failed to fetch'))).toBe(
      '上传请求未完成，请检查网络连接或反向代理后重试。'
    );
  });

  test('explains a backend-reported package platform mismatch in Chinese', () => {
    expect(
      getPluginUploadErrorMessage(
        new ApiClientError({
          status: 400,
          code: 'plugin_runtime_target_mismatch',
          message: 'plugin runtime target linux/arm64 is incompatible with host linux/amd64'
        })
      )
    ).toBe('安装包平台与当前宿主不兼容，请上传 linux-amd64 版本。');
  });
});
