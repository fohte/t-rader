import {
  formatOptionalPercent,
  optionalPnlColorClass,
} from '#components/trades/format'

export { formatOptionalPercent, optionalPnlColorClass }

const percentFormatter = new Intl.NumberFormat('ja-JP', {
  style: 'percent',
  minimumFractionDigits: 1,
  maximumFractionDigits: 1,
})

export function formatPercent(value: number, signed = false): string {
  const formatted = percentFormatter.format(Math.abs(value))
  if (!signed || value === 0) return formatted
  return (value > 0 ? '+' : '−') + formatted
}

export function formatWinRate(
  closedTradeCount: number,
  winRate: number | null | undefined,
): string {
  return closedTradeCount === 0 ? '—' : formatOptionalPercent(winRate)
}

export function formatDateTime(value: string): string {
  return new Intl.DateTimeFormat('sv-SE', {
    timeZone: 'Asia/Tokyo',
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    hourCycle: 'h23',
  }).format(new Date(value))
}
