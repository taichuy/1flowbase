import {
  Alert as AntdAlert,
  Badge as AntdBadge,
  Button as AntdButton,
  Checkbox as AntdCheckbox,
  DatePicker as AntdDatePicker,
  Divider as AntdDivider,
  Empty as AntdEmpty,
  Input as AntdInput,
  InputNumber as AntdInputNumber,
  Select as AntdSelect,
  Switch as AntdSwitch,
  Typography
} from 'antd';
import { Fragment, useState, type CSSProperties, type ReactNode } from 'react';

import {
  validateBlockUiSchema,
  type BlockProtocolError,
  type BlockUiPrimitive,
  type BlockUiSchemaNode,
  type BlockUiSchemaValidationOptions,
  type BlockUiStyle
} from '@1flowbase/page-protocol';

export { BlockUiLoadingShell } from './BlockUiLoadingShell';

const { Text: AntdText, Title: AntdTitle, Paragraph } = Typography;
const { TextArea } = AntdInput;

export interface BlockRendererActionEvent {
  type: 'action';
  primitive: Extract<BlockUiPrimitive, 'Button' | 'IconButton'>;
  actionId: string;
  key?: string;
  payload?: unknown;
  formValues?: Record<string, unknown>;
}

export interface BlockUiRendererProps {
  schema: unknown;
  validationOptions?: BlockUiSchemaValidationOptions;
  onAction?: (event: BlockRendererActionEvent) => void;
}

interface RenderContext {
  onAction?: (event: BlockRendererActionEvent) => void;
  fieldName?: string;
  formValues: Record<string, unknown>;
  setFormValue: (name: string, value: unknown) => void;
}

type SafeProps = Record<string, unknown>;

const spacingTokens: Record<string, string | number> = {
  'space.0': 0,
  'space.1': 4,
  'space.2': 8,
  'space.3': 12,
  'space.4': 16,
  'space.5': 20,
  'space.6': 24
};

const colorTokens: Record<string, string> = {
  'surface.default': '#ffffff',
  'surface.subtle': '#f8fcf9',
  'text.primary': '#16211d',
  'text.secondary': '#55645d',
  'text.tertiary': '#7b8982',
  'border.default': '#d5ddd8',
  'accent.primary': '#00ab73',
  'status.success': '#19b36b',
  'status.warning': '#ffba00',
  'status.error': '#fb565b'
};

const typographyTokens: Record<string, string | number> = {
  'font.size.caption': 12,
  'font.size.body': 14,
  'font.size.title': 16,
  regular: 400,
  medium: 500,
  semibold: 600,
  left: 'left',
  center: 'center',
  right: 'right'
};

const borderTokens: Record<string, string | number> = {
  'border.0': 0,
  'border.1': 1,
  'border.default': '#d5ddd8',
  solid: 'solid',
  dashed: 'dashed'
};

const radiusTokens: Record<string, string | number> = {
  'radius.0': 0,
  'radius.1': 4,
  'radius.2': 6,
  'radius.3': 8
};

const layoutTokens: Record<string, string | number> = {
  full: '100%',
  auto: 'auto',
  hidden: 'hidden',
  visible: 'visible',
  start: 'flex-start',
  center: 'center',
  end: 'flex-end',
  between: 'space-between',
  row: 'row',
  column: 'column',
  wrap: 'wrap',
  nowrap: 'nowrap'
};

export function BlockUiRenderer({
  schema,
  validationOptions,
  onAction
}: BlockUiRendererProps) {
  const [formValues, setFormValues] = useState<Record<string, unknown>>({});
  const validation = validateBlockUiSchema(schema, validationOptions);
  if (!validation.ok) {
    return <BlockRendererError errors={validation.errors} />;
  }

  try {
    const context: RenderContext = {
      onAction,
      formValues,
      setFormValue(name, value) {
        setFormValues((current) => ({ ...current, [name]: value }));
      }
    };
    return (
      <>{renderNode(validation.schema, context)}</>
    );
  } catch {
    return (
      <BlockRendererError
        errors={[
          {
            code: 'schema_invalid',
            path: 'root',
            message: 'Block UI renderer failed.'
          }
        ]}
      />
    );
  }
}

