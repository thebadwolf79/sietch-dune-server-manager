export type CommandFailure = {
  message: string;
  stdout?: string;
  stderr?: string;
  code?: number;
};
