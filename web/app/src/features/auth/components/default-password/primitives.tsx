import {
  Children,
  type ButtonHTMLAttributes,
  type CSSProperties,
  type InputHTMLAttributes,
  type ReactNode,
  type SVGProps
} from 'react';
import {
  AUTH_ERROR_ICON_PATH,
  AUTH_LOADING_ICON_PATH
} from '../anonymous-loading/AnonymousLoadingView';

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

const paths = {
  back: 'M872 474H286.9l350.2-304c5.6-4.9 2.2-14-5.2-14h-88.5c-3.9 0-7.6 1.4-10.5 3.9L155 487.8a31.96 31.96 0 000 48.3L535.1 866c1.5 1.3 3.3 2 5.2 2h91.5c7.4 0 10.8-9.2 5.2-14L286.9 550H872c4.4 0 8-3.6 8-8v-60c0-4.4-3.6-8-8-8z',
  loading: AUTH_LOADING_ICON_PATH,
  error: AUTH_ERROR_ICON_PATH
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