export function BlockRendererError({
  errors
}: {
  errors: readonly BlockProtocolError[];
}) {
  const firstError = errors[0];

  return (
    <AntdAlert
      role="alert"
      type="error"
      showIcon
      message="Block UI schema is invalid"
      description={
        firstError
          ? `${firstError.code}: ${firstError.path} ${firstError.message}`
          : 'schema_invalid: root Block UI schema is invalid.'
      }
    />
  );
}

function renderNode(node: BlockUiSchemaNode, context: RenderContext): ReactNode {
  const key = node.key;
  const props = getSafeProps(node.props);
  const style = mapStyle(node.style);
  const children = renderChildren(node, context);

  switch (node.primitive) {
    case 'Stack':
      return (
        <div key={key} style={{ display: 'flex', flexDirection: 'column', ...style }}>
          {children}
        </div>
      );
    case 'Inline':
      return (
        <div key={key} style={{ display: 'flex', alignItems: 'center', ...style }}>
          {children}
        </div>
      );
    case 'Grid':
      return (
        <div key={key} style={{ display: 'grid', ...gridStyle(props), ...style }}>
          {children}
        </div>
      );
    case 'Divider':
      return <AntdDivider key={key} style={style} />;
    case 'Text':
      return (
        <AntdText key={key} style={style}>
          {readRenderable(props.children)}
        </AntdText>
      );
    case 'Title':
      return (
        <AntdTitle key={key} level={readTitleLevel(props.level)} style={style}>
          {readRenderable(props.children)}
        </AntdTitle>
      );
    case 'Caption':
      return (
        <Paragraph key={key} type="secondary" style={{ marginBottom: 0, ...style }}>
          {readRenderable(props.children)}
        </Paragraph>
      );
    case 'Badge':
      return (
        <AntdBadge
          key={key}
          status={readBadgeStatus(props.status)}
          text={readRenderable(props.children)}
          style={style}
        />
      );
    case 'Table':
      return renderTable(node, props, style);
    case 'Descriptions':
      return renderDescriptions(node, props, style);
    case 'Empty':
      return (
        <AntdEmpty
          key={key}
          description={readString(props.description) ?? 'No data'}
          style={style}
        />
      );
    case 'Alert':
      return (
        <AntdAlert
          key={key}
          type={readAlertType(props.type)}
          message={readRenderable(props.message)}
          description={readRenderable(props.description)}
          showIcon={readBoolean(props.showIcon)}
          style={style}
        />
      );
    case 'Form':
      return (
        <div key={key} role="form" style={style}>
          {children}
        </div>
      );
    case 'FormItem':
      return renderFormItem(node, props, style, context);
    case 'Input':
      return renderInput(node, props, style, context);
    case 'Textarea':
      {
        const fieldName = context.fieldName ?? readString(props.id);
      return (
        <TextArea
          key={key}
          id={readString(props.id)}
          placeholder={readString(props.placeholder)}
          value={readInputValue(readFieldValue(context, fieldName, props.value))}
          disabled={readBoolean(props.disabled)}
          readOnly={readBoolean(props.readOnly)}
          style={style}
          onChange={(event) => updateFieldValue(context, fieldName, event.target.value)}
        />
      );
      }
    case 'Select':
      return renderSelect(node, props, style, context);
    case 'Checkbox':
      {
        const fieldName = context.fieldName ?? readString(props.id);
      return (
        <AntdCheckbox
          key={key}
          id={readString(props.id)}
          checked={readBoolean(readFieldValue(context, fieldName, props.checked))}
          disabled={readBoolean(props.disabled)}
          style={style}
          onChange={(event) => updateFieldValue(context, fieldName, event.target.checked)}
        >
          {readRenderable(props.children)}
        </AntdCheckbox>
      );
      }
    case 'Switch':
      {
        const fieldName = context.fieldName ?? readString(props.id);
      return (
        <AntdSwitch
          key={key}
          id={readString(props.id)}
          checked={readBoolean(readFieldValue(context, fieldName, props.checked))}
          disabled={readBoolean(props.disabled)}
          style={style}
          onChange={(checked) => updateFieldValue(context, fieldName, checked)}
        />
      );
      }
    case 'DatePicker':
      {
        const fieldName = context.fieldName ?? readString(props.id);
      return (
        <AntdDatePicker
          key={key}
          id={readString(props.id)}
          placeholder={readString(props.placeholder)}
          disabled={readBoolean(props.disabled)}
          style={style}
          onChange={(_date, dateString) => updateFieldValue(context, fieldName, dateString)}
        />
      );
      }
    case 'NumberInput':
      {
        const fieldName = context.fieldName ?? readString(props.id);
      return (
        <AntdInputNumber
          key={key}
          id={readString(props.id)}
          placeholder={readString(props.placeholder)}
          value={readNumber(readFieldValue(context, fieldName, props.value))}
          min={readNumber(props.min)}
          max={readNumber(props.max)}
          disabled={readBoolean(props.disabled)}
          readOnly={readBoolean(props.readOnly)}
          style={style}
          onChange={(value) => updateFieldValue(context, fieldName, value)}
        />
      );
      }
    case 'Button':
      return renderActionButton(node, props, style, context, false);
    case 'IconButton':
      return renderActionButton(node, props, style, context, true);
    case 'Modal':
      return renderModal(node, props, style, children);
    default:
      return null;
  }
}

