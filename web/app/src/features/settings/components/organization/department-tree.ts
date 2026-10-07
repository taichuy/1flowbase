import type { DepartmentTreeItem } from '../../api/departments';
export interface DepartmentTreeRow extends DepartmentTreeItem {
  children?: DepartmentTreeRow[];
}
export function departmentTree(
  departments: DepartmentTreeItem[]
): DepartmentTreeRow[] {
  const nodes = new Map(
    departments.map((department) => [
      department.id,
      {
        ...department,
        ...(department.has_children ? { children: [] } : {})
      } as DepartmentTreeRow
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
