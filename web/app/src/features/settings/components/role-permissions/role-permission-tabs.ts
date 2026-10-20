const CONSOLE_POLICY_TAB = 'console-policy';

const OTHER_POLICY_TAB = 'other-policy';

const DYNAMIC_ROUTE_TAB = 'dynamic-routes';

const ROLE_TABLE_GENERAL_TAB = 'table-general-policy';

const ROLE_TABLE_SINGLE_TAB = 'table-single-policy';

export type RolePermissionTab =
  | typeof CONSOLE_POLICY_TAB
  | typeof OTHER_POLICY_TAB
  | typeof DYNAMIC_ROUTE_TAB
  | typeof ROLE_TABLE_GENERAL_TAB
  | typeof ROLE_TABLE_SINGLE_TAB;
