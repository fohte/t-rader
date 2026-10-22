export function StrategyListCount({ count }: { count: number }) {
  return (
    <div className="mb-2.5 flex items-baseline gap-2 font-mono text-2xs uppercase tracking-wider text-muted-foreground">
      <span className="text-muted-foreground-strong">&gt;</span>
      <span>strategies</span>
      <span className="text-muted-foreground-strong">{count} 件</span>
    </div>
  )
}
