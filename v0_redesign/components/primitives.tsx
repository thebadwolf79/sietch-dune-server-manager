import { cn } from "@/lib/utils"

export function SectionLabel({
  children,
  className,
}: {
  children: React.ReactNode
  className?: string
}) {
  return (
    <p
      className={cn(
        "font-display text-[11px] text-muted-foreground",
        className,
      )}
    >
      {children}
    </p>
  )
}

export function Panel({
  children,
  className,
}: {
  children: React.ReactNode
  className?: string
}) {
  return (
    <div
      className={cn(
        "chamfer border border-border bg-card",
        className,
      )}
    >
      {children}
    </div>
  )
}

export function ViewContainer({ children }: { children: React.ReactNode }) {
  return (
    <div className="mx-auto w-full max-w-6xl px-4 py-6 md:px-6 lg:py-8">
      {children}
    </div>
  )
}
