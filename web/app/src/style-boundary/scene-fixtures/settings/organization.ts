import type { ConsoleDepartment, ConsoleMember } from '@1flowbase/api-client';
const departments: ConsoleDepartment[] = [
  {
    id: 'engineering',
    name: 'Engineering',
    parent_id: null,
    role_codes: ['operator'],
    member_count: 2
  },
  {
    id: 'development',
    name: 'Development',
    parent_id: 'engineering',
    role_codes: ['member'],
    member_count: 1
  },
  {
    id: 'operations',
    name: 'Operations',
    parent_id: null,
    role_codes: [],
    member_count: 1
  }
];
const members: ConsoleMember[] = [
  {
    id: 'boundary-member-1',
    account: 'alex',
    email: 'alex@example.com',
    phone: null,
    name: 'Alex Chen',
    nickname: 'Alex',
    introduction: '',
    default_display_role: null,
    email_login_enabled: true,
    phone_login_enabled: false,
    status: 'active',
    role_codes: ['member'],
    department_ids: ['development'],
    primary_department_id: 'development'
  },
  {
    id: 'boundary-member-2',
    account: 'riley',
    email: 'riley@example.com',
    phone: null,
    name: 'Riley Wang',
    nickname: 'Riley',
    introduction: '',
    default_display_role: null,
    email_login_enabled: true,
    phone_login_enabled: false,
    status: 'active',
    role_codes: ['member'],
    department_ids: ['engineering'],
    primary_department_id: 'engineering'
  },
  {
    id: 'boundary-member-3',
    account: 'sam',
    email: 'sam@example.com',
    phone: null,
    name: 'Sam Liu',
    nickname: 'Sam',
    introduction: '',
    default_display_role: null,
    email_login_enabled: true,
    phone_login_enabled: false,
    status: 'disabled',
    role_codes: ['member'],
    department_ids: ['operations'],
    primary_department_id: 'operations'
  }
];
export function seedStyleBoundaryOrganizationFetch() {
  const fallback = globalThis.fetch.bind(globalThis);
  globalThis.fetch = async (input, init) => {
    const url = new URL(
      input instanceof Request ? input.url : String(input),
      document.baseURI
    );
    let data: unknown;
    switch (url.pathname) {
      case '/api/console/settings/departments/access':
        data = {
          can_list: true,
          can_create: true,
          can_update: true,
          can_delete: true,
          can_assign_roles: true,
          can_replace_member_departments: true
        };
        break;
      case '/api/console/settings/departments':
        data = departments;
        break;
      case '/api/console/settings/members/role-options':
        data = [
          { code: 'member', name: 'Member' },
          { code: 'operator', name: 'Operator' }
        ];
        break;
      case '/api/console/settings/members':
        data = members;
        break;
      default:
        return fallback(input, init);
    }
    return new Response(JSON.stringify({ data, meta: null }), {
      status: 200,
      headers: { 'content-type': 'application/json' }
    });
  };
}
