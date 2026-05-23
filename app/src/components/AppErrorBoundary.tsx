import { Component, type ErrorInfo, type ReactNode } from "react";
import { Card, Flex, Heading, Text } from "@radix-ui/themes";

export type AppErrorBoundaryProps = {
  onError: (message: string) => void;
  children: ReactNode;
};

type AppErrorBoundaryState = {
  error: string | null;
};

export default class AppErrorBoundary extends Component<AppErrorBoundaryProps, AppErrorBoundaryState> {
  state: AppErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): AppErrorBoundaryState {
    return { error: error.message };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    this.props.onError(`${error.message}\n${info.componentStack}`);
  }

  render() {
    if (this.state.error) {
      return (
        <Card size="3" variant="surface" className="pane page-pane">
          <Flex direction="column" gap="3">
            <Heading size="4">UI Error</Heading>
            <Text size="2" color="gray">
              The view failed to render. Details were written to the log window.
            </Text>
            <Text size="2" className="mono">
              {this.state.error}
            </Text>
          </Flex>
        </Card>
      );
    }

    return this.props.children;
  }
}
