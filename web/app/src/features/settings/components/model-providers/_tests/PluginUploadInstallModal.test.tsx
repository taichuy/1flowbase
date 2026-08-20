import { render, screen } from '@testing-library/react';
import type { UploadFile } from 'antd/es/upload/interface';
import { describe, expect, test, vi } from 'vitest';

import { PluginUploadInstallModal } from '../PluginUploadInstallModal';

describe('PluginUploadInstallModal', () => {
  test('renders a constrained filename trigger that retains the full name', () => {
    const filename =
      '1flowbase@chatgpt@0.1.0@linux-amd64@67246ae7b0c0df1b4dfdc4dfff2d7a67f4aa5d5837a230b528221c0b3f5aeb3d.1flowbasepkg';
    render(
      <PluginUploadInstallModal
        open
        submitting={false}
        resultSummary={null}
        errorMessage={null}
        fileList={[
          { uid: 'plugin-package', name: filename, status: 'done' }
        ] as UploadFile[]}
        onClose={vi.fn()}
        onChange={vi.fn()}
        onSubmit={vi.fn()}
      />
    );

    expect(screen.getByText(filename)).toHaveClass(
      'model-provider-panel__upload-file-name'
    );
    expect(screen.getByText(filename)).toHaveAttribute('title', filename);
  });
});
