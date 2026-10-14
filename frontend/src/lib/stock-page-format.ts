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
