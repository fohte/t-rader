import { formatYen } from '#components/trades/format'

export interface AllocationSegment {
  key: string
  label: string
  value: number
  kind: 'position' | 'cash'
}

const POSITION_HUES = [350, 200, 35, 280, 140, 25, 220]
const CASH_BG = 'hsl(0, 0%, 35%)'

function segmentBg(seg: AllocationSegment, idx: number): string {
  if (seg.kind === 'cash') return CASH_BG
  const hue = POSITION_HUES[idx % POSITION_HUES.length] ?? 200
  return `hsl(${String(hue)}, 55%, 45%)`
}

export function AllocationBar({ segments }: { segments: AllocationSegment[] }) {
  const total = segments.reduce((s, seg) => s + Math.max(seg.value, 0), 0)
  if (total <= 0) {
    return (
      <div className="border border-border bg-card px-4 py-8 text-center font-mono text-xs text-muted-foreground">
        資産がありません
      </div>
    )
  }

  return (
    <div className="space-y-3">
      <div className="flex h-3 w-full overflow-hidden border border-border">
        {segments.map((seg, i) => {
          const v = Math.max(seg.value, 0)
          if (v === 0) return null
          return (
            <div
              key={seg.key}
              style={{ flex: v, background: segmentBg(seg, i) }}
              title={`${seg.label} ${formatYen(seg.value)}`}
            />
          )
        })}
      </div>
      <ul className="space-y-1 font-mono text-xs">
        {segments.map((seg, i) => {
          const v = Math.max(seg.value, 0)
          const pct = (v / total) * 100
          return (
            <li
              key={seg.key}
              className="flex items-center gap-2 text-muted-foreground-strong"
            >
              <span
                className="inline-block h-2.5 w-2.5 border border-border"
                style={{ background: segmentBg(seg, i) }}
              />
              <span className="min-w-0 flex-1 truncate text-foreground">
                {seg.label}
              </span>
              <span className="tabular-nums text-muted-foreground-strong">
                {formatYen(seg.value)}
              </span>
              <span className="w-[52px] text-right tabular-nums text-muted-foreground">
                {pct.toFixed(1)}%
              </span>
            </li>
          )
        })}
      </ul>
    </div>
  )
}
