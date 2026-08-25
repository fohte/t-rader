export const HYPOTHESIS_STATUSES = [
  'unverified',
  'supported',
  'refuted',
  'obsolete',
] as const

export const HYPOTHESIS_STATUS_LABEL: Record<string, string> = {
  unverified: '未検証',
  supported: '支持',
  refuted: '反証',
  obsolete: '廃止',
}

const HYPOTHESIS_STATUS_COLOR: Record<string, string> = {
  unverified: 'var(--color-status-unread)',
  supported: 'var(--color-status-approved)',
  refuted: 'var(--color-down)',
  obsolete: 'var(--color-text-tertiary)',
}

interface HypothesisStatusPillProps {
  status: string
  className?: string
}

export function HypothesisStatusPill({
  status,
  className = '',
}: HypothesisStatusPillProps) {
  const label = HYPOTHESIS_STATUS_LABEL[status] ?? status
  const color = HYPOTHESIS_STATUS_COLOR[status] ?? 'var(--color-text-tertiary)'
  return (
    <span
      data-testid="hypothesis-status-pill"
      className={`inline-flex items-center gap-1.5 border border-border px-1.5 py-px font-mono text-[10px] text-muted-foreground-strong ${className}`}
    >
      <span
        className="inline-block size-1.5 rounded-full"
        style={{ background: color }}
      />
      {label}
    </span>
  )
}
