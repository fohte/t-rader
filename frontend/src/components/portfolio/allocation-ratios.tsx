export function AllocationRatios({
  cashRatio,
  investedRatio,
}: {
  cashRatio: number | null
  investedRatio: number | null
}) {
  return (
    <span className="font-mono text-2xs text-muted-foreground">
      現金 {cashRatio == null ? '—' : `${cashRatio.toFixed(1)}%`} / 投資{' '}
      {investedRatio == null ? '—' : `${investedRatio.toFixed(1)}%`}
    </span>
  )
}
