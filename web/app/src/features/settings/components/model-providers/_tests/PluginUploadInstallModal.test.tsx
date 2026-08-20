import { render, screen } from '@testing-library/react';
import type { UploadFile } from 'antd/es/upload/interface';
import { describe, expect, test, vi } from 'vitest';

import { PluginUploadInstallModal } from '../PluginUploadInstallModal';

describe('PluginUploadInstallModal', () => {
  test('keeps the controlled long package filename and upload error in the modal content', () => {
    const filename = `${'a'.repeat(256)}.1flowbasepkg`;
    const errorMessage =
      'Plugin upload failed. Check the package and try again.';
    render(
      <PluginUploadInstallModal
        open
        submitting={false}
        resultSummary={null}
        errorMessage={errorMessage}
        fileList={
          [
            { uid: 'plugin-package', name: filename, status: 'done' }
          ] as UploadFile[]
        }
        onClose={vi.fn()}
        onChange={vi.fn()}
        onSubmit={vi.fn()}
      />
    );

    const modalContent = document.querySelector(
      '.model-provider-panel__upload-modal'
    );
    const uploadControl = document.querySelector(
      '.model-provider-panel__upload-control'
    );
    const selectedFileList = document.querySelector(
      '.model-provider-panel__upload-file-list'
    );
    const filenameTrigger = screen.getByText(filename);
    const errorAlert = screen.getByText(errorMessage);

    if (!modalContent || !uploadControl || !selectedFileList) {
      throw new Error('Expected the plugin upload modal content to render.');
    }

    expect(modalContent).toContainElement(filenameTrigger);
    expect(selectedFileList).toContainElement(filenameTrigger);
    expect(uploadControl).not.toContainElement(filenameTrigger);
    expect(modalContent).toContainElement(errorAlert);
    expect(filenameTrigger).toHaveAttribute('title', filename);
  });
});
