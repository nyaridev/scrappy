// ViewError: catches a view that throws while drawing, so it shows the error in its place instead of the whole window
// going blank; the other views keep working.

import { Button, EmptyState } from "iro-ui";
import { Component, type ReactNode } from "react";

// ── Public API ────────────────────────────
/** Shows the error a view threw, with a button to draw it again. */
export class ViewError extends Component<{ children: ReactNode }, { error: Error | null }> {
  state = { error: null as Error | null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  render() {
    const { error } = this.state;
    if (!error) return this.props.children;
    return (
      <EmptyState
        tone="warning"
        title="This view failed"
        action={<Button onClick={() => this.setState({ error: null })}>Try again</Button>}
      >
        <span className="selectable">{error.message}</span>
      </EmptyState>
    );
  }
}
