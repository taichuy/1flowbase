import {
  Component,
  type ErrorInfo,
  type PropsWithChildren,
  type ReactNode
} from 'react';

type ApplicationBootBoundaryState = {
  error: Error | null;
};

class ApplicationBootBoundary extends Component<
  PropsWithChildren,
  ApplicationBootBoundaryState
> {
  state: ApplicationBootBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): ApplicationBootBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    if (import.meta.env.DEV) {
      console.error('[1flowbase-dev-runtime] application boot failed', {
        error,
        componentStack: errorInfo.componentStack
      });
    }
  }

  private retry = () => {
    window.location.reload();
  };

  render(): ReactNode {
    if (!this.state.error) return this.props.children;

    return (
      <div className="application-bootstrap-failure" role="alert">
        <strong>应用模块加载失败</strong>
        <span>开发依赖图已失效，请重新加载当前 generation。</span>
        <button type="button" onClick={this.retry}>
          重新加载
        </button>
      </div>
    );
  }
}

export { ApplicationBootBoundary };
