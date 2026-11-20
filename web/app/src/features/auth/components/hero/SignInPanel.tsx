import React, { type PropsWithChildren } from 'react';

export function SignInPanel({
  children,
  inert,
  footer,
  filing_html
}: PropsWithChildren<{
  inert?: boolean;
  footer: string;
  filing_html?: string;
}>) {
  return (
    <div
      className="auth-sign-in-panel"
      inert={inert}
      style={{
        display: 'flex',
        flexDirection: 'column',
        justifyContent: 'center',
        position: 'relative',
        zIndex: 10
      }}
    >
      <div className="auth-sign-in-content">{children}</div>
      <div style={{ textAlign: 'center', marginTop: 48 }}>
        <span className="auth-sign-in-footer">
          <a
            href="https://1flowbase.taichuy.com/"
            target="_blank"
            rel="noreferrer"
          >
            {footer}
          </a>
        </span>
        {filing_html ? (
          <div
            className="auth-sign-in-filing"
            dangerouslySetInnerHTML={{ __html: filing_html }}
          />
        ) : (
          <div data-opening-filing="" hidden />
        )}
      </div>
    </div>
  );
}
