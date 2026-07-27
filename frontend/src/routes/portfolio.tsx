import { createFileRoute, Link } from '@tanstack/react-router'
import { Pencil } from 'lucide-react'
import { useMemo, useState } from 'react'

import {
  AllocationBar,
  type AllocationSegment,
} from '#components/portfolio/allocation-bar'
import { CashBalanceDialog } from '#components/portfolio/cash-balance-dialog'
import { PositionsTable } from '#components/portfolio/positions-table'
import { type StatItem, StatRow } from '#components/portfolio/stat-row'
import { useCashBalance } from '#components/portfolio/use-cash-balance'
import { formatYen, pnlColorClass } from '#components/trades/format'
import { Button } from '#components/ui/button'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/portfolio')({
  component: PortfolioPage,
})

function PortfolioPage() {
  const { cash, setCash } = useCashBalance()
  const [cashOpen, setCashOpen] = useState(false)

  const { data: summary, isPending: summaryPending } = $api.useQuery(
    'get',
    '/api/trades/summary',
  )
  const { data: stocks = [] } = $api.useQuery('get', '/api/refs/stocks')

  const openPositions = useMemo(
    () => (summary?.positions ?? []).filter((p) => p.qty > 0),
    [summary],
  )

  const equity = openPositions.reduce((s, p) => s + p.cost_basis, 0)
  const totalAssets = equity + cash
  const cashRatio = totalAssets > 0 ? (cash / totalAssets) * 100 : 0
  const investedRatio = totalAssets > 0 ? 100 - cashRatio : 0
  const realizedPnl = summary?.realized_pnl ?? 0

  const stockNameById = useMemo(
    () => new Map(stocks.map((s) => [s.id, s.name])),
    [stocks],
  )

  const allocation: AllocationSegment[] = useMemo(() => {
    const segs: AllocationSegment[] = [...openPositions]
      .sort((a, b) => b.cost_basis - a.cost_basis)
      .map((p) => ({
        key: `pos:${p.symbol}`,
        label: stockNameById.get(p.symbol) ?? p.symbol,
        value: p.cost_basis,
        kind: 'position' as const,
      }))
    segs.push({ key: 'cash', label: '現金', value: cash, kind: 'cash' })
    return segs
  }, [openPositions, cash, stockNameById])

  const stats: StatItem[] = [
    { label: '総資産', value: formatYen(totalAssets) },
    { label: '評価額 (株式・簿価)', value: formatYen(equity) },
    {
      label: '現金',
      value: formatYen(cash),
      sub: `${cashRatio.toFixed(1)}% 比率`,
    },
    {
      label: '実現損益 (累計)',
      value: formatYen(realizedPnl, true),
      cls: pnlColorClass(realizedPnl),
    },
    {
      label: '保有銘柄',
      value: openPositions.length.toLocaleString(),
    },
  ]

  return (
    <div className="space-y-5 font-sans text-[color:var(--color-text-primary)]">
      <div>
        <Link
          to="/strategies"
          className="font-mono text-[12px] text-[color:var(--color-text-tertiary)] hover:text-[color:var(--color-text-primary)]"
        >
          &lt; 戦略一覧に戻る
        </Link>
      </div>

      <header className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
        <div className="min-w-0 flex-1">
          <h1 className="mb-1.5 text-[24px] font-bold leading-tight tracking-tight">
            ポートフォリオ
          </h1>
          <p className="max-w-[720px] text-[14px] leading-relaxed text-[color:var(--color-text-secondary)]">
            戦略横断の全体ビュー。現金比率・全保有・全体損益を把握します。LLM
            もこのコンテキストを参照します。
          </p>
        </div>
        <Button
          type="button"
          variant="outline"
          onClick={() => {
            setCashOpen(true)
          }}
        >
          <Pencil />
          現金残高を更新
        </Button>
      </header>

      {summaryPending ? (
        <Skeleton className="h-[88px] w-full" />
      ) : (
        <StatRow stats={stats} />
      )}

      <div className="grid grid-cols-1 gap-5 lg:grid-cols-[minmax(0,1fr)_360px]">
        <section className="space-y-2">
          <div className="flex items-baseline justify-between">
            <h2 className="font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
              <span className="mr-2 text-[color:var(--color-accent-strategy)]">
                &gt;
              </span>
              保有銘柄
            </h2>
            <span className="font-mono text-[11px] text-[color:var(--color-text-tertiary)]">
              {openPositions.length} 銘柄
            </span>
          </div>
          {summaryPending ? (
            <Skeleton className="h-[160px] w-full" />
          ) : (
            <PositionsTable positions={openPositions} stocks={stocks} />
          )}
        </section>

        <section className="space-y-2">
          <div className="flex items-baseline justify-between">
            <h2 className="font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
              <span className="mr-2 text-[color:var(--color-accent-strategy)]">
                &gt;
              </span>
              アロケーション
            </h2>
            <span className="font-mono text-[11px] text-[color:var(--color-text-tertiary)]">
              現金 {cashRatio.toFixed(1)}% / 投資 {investedRatio.toFixed(1)}%
            </span>
          </div>
          <div className="border border-[color:var(--color-border-strategy)] bg-[color:var(--panel)] p-4">
            <AllocationBar segments={allocation} />
          </div>
        </section>
      </div>

      <p className="font-mono text-[11px] text-[color:var(--color-text-tertiary)]">
        # 現在値の取得は未対応のため、評価額・アロケーションは取得簿価
        (cost_basis) で計算しています。
      </p>

      <div>
        <Link
          to="/trades"
          className="font-mono text-[13px] text-[color:var(--color-accent-strategy)] hover:underline"
        >
          取引履歴をすべて見る →
        </Link>
      </div>

      <CashBalanceDialog
        open={cashOpen}
        initial={cash}
        onOpenChange={setCashOpen}
        onSave={setCash}
      />
    </div>
  )
}
