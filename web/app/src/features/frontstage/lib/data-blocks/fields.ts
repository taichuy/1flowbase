import type { DataBlockField } from './types';

export interface DataFieldOption { label: string; value: string | number | boolean; }

export function dataFieldOptions(field: DataBlockField): DataFieldOption[] {
  if (!field.display_options || typeof field.display_options !== 'object') return [];
  const options = (field.display_options as Record<string, unknown>).options;
  if (!Array.isArray(options)) return [];
  return options.flatMap((option: unknown) => {
    if (!option || typeof option !== 'object') return [];
    const { label, value } = option as Record<string, unknown>;
    if (typeof label !== 'string' || !['string', 'number', 'boolean'].includes(typeof value)) return [];
    return [{ label, value: value as string | number | boolean }];
  });
}

export function isMultipleDataField(field: DataBlockField): boolean {
  return field.field_kind === 'many_to_many' || field.display_interface === 'multi_select' || field.display_interface === 'checkbox_group';
}

export function dataFieldText(field: DataBlockField, value: unknown): string {
  if (value == null) return '';
  const options = dataFieldOptions(field);
  if (Array.isArray(value)) return value.map((item) => dataFieldText(field, item)).join(', ');
  const option = options.find((item) => item.value === value);
  if (option) return option.label;
  if (typeof value === 'object') {
    const record = value as Record<string, unknown>;
    // Expanded relations keep their backend keys; choose a display value without remapping DTOs.
    const display = record.title ?? record.name ?? record.id;
    return display == null ? '' : String(display);
  }
  return String(value);
}

export function dataBlockError(error: unknown): string {
  return error instanceof Error ? error.message : String(error ?? '');
}
