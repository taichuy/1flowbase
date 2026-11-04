import { VditorEditor, type VditorEditorProps } from '@1flowbase/rich-text';
import { hasUserPreferenceWriteAccess } from '../../user-preferences/user-preferences';

/** Host-owned persistence applies to shell and native module consumers alike. */
export function UserPreferenceVditorEditor(props: VditorEditorProps) {
  return (
    <VditorEditor
      {...props}
      canPersistModePreference={hasUserPreferenceWriteAccess}
    />
  );
}
