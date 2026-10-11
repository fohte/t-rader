import { PaperAccountStatsTable } from '#components/paper-trading/account-stats-table'
import type { components } from '#lib/api/schema.gen'

type PaperAccountStats = components['schemas']['PaperAccountStats']

export function PaperTradingAccountsView({
  stats,
  strategyNames,
}: {
  stats: PaperAccountStats[]
  strategyNames: Record<string, string>
}) {
  return (
    <div className="space-y-5 font-sans text-foreground">
      <header>
        <h1 className="mb-2 text-2xl font-bold leading-tight tracking-tight">
          ペーパートレード
        </h1>
      </header>
      <PaperAccountStatsTable stats={stats} strategyNames={strategyNames} />
    </div>
  )
}
