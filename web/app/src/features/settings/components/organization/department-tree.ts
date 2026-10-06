import type { SettingsDepartment } from '../../api/departments';
export interface DepartmentTreeRow extends SettingsDepartment {
  children?: DepartmentTreeRow[];
}
export function departmentTree(
  departments: SettingsDepartment[]
): DepartmentTreeRow[] {
  const nodes = new Map(
    departments.map((department) => [
      department.id,
      { ...department } as DepartmentTreeRow
    ])
  );
  const roots: DepartmentTreeRow[] = [];
  for (const department of departments) {
    const node = nodes.get(department.id)!;
    const parent = department.parent_id
      ? nodes.get(department.parent_id)
      : undefined;
    if (parent) (parent.children ??= []).push(node);
    else roots.push(node);
  }
  return roots;
}
