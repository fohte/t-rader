export type Trigger = string

const GLYPH: Record<string, string> = {
  cron: '⏱',
  hook: '⚓',
  'on-demand': '>_',
  manual: '·',
}

interface TriggerTagProps {
  trigger?: Trigger | null
  label?: string | null
  className?: string
}

export function TriggerTag({
  trigger,
  label,
  className = '',
}: TriggerTagProps) {
  if (trigger == null) return null
  const glyph = GLYPH[trigger] ?? '·'
  return (
    <span
      className={`inline-flex items-center gap-1 font-mono text-[10px] tracking-wide text-muted-foreground ${className}`}
      title={trigger}
    >
      <span className="text-primary">{glyph}</span>
      <span>{label ?? trigger}</span>
    </span>
  )
}
