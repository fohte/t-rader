interface EmptyPlaceholderProps {
  title: string
  description: string
}

export function EmptyPlaceholder({
  title,
  description,
}: EmptyPlaceholderProps) {
  return (
    <div className="font-sans text-[color:var(--color-text-primary)]">
      <h1 className="mb-3 text-[22px] font-bold">
        <span className="font-mono font-bold text-[color:var(--color-accent-strategy)]">
          #
        </span>{' '}
        {title}
      </h1>
      <p className="max-w-[640px] text-[14px] leading-relaxed text-[color:var(--color-text-secondary)]">
        {description}
      </p>
      <div className="mt-6 inline-flex items-center gap-2 border border-dashed border-[color:var(--color-border-strategy)] px-3 py-1.5 font-mono text-[11px] text-[color:var(--color-text-tertiary)]">
        未実装
      </div>
    </div>
  )
}
