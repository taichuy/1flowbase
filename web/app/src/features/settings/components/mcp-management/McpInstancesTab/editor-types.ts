import type { SaveConsoleMcpInstanceBody } from '@1flowbase/api-client';
import { type CopySettingsMcpInstanceBody } from '../../../api/mcp-management';

export type InstanceFormValues = SaveConsoleMcpInstanceBody;
export type CopyInstanceFormValues = CopySettingsMcpInstanceBody;
export type GroupFormValues = {
  instance_id: string;
  path: string;
  path_segment?: string;
  display_name?: string;
  description_short: string | null;
  enabled: boolean;
  sort_order: number;
};
export type BindingFormValues = {
  instance_id: string;
  group_path: string;
  tool_id: string;
  visible: boolean;
  sort_order: number;
};
