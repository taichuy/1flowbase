import {
  Children,
  type ButtonHTMLAttributes,
  type CSSProperties,
  type InputHTMLAttributes,
  type ReactNode,
  type SVGProps
} from 'react';

/** Auth-only aliases for the exact backend template, not a general Antd facade.
 * Keep Space's item wrappers: their layout (including h2 margins) is observable.
 */
export function Space({
  children,
  direction,
  size,
  style
}: {
  children: ReactNode;
  direction: 'vertical';
  size: 'middle';
  style?: CSSProperties;
}) {
  return (
    <div
      className={`auth-password-space auth-password-space-${direction} auth-password-space-${size}`}
      style={style}
    >
      {Children.toArray(children).map((child, index) => (
        <div className="auth-password-space-item" key={index}>
          {child}
        </div>
      ))}
    </div>
  );
}

export function Input(props: InputHTMLAttributes<HTMLInputElement>) {
  // Input(type=password) intentionally remains a plain password input. The
  // backend does not use Input.Password and has no custom reveal interaction.
  return <input {...props} className="auth-password-input" />;
}

type ButtonProps = Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'type'> & {
  type: 'primary' | 'text' | 'link';
  htmlType?: ButtonHTMLAttributes<HTMLButtonElement>['type'];
  block?: boolean;
  loading?: boolean;
  icon?: ReactNode;
};

export function Button({
  type,
  htmlType = 'button',
  block,
  loading,
  icon,
  children,
  onClick,
  ...props
}: ButtonProps) {
  return (
    <button
      {...props}
      type={htmlType}
      className={`auth-password-button auth-password-button-${type}${block ? ' auth-password-button-block' : ''}${!children && icon ? ' auth-password-button-icon-only' : ''}`}
      aria-busy={loading || undefined}
      onClick={(event) => {
        // Antd suppresses clicks while loading; it does not disable the button
        // (which would apply disabled colors instead of its loading opacity).
        if (loading) {
          event.preventDefault();
          return;
        }
        onClick?.(event);
      }}
    >
      {loading ? (
        <span className="auth-password-button-icon auth-password-loading">
          <Glyph name="loading" />
        </span>
      ) : icon ? (
        <span className="auth-password-button-icon">{icon}</span>
      ) : null}
      {children ? <span>{children}</span> : null}
    </button>
  );
}

export function Alert({
  message,
  showIcon
}: {
  type: 'error';
  message: ReactNode;
  showIcon?: boolean;
}) {
  return (
    <div className="auth-password-alert" role="alert">
      {showIcon ? (
        <span className="auth-password-alert-icon">
          <Glyph name="error" />
        </span>
      ) : null}
      <div className="auth-password-alert-section">
        <div className="auth-password-alert-title">{message}</div>
      </div>
    </div>
  );
}

export function ArrowLeftOutlined(props: SVGProps<SVGSVGElement>) {
  return <Glyph {...props} name="back" />;
}

// SVG geometry from the installed @ant-design/icons-svg definitions:
// ArrowLeftOutlined, LoadingOutlined, CloseCircleFilled. No icon runtime/barrel.
const paths = {
  back: 'M872 474H286.9l350.2-304c5.6-4.9 2.2-14-5.2-14h-88.5c-3.9 0-7.6 1.4-10.5 3.9L155 487.8a31.96 31.96 0 000 48.3L535.1 866c1.5 1.3 3.3 2 5.2 2h91.5c7.4 0 10.8-9.2 5.2-14L286.9 550H872c4.4 0 8-3.6 8-8v-60c0-4.4-3.6-8-8-8z',
  loading:
    'M988 548c-19.9 0-36-16.1-36-36 0-59.4-11.6-117-34.6-171.3a440.45 440.45 0 00-94.3-139.9 437.71 437.71 0 00-139.9-94.3C629 83.6 571.4 72 512 72c-19.9 0-36-16.1-36-36s16.1-36 36-36c69.1 0 136.2 13.5 199.3 40.3C772.3 66 827 103 874 150c47 47 83.9 101.8 109.7 162.7 26.7 63.1 40.2 130.2 40.2 199.3.1 19.9-16 36-35.9 36z',
  error:
    'M512 64c247.4 0 448 200.6 448 448S759.4 960 512 960 64 759.4 64 512 264.6 64 512 64zm127.98 274.82h-.04l-.08.06L512 466.75 384.14 338.88c-.04-.05-.06-.06-.08-.06a.12.12 0 00-.07 0c-.03 0-.05.01-.09.05l-45.02 45.02a.2.2 0 00-.05.09.12.12 0 000 .07v.02a.27.27 0 00.06.06L466.75 512 338.88 639.86c-.05.04-.06.06-.06.08a.12.12 0 000 .07c0 .03.01.05.05.09l45.02 45.02a.2.2 0 00.09.05.12.12 0 00.07 0c.02 0 .04-.01.08-.05L512 557.25l127.86 127.87c.04.04.06.05.08.05a.12.12 0 00.07 0c.03 0 .05-.01.09-.05l45.02-45.02a.2.2 0 00.05-.09.12.12 0 000-.07v-.02a.27.27 0 00-.05-.06L557.25 512l127.87-127.86c.04-.04.05-.06.05-.08a.12.12 0 000-.07c0-.03-.01-.05-.05-.09l-45.02-45.02a.2.2 0 00-.09-.05.12.12 0 00-.07 0z'
};
function Glyph({
  name,
  ...props
}: SVGProps<SVGSVGElement> & { name: keyof typeof paths }) {
  return (
    <svg
      className="auth-password-icon"
      width="1em"
      height="1em"
      viewBox={name === 'loading' ? '0 0 1024 1024' : '64 64 896 896'}
      focusable="false"
      fill="currentColor"
      fillRule={name === 'error' ? 'evenodd' : undefined}
      aria-hidden="true"
      {...props}
    >
      <path d={paths[name]} />
    </svg>
  );
}
