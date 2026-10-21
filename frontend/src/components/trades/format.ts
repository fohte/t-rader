const intFmt = new Intl.NumberFormat('en-US')
const percentFormatter = new Intl.NumberFormat('ja-JP', {
  style: 'percent',
  minimumFractionDigits: 1,
  maximumFractionDigits: 1,
})

export function formatYen(value: number, signed = false): string {
  const rounded = Math.round(value)
  const abs = intFmt.format(Math.abs(rounded))
  if (signed) {
    if (rounded > 0) return `+¥${abs}`
    if (rounded < 0) return `−¥${abs}`
    return `¥${abs}`
  }
  return rounded < 0 ? `−¥${abs}` : `¥${abs}`
}

export type PnlTone = 'positive' | 'negative' | 'muted'

const pnlClassByTone: Record<PnlTone, string> = {
  positive: 'text-up',
  negative: 'text-down',
  muted: 'text-muted-foreground-strong',
}

export function pnlTone(value: number): PnlTone {
  if (value > 0) return 'positive'
  if (value < 0) return 'negative'
  return 'muted'
}

export function pnlColorClass(value: number): string {
  return pnlClassByTone[pnlTone(value)]
}

export function formatOptionalPercent(
  value: number | null | undefined,
  signed = false,
): string {
  if (value == null) return '—'

  const formatted = percentFormatter.format(Math.abs(value))
  if (!signed || value === 0) return formatted
  return (value > 0 ? '+' : '−') + formatted
}

export function optionalPnlColorClass(
  value: number | null | undefined,
): string {
  return value == null ? 'text-muted-foreground-strong' : pnlColorClass(value)
}

export const SOURCE_LABEL: Record<string, string> = {
  manual: '手入力',
  csv: 'CSV 取込',
  api: 'API',
}
