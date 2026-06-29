import { Component, ReactNode } from "react";

type ErrorBoundaryProps = {
  children: ReactNode;
  resetKey?: string;
};

type ErrorBoundaryState = {
  error: Error | null;
};

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidUpdate(previousProps: ErrorBoundaryProps) {
    if (previousProps.resetKey !== this.props.resetKey && this.state.error) {
      this.setState({ error: null });
    }
  }

  render() {
    if (!this.state.error) return this.props.children;

    return (
      <section className="empty-state" role="alert">
        <strong>Modulo no disponible</strong>
        <span>{this.state.error.message || "Error inesperado en interfaz"}</span>
        <button className="primary-button" type="button" onClick={() => this.setState({ error: null })}>
          Reintentar
        </button>
      </section>
    );
  }
}
