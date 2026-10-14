export function formatDecimal(value: number | null | undefined): string {
  return value == null
    ? '—'
    : value.toLocaleString('ja-JP', { maximumFractionDigits: 2 })
}

export function formatRatioAsPercent(value: number | null | undefined): string {
  return value == null ? '—' : `${formatDecimal(value * 100)}%`
}

export function formatYenInHundredMillions(
  value: number | null | undefined,
): string {
  if (value == null) return '—'
  if (Math.abs(value) < 100_000_000) {
    return `${value.toLocaleString('ja-JP', { maximumFractionDigits: 0 })} 円`
  }

  return `${(value / 100_000_000).toLocaleString('ja-JP', {
    maximumFractionDigits: 1,
  })} 億円`
}

export function formatMillionYenInHundredMillions(
  value: number | null | undefined,
): string {
  return value == null ? '—' : formatYenInHundredMillions(value * 1_000_000)
}
