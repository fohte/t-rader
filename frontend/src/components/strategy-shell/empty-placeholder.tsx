interface EmptyPlaceholderProps {
  title: string
  description: string
}

export function EmptyPlaceholder({
  title,
  description,
}: EmptyPlaceholderProps) {
  return (
    <div className="font-sans text-foreground">
      <h1 className="mb-3 text-2xl font-bold">
        <span className="font-mono font-bold text-primary">#</span> {title}
      </h1>
      <p className="max-w-160 text-sm leading-relaxed text-muted-foreground-strong">
        {description}
      </p>
      <div className="mt-6 inline-flex items-center gap-2 border border-dashed border-border px-3 py-1.5 font-mono text-2xs text-muted-foreground">
        未実装
      </div>
    </div>
  )
}
