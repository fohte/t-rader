export type ReviewStatus = string

const STATUS_LABEL: Record<string, string> = {
  approved: '承認済み',
  unread: '未レビュー',
  rejected: '却下',
}

const STATUS_COLOR: Record<string, string> = {
  approved: 'var(--color-status-approved)',
  unread: 'var(--color-status-unread)',
  rejected: 'var(--color-status-rejected)',
}

interface StatusPillProps {
  status: ReviewStatus
  className?: string
}

export function StatusPill({ status, className = '' }: StatusPillProps) {
  const label = STATUS_LABEL[status] ?? status
  const color = STATUS_COLOR[status] ?? 'var(--color-text-tertiary)'
  return (
    <span
      className={`inline-flex items-center gap-1.5 border border-[color:var(--color-border-strategy)] px-1.5 py-px font-mono text-[10px] text-[color:var(--color-text-secondary)] ${className}`}
    >
      <span
        className="inline-block size-1.5 rounded-full"
        style={{ background: color }}
      />
      {label}
    </span>
  )
}
