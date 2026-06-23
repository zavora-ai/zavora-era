import { Component, type ReactNode } from 'react';
import ErrorRetry from './ErrorRetry';

interface Props {
  children: ReactNode;
  label?: string;
}
interface State {
  hasError: boolean;
}

// Class error boundary so one widget crashing doesn't take down the whole page.
export default class WidgetErrorBoundary extends Component<Props, State> {
  state: State = { hasError: false };

  static getDerivedStateFromError(): State {
    return { hasError: true };
  }

  render() {
    if (this.state.hasError) {
      return (
        <ErrorRetry
          message={`${this.props.label ?? 'This section'} couldn't be displayed.`}
          onRetry={() => this.setState({ hasError: false })}
        />
      );
    }
    return this.props.children;
  }
}
