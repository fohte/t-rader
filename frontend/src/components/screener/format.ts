export function formatScreenerMetric(
  value: number | null | undefined,
  unit: string,
): string {
  return value == null
    ? '—'
    : `${value.toLocaleString('ja-JP', { maximumFractionDigits: 2 })}${unit}`
}