function renderChildren(node: BlockUiSchemaNode, context: RenderContext): ReactNode {
  if (node.children !== undefined) {
    return node.children.map((child, index) => (
      <Fragment key={child.key ?? `${child.primitive}-${index}`}>
        {renderNode(child, context)}
      </Fragment>
    ));
  }

  const props = getSafeProps(node.props);
  return readRenderable(props.children);
}

function renderTable(
  node: BlockUiSchemaNode,
  props: SafeProps,
  style: CSSProperties
): ReactNode {
  const columns = readColumns(props.columns);
  const rows = readRows(props.dataSource ?? props.data);
  const rowKey = readString(props.rowKey) ?? 'key';

  return (
    <table key={node.key} style={{ width: '100%', borderCollapse: 'collapse', ...style }}>
      <thead>
        <tr>
          {columns.map((column) => (
            <th key={column.key} scope="col" style={tableCellStyle}>
              {column.title}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.map((row, index) => (
          <tr key={readRowKey(row, rowKey, index)}>
            {columns.map((column) => (
              <td key={column.key} style={tableCellStyle}>
                {readRenderable(row[column.dataIndex])}
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function renderDescriptions(
  node: BlockUiSchemaNode,
  props: SafeProps,
  style: CSSProperties
): ReactNode {
  const items = readDescriptionItems(props.items);

  return (
    <dl key={node.key} style={{ margin: 0, ...style }}>
      {items.map((item) => (
        <div key={item.key} style={{ display: 'grid', gridTemplateColumns: '120px 1fr' }}>
          <dt>{item.label}</dt>
          <dd style={{ margin: 0 }}>{readRenderable(item.children)}</dd>
        </div>
      ))}
    </dl>
  );
}

function renderFormItem(
  node: BlockUiSchemaNode,
  props: SafeProps,
  style: CSSProperties,
  context: RenderContext
): ReactNode {
  const id = readString(props.id) ?? readString(props.name);
  const fieldName = readString(props.name) ?? id;
  const label = readString(props.label);

  return (
    <div key={node.key} style={{ display: 'grid', gap: 4, marginBottom: 12, ...style }}>
      {label ? <label htmlFor={id}>{label}</label> : null}
      {renderFormItemChildren(node, id, { ...context, fieldName })}
    </div>
  );
}

function renderFormItemChildren(
  node: BlockUiSchemaNode,
  id: string | undefined,
  context: RenderContext
): ReactNode {
  if (node.children === undefined) {
    return null;
  }

  return node.children.map((child, index) => (
    <Fragment key={child.key ?? `${child.primitive}-${index}`}>
      {renderNode(withInputId(child, id), context)}
    </Fragment>
  ));
}

function renderInput(
  node: BlockUiSchemaNode,
  props: SafeProps,
  style: CSSProperties,
  context: RenderContext
): ReactNode {
  const fieldName = context.fieldName ?? readString(props.id);
  return (
    <AntdInput
      key={node.key}
      id={readString(props.id)}
      placeholder={readString(props.placeholder)}
      type={readString(props.type)}
      value={readInputValue(readFieldValue(context, fieldName, props.value))}
      disabled={readBoolean(props.disabled)}
      readOnly={readBoolean(props.readOnly)}
      style={style}
      onChange={(event) => updateFieldValue(context, fieldName, event.target.value)}
    />
  );
}

function renderSelect(
  node: BlockUiSchemaNode,
  props: SafeProps,
  style: CSSProperties,
  context: RenderContext
): ReactNode {
  const fieldName = context.fieldName ?? readString(props.id);
  return (
    <AntdSelect
      key={node.key}
      id={readString(props.id)}
      placeholder={readString(props.placeholder)}
      value={readSelectValue(readFieldValue(context, fieldName, props.value))}
      disabled={readBoolean(props.disabled)}
      options={readSelectOptions(props.options)}
      style={{ minWidth: 160, ...style }}
      onChange={(value) => updateFieldValue(context, fieldName, value)}
    />
  );
}

function renderActionButton(
  node: BlockUiSchemaNode,
  props: SafeProps,
  style: CSSProperties,
  context: RenderContext,
  iconOnly: boolean
): ReactNode {
  const actionId = readString(props.actionId);
  const payload = props.actionPayload;
  const label = readString(props.label) ?? readString(props.children);
  const children = iconOnly ? readIconLabel(props.icon, label) : readRenderable(props.children);

  return (
    <AntdButton
      key={node.key}
      type={readButtonType(props.type)}
      danger={readBoolean(props.danger)}
      disabled={readBoolean(props.disabled)}
      aria-label={iconOnly ? label : undefined}
      style={style}
      onClick={() => {
        if (actionId === undefined || context.onAction === undefined) {
          return;
        }

        context.onAction({
          type: 'action',
          primitive: node.primitive as 'Button' | 'IconButton',
          ...(node.key === undefined ? {} : { key: node.key }),
          actionId,
          ...(payload === undefined ? {} : { payload }),
          ...(Object.keys(context.formValues).length === 0
            ? {}
            : { formValues: context.formValues })
        });
      }}
    >
      {children}
    </AntdButton>
  );
}

function readFieldValue(
  context: RenderContext,
  fieldName: string | undefined,
  fallback: unknown
): unknown {
  if (fieldName === undefined || !(fieldName in context.formValues)) {
    return fallback;
  }

  return context.formValues[fieldName];
}

function updateFieldValue(
  context: RenderContext,
  fieldName: string | undefined,
  value: unknown
): void {
  if (fieldName !== undefined) {
    context.setFormValue(fieldName, value);
  }
}

function renderModal(
  node: BlockUiSchemaNode,
  props: SafeProps,
  style: CSSProperties,
  children: ReactNode
): ReactNode {
  if (!readBoolean(props.open)) {
    return null;
  }

  const title = readString(props.title) ?? 'Dialog';
  const titleId = `${node.key ?? 'block-modal'}-title`;

  return (
    <section
      key={node.key}
      role="dialog"
      aria-modal="true"
      aria-labelledby={titleId}
      data-block-renderer-modal={node.key ?? 'modal'}
      style={{
        border: '1px solid #d5ddd8',
        borderRadius: 8,
        background: '#ffffff',
        padding: 16,
        ...style
      }}
    >
      <AntdTitle id={titleId} level={5} style={{ marginTop: 0 }}>
        {title}
      </AntdTitle>
      {children}
    </section>
  );
}

const tableCellStyle: CSSProperties = {
  border: '1px solid #e8edea',
  padding: '8px 12px',
  textAlign: 'left'
};

function getSafeProps(props: BlockUiSchemaNode['props']): SafeProps {
  if (props === undefined) {
    return {};
  }

  return props;
}

function withInputId(
  node: BlockUiSchemaNode,
  id: string | undefined
): BlockUiSchemaNode {
  if (id === undefined || !isInputLike(node.primitive)) {
    return node;
  }

  return {
    ...node,
    props: {
      ...node.props,
      id
    }
  };
}

function isInputLike(primitive: BlockUiPrimitive): boolean {
  return [
    'Input',
    'Textarea',
    'Select',
    'Checkbox',
    'Switch',
    'DatePicker',
    'NumberInput'
  ].includes(primitive);
}

function gridStyle(props: SafeProps): CSSProperties {
  const columns = readNumber(props.columns);
  if (columns === undefined) {
    return {};
  }

  return {
    gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`
  };
}

function mapStyle(style: BlockUiStyle | undefined): CSSProperties {
  if (style === undefined) {
    return {};
  }

  const output: CSSProperties = {};
  mapSpacing(style, output);
  mapColor(style, output);
  mapTypography(style, output);
  mapBorder(style, output);
  mapRadius(style, output);
  mapLayout(style, output);
  return output;
}

function mapSpacing(style: BlockUiStyle, output: CSSProperties): void {
  const spacing = style.spacing;
  if (spacing === undefined) {
    return;
  }

  assignToken(output, 'margin', spacing.margin, spacingTokens);
  assignToken(output, 'marginInline', spacing.marginX, spacingTokens);
  assignToken(output, 'marginBlock', spacing.marginY, spacingTokens);
  assignToken(output, 'marginTop', spacing.marginTop, spacingTokens);
  assignToken(output, 'marginRight', spacing.marginRight, spacingTokens);
  assignToken(output, 'marginBottom', spacing.marginBottom, spacingTokens);
  assignToken(output, 'marginLeft', spacing.marginLeft, spacingTokens);
  assignToken(output, 'padding', spacing.padding, spacingTokens);
  assignToken(output, 'paddingInline', spacing.paddingX, spacingTokens);
  assignToken(output, 'paddingBlock', spacing.paddingY, spacingTokens);
  assignToken(output, 'paddingTop', spacing.paddingTop, spacingTokens);
  assignToken(output, 'paddingRight', spacing.paddingRight, spacingTokens);
  assignToken(output, 'paddingBottom', spacing.paddingBottom, spacingTokens);
  assignToken(output, 'paddingLeft', spacing.paddingLeft, spacingTokens);
  assignToken(output, 'gap', spacing.gap, spacingTokens);
  assignToken(output, 'rowGap', spacing.rowGap, spacingTokens);
  assignToken(output, 'columnGap', spacing.columnGap, spacingTokens);
}

function mapColor(style: BlockUiStyle, output: CSSProperties): void {
  const color = style.color;
  if (color === undefined) {
    return;
  }

  assignToken(output, 'color', color.text, colorTokens);
  assignToken(output, 'backgroundColor', color.background, colorTokens);
  assignToken(output, 'borderColor', color.border, colorTokens);
}

function mapTypography(style: BlockUiStyle, output: CSSProperties): void {
  const typography = style.typography;
  if (typography === undefined) {
    return;
  }

  assignToken(output, 'fontSize', typography.fontSize, typographyTokens);
  assignToken(output, 'fontWeight', typography.fontWeight, typographyTokens);
  assignToken(output, 'lineHeight', typography.lineHeight, typographyTokens);
  assignToken(output, 'textAlign', typography.align, typographyTokens);
  if (typography.truncate === true) {
    output.overflow = 'hidden';
    output.textOverflow = 'ellipsis';
    output.whiteSpace = 'nowrap';
  }
}

function mapBorder(style: BlockUiStyle, output: CSSProperties): void {
  const border = style.border;
  if (border === undefined) {
    return;
  }

  assignToken(output, 'borderWidth', border.width, borderTokens);
  assignToken(output, 'borderStyle', border.style, borderTokens);
  assignToken(output, 'borderColor', border.color, borderTokens);
}

function mapRadius(style: BlockUiStyle, output: CSSProperties): void {
  const radius = style.radius;
  if (radius === undefined) {
    return;
  }

  assignToken(output, 'borderRadius', radius.all, radiusTokens);
  assignToken(output, 'borderTopLeftRadius', radius.top, radiusTokens);
  assignToken(output, 'borderTopRightRadius', radius.right, radiusTokens);
  assignToken(output, 'borderBottomRightRadius', radius.bottom, radiusTokens);
  assignToken(output, 'borderBottomLeftRadius', radius.left, radiusTokens);
}

function mapLayout(style: BlockUiStyle, output: CSSProperties): void {
  const layout = style.layout;
  if (layout === undefined) {
    return;
  }

  assignToken(output, 'width', layout.width, layoutTokens);
  assignToken(output, 'minWidth', layout.minWidth, layoutTokens);
  assignToken(output, 'maxWidth', layout.maxWidth, layoutTokens);
  assignToken(output, 'height', layout.height, layoutTokens);
  assignToken(output, 'minHeight', layout.minHeight, layoutTokens);
  assignToken(output, 'maxHeight', layout.maxHeight, layoutTokens);
  assignToken(output, 'overflow', layout.overflow, layoutTokens);
  assignToken(output, 'alignItems', layout.align, layoutTokens);
  assignToken(output, 'justifyContent', layout.justify, layoutTokens);
  assignToken(output, 'flexDirection', layout.direction, layoutTokens);
  assignToken(output, 'flexWrap', layout.wrap, layoutTokens);
}

function assignToken(
  output: CSSProperties,
  key: keyof CSSProperties,
  token: unknown,
  tokens: Record<string, string | number>
): void {
  if (typeof token === 'string' && token in tokens) {
    output[key] = tokens[token] as never;
  }
}

function readColumns(value: unknown): Array<{
  key: string;
  title: string;
  dataIndex: string;
}> {
  if (!Array.isArray(value)) {
    return [];
  }

  return value.flatMap((item, index) => {
    if (!isRecord(item)) {
      return [];
    }

    const dataIndex = readString(item.dataIndex) ?? readString(item.key);
    if (dataIndex === undefined) {
      return [];
    }

    return [
      {
        key: readString(item.key) ?? dataIndex ?? `column-${index}`,
        title: readString(item.title) ?? dataIndex,
        dataIndex
      }
    ];
  });
}

function readRows(value: unknown): Array<Record<string, unknown>> {
  if (!Array.isArray(value)) {
    return [];
  }

  return value.filter(isRecord);
}

function readRowKey(
  row: Record<string, unknown>,
  rowKey: string,
  index: number
): string {
  const value = row[rowKey];
  return typeof value === 'string' || typeof value === 'number'
    ? String(value)
    : `row-${index}`;
}

function readDescriptionItems(value: unknown): Array<{
  key: string;
  label: string;
  children: unknown;
}> {
  if (!Array.isArray(value)) {
    return [];
  }

  return value.flatMap((item, index) => {
    if (!isRecord(item)) {
      return [];
    }

    return [
      {
        key: readString(item.key) ?? `item-${index}`,
        label: readString(item.label) ?? '',
        children: item.children
      }
    ];
  });
}

function readSelectOptions(value: unknown): Array<{
  label: string;
  value: string | number;
}> {
  if (!Array.isArray(value)) {
    return [];
  }

  return value.flatMap((item) => {
    if (!isRecord(item)) {
      return [];
    }

    const value = readSelectValue(item.value);
    const label = readString(item.label);
    if (value === undefined || label === undefined) {
      return [];
    }

    return [{ label, value }];
  });
}

function readRenderable(value: unknown): ReactNode {
  if (
    value === null ||
    typeof value === 'string' ||
    typeof value === 'number' ||
    typeof value === 'boolean'
  ) {
    return String(value);
  }

  return null;
}

function readIconLabel(icon: unknown, label: string | undefined): ReactNode {
  const iconText = readString(icon);
  return iconText ?? label ?? null;
}

function readString(value: unknown): string | undefined {
  return typeof value === 'string' ? value : undefined;
}

function readNumber(value: unknown): number | undefined {
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined;
}

function readBoolean(value: unknown): boolean | undefined {
  return typeof value === 'boolean' ? value : undefined;
}

function readInputValue(value: unknown): string | number | readonly string[] | undefined {
  if (typeof value === 'string' || typeof value === 'number') {
    return value;
  }

  return undefined;
}

function readSelectValue(value: unknown): string | number | undefined {
  if (typeof value === 'string' || typeof value === 'number') {
    return value;
  }

  return undefined;
}

function readButtonType(value: unknown): 'primary' | 'default' | 'dashed' | 'link' | 'text' {
  return value === 'primary' ||
    value === 'dashed' ||
    value === 'link' ||
    value === 'text'
    ? value
    : 'default';
}

function readAlertType(value: unknown): 'success' | 'info' | 'warning' | 'error' {
  return value === 'success' ||
    value === 'warning' ||
    value === 'error' ||
    value === 'info'
    ? value
    : 'info';
}

function readBadgeStatus(
  value: unknown
): 'success' | 'processing' | 'default' | 'error' | 'warning' | undefined {
  return value === 'success' ||
    value === 'processing' ||
    value === 'default' ||
    value === 'error' ||
    value === 'warning'
    ? value
    : undefined;
}

function readTitleLevel(value: unknown): 1 | 2 | 3 | 4 | 5 {
  return value === 1 || value === 2 || value === 3 || value === 4 || value === 5
    ? value
    : 4;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
