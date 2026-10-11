import { Link } from '@tanstack/react-router'

import { formatPercent } from '#components/paper-trading/format'
import { formatYen, pnlColorClass } from '#components/trades/format'
import type { components } from '#lib/api/schema.gen'

type PaperAccountStats = components['schemas']['PaperAccountStats']

export function PaperAccountStatsTable({
  stats,
  strategyNames,
}: {
  stats: PaperAccountStats[]
  strategyNames: Record<string, string>
}) {
  if (stats.length === 0) {
    return (
      <div className="border border-border bg-card px-4 py-8 text-center font-mono text-xs text-muted-foreground">
        ペーパートレード口座はありません。
      </div>
    )
  }

  return (
    <div className="overflow-x-auto border border-border bg-card">
      <table className="w-full min-w-260 font-mono text-xs">
        <thead>
          <tr className="border-b border-border text-2xs uppercase tracking-wider text-muted-foreground">
            <th className="px-3 py-2 text-left font-normal">口座</th>
            <th className="px-3 py-2 text-left font-normal">戦略</th>
            <th className="px-3 py-2 text-left font-normal">設定 (purpose)</th>
            <th className="px-3 py-2 text-left font-normal">開始日</th>
            <th className="px-3 py-2 text-right font-normal">総資産</th>
            <th className="px-3 py-2 text-right font-normal">開始来</th>
            <th className="px-3 py-2 text-right font-normal">ベンチマーク</th>
            <th className="px-3 py-2 text-right font-normal">超過</th>
            <th className="px-3 py-2 text-right font-normal">勝率</th>
            <th className="px-3 py-2 text-right font-normal">決済</th>
            <th className="px-3 py-2 text-right font-normal">含み損益</th>
          </tr>
        </thead>
        <tbody>
          {stats.map((account) => {
            const excessReturn =
              account.benchmark_return == null
                ? null
                : account.return_since_start - account.benchmark_return
            const winRate =
              account.closed_trade_count === 0 || account.win_rate == null
                ? '—'
                : formatPercent(account.win_rate)

            return (
              <tr
                key={account.account_id}
                className="border-b border-border last:border-b-0 hover:bg-surface-strong"
              >
                <td className="px-3 py-2">
                  <Link
                    to="/paper-trading/$accountId"
                    params={{ accountId: account.account_id }}
                    className="text-primary hover:underline"
                  >
                    {account.account_name}
                  </Link>
                </td>
                <td className="px-3 py-2 text-muted-foreground-strong">
                  {strategyNames[account.strategy_id] ?? account.strategy_id}
                </td>
                <td className="px-3 py-2 text-muted-foreground-strong">
                  {account.purpose}
                </td>
                <td className="px-3 py-2 tabular-nums text-muted-foreground-strong">
                  {account.started_on}
                </td>
                <td className="px-3 py-2 text-right tabular-nums">
                  {formatYen(account.total_assets_jpy)}
                </td>
                <td
                  className={
                    'px-3 py-2 text-right tabular-nums ' +
                    pnlColorClass(account.return_since_start)
                  }
                >
                  {formatPercent(account.return_since_start, true)}
                </td>
                <td className="px-3 py-2 text-right tabular-nums">
                  <div
                    className={
                      account.benchmark_return == null
                        ? 'text-muted-foreground-strong'
                        : pnlColorClass(account.benchmark_return)
                    }
                  >
                    {account.benchmark_return == null
                      ? '—'
                      : formatPercent(account.benchmark_return, true)}
                  </div>
                </td>
                <td
                  className={
                    'px-3 py-2 text-right tabular-nums ' +
                    (excessReturn == null
                      ? 'text-muted-foreground-strong'
                      : pnlColorClass(excessReturn))
                  }
                >
                  {excessReturn == null
                    ? '—'
                    : formatPercent(excessReturn, true)}
                </td>
                <td className="px-3 py-2 text-right tabular-nums">{winRate}</td>
                <td className="px-3 py-2 text-right tabular-nums">
                  {account.closed_trade_count.toLocaleString()}
                </td>
                <td
                  className={
                    'px-3 py-2 text-right tabular-nums ' +
                    pnlColorClass(account.unrealized_pnl_jpy)
                  }
                >
                  {formatYen(account.unrealized_pnl_jpy, true)}
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>
    </div>
  )
}
