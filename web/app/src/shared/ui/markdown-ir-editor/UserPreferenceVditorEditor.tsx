import { VditorEditor, type VditorEditorProps } from '@1flowbase/rich-text';
import { useConsoleOperationAccess } from '../../../state/console-operation-access';
import { hasUserPreferenceWriteAccess } from '../../user-preferences/user-preferences';

/** Host-owned persistence applies to shell and native module consumers alike. */
export function UserPreferenceVditorEditor(props: VditorEditorProps) {
  // Warm the decision while the editor is mounted; the callback rechecks on write.
  useConsoleOperationAccess()('patch_me_meta');
  return (
    <VditorEditor
      {...props}
      canPersistModePreference={hasUserPreferenceWriteAccess}
    />
  );
}
