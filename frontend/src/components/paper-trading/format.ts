import {
  formatOptionalPercent,
  optionalPnlColorClass,
} from '#components/trades/format'
import { formatPercentValue } from '#lib/percent-format'

export { formatOptionalPercent, optionalPnlColorClass }

export function formatPercent(value: number, signed = false): string {
  return formatPercentValue(value, { signed })
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
