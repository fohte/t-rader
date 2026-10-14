import {
  allocationTotal,
  positiveAllocationValue,
} from '#components/portfolio/allocation'
import { formatYen } from '#components/trades/format'

export interface AllocationSegment {
  key: string
  label: string
  value: number | null
  kind: 'position' | 'cash'
}

const POSITION_HUES = [350, 200, 35, 280, 140, 25, 220]
const CASH_BG = 'hsl(0, 0%, 35%)'

function segmentBg(seg: AllocationSegment, idx: number): string {
  if (seg.kind === 'cash') return CASH_BG
  const hue = POSITION_HUES[idx % POSITION_HUES.length] ?? 200
  return `hsl(${String(hue)}, 55%, 45%)`
}

export function AllocationBar({
  segments,
  priceObservedAt,
}: {
  segments: AllocationSegment[]
  priceObservedAt?: string | null
}) {
  const total = allocationTotal(segments.map((segment) => segment.value))
  const hasUnavailableValue = segments.some((segment) => segment.value == null)

  return (
    <div className="space-y-3">
      {total > 0 ? (
        <div className="flex h-3 w-full overflow-hidden border border-border">
          {segments.map((segment, index) => {
            const value = positiveAllocationValue(segment.value)
            if (value === 0) return null
            return (
              <div
                key={segment.key}
                style={{ flex: value, background: segmentBg(segment, index) }}
                title={`${segment.label} ${formatYen(value)}`}
              />
            )
          })}
        </div>
      ) : (
        <div className="border border-border bg-card px-4 py-8 text-center font-mono text-xs text-muted-foreground">
          {hasUnavailableValue
            ? '時価を取得できる資産がありません'
            : '資産がありません'}
        </div>
      )}
      {segments.length > 0 && (
        <ul className="space-y-1 font-mono text-xs">
          {segments.map((segment, index) => {
            const value =
              segment.value == null
                ? null
                : positiveAllocationValue(segment.value)
            const percentage =
              value == null || total <= 0 ? null : (value / total) * 100
            return (
              <li
                key={segment.key}
                className="flex items-center gap-2 text-muted-foreground-strong"
              >
                <span
                  className="inline-block h-2.5 w-2.5 border border-border"
                  style={{ background: segmentBg(segment, index) }}
                />
                <span className="min-w-0 flex-1 truncate text-foreground">
                  {segment.label}
                </span>
                <span className="tabular-nums text-muted-foreground-strong">
                  {value == null ? '—' : formatYen(segment.value ?? 0)}
                </span>
                <span className="w-13 text-right tabular-nums text-muted-foreground">
                  {percentage == null ? '—' : `${percentage.toFixed(1)}%`}
                </span>
              </li>
            )
          })}
        </ul>
      )}
      {hasUnavailableValue && (
        <p className="font-mono text-2xs text-muted-foreground">
          時価を取得できない銘柄は配分に含まれていません。
        </p>
      )}
      {priceObservedAt !== undefined && (
        <p className="font-mono text-2xs text-muted-foreground">
          価格観測日: {priceObservedAt ?? '—'}
        </p>
      )}
    </div>
  )
}
