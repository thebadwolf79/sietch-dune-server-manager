export type BusySpinnerProps = Record<string, never>;

export default function BusySpinner(_props: BusySpinnerProps) {
  return <span className="inline-spinner" aria-hidden />;
}
