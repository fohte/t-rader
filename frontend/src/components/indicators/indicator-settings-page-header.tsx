import { Link } from '@tanstack/react-router'

export function IndicatorSettingsPageHeader({
  scope,
}: {
  scope: 'global' | 'strategy'
}) {
  if (scope === 'strategy') {
    return (
      <header className="flex flex-wrap items-center justify-between gap-3 border-b border-border pb-2">
        <p className="font-mono text-sm text-muted-foreground-strong">
          <span className="mr-1 text-primary">&gt;</span>
          この戦略だけの指標
        </p>
        <Link
          to="/settings/indicators"
          className="font-mono text-xs text-muted-foreground hover:text-foreground"
        >
          共通の指標 (全体の設定) →
        </Link>
      </header>
    )
  }

  return (
    <div className="space-y-2">
      <div>
        <Link
          to="/settings"
          className="font-mono text-xs text-muted-foreground hover:text-foreground"
        >
          &lt; 設定に戻る
        </Link>
      </div>
      <header>
        <h1 className="mb-1 text-2xl font-bold leading-tight tracking-tight">
          設定 — 自作指標 (共通)
        </h1>
        <p className="text-sm text-muted-foreground-strong">
          全体の設定に登録した自作指標は、すべての戦略から参照できます。
        </p>
      </header>
    </div>
  )
}
