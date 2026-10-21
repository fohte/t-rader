import { Link } from '@tanstack/react-router'

export function StrategyRunsHeading() {
  return (
    <header className="mb-2 flex items-baseline justify-between gap-3 font-mono text-2xs tracking-wider">
      <h1 className="flex items-baseline gap-2 text-muted-foreground">
        <span aria-hidden="true" className="text-foreground">
          &gt;
        </span>
        <span>この戦略の実行</span>
      </h1>
      <Link
        to="/strategies"
        search={{ strategyTab: 'runs' }}
        className="shrink-0 text-muted-foreground-strong underline decoration-border underline-offset-4 hover:text-foreground hover:decoration-foreground"
      >
        全戦略の実行 <span aria-hidden="true">→</span>
      </Link>
    </header>
  )
}
