import { createFileRoute, Link } from '@tanstack/react-router'
import { Plus, Upload } from 'lucide-react'
import { useMemo, useState } from 'react'

import {
  type StrategyFilter,
  StrategyFilterBar,
} from '@/components/trades/strategy-filter-bar'
import { TradeFormDialog } from '@/components/trades/trade-form-dialog'
import { TradeStats } from '@/components/trades/trade-stats'
import { TradesTable } from '@/components/trades/trades-table'
import { useInvalidateTrades } from '@/components/trades/use-invalidate-trades'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { $api } from '@/lib/api/client'
import type { components } from '@/lib/api/schema.gen'

type Trade = components['schemas']['Trade']

export const Route = createFileRoute('/trades')({
  component: TradesPage,
})

function TradesPage() {
  const invalidateTrades = useInvalidateTrades()
  const [filter, setFilter] = useState<StrategyFilter>('all')
  const [formOpen, setFormOpen] = useState(false)
  const [editing, setEditing] = useState<Trade | null>(null)

  const { data: trades, isPending: tradesPending } = $api.useQuery(
    'get',
    '/api/trades',
  )
  const { data: strategies = [] } = $api.useQuery('get', '/api/strategies')
  const { data: stocks = [] } = $api.useQuery('get', '/api/refs/stocks')
  const { data: summary } = $api.useQuery('get', '/api/trades/summary')

  const deleteMutation = $api.useMutation('delete', '/api/trades/{id}', {
    onSuccess: () => {
      invalidateTrades()
    },
  })

  const tradeList = trades ?? []
  const shown = useMemo(
    () =>
      filter === 'all'
        ? tradeList
        : tradeList.filter((t) => t.strategy_id === filter),
    [tradeList, filter],
  )

  const feesTotal = useMemo(
    () => tradeList.reduce((sum, t) => sum + t.fee, 0),
    [tradeList],
  )
  const openPositions = useMemo(
    () => (summary?.positions ?? []).filter((p) => p.qty > 0).length,
    [summary],
  )

  function handleAdd() {
    setEditing(null)
    setFormOpen(true)
  }
  function handleEdit(t: Trade) {
    setEditing(t)
    setFormOpen(true)
  }
  function handleDelete(t: Trade) {
    if (
      !window.confirm(`${t.date} の ${t.symbol} (${t.side}) を削除します。`)
    ) {
      return
    }
    deleteMutation.mutate({ params: { path: { id: t.id } } })
  }

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
            取引履歴
          </h1>
          <p className="max-w-[720px] text-[14px] leading-relaxed text-[color:var(--color-text-secondary)]">
            全戦略横断の約定記録。入力ソースは証券会社 API / CSV 取込 /
            手入力に非依存のモデル。成績・保有はここから導出します。
          </p>
        </div>
        <div className="flex flex-shrink-0 gap-2">
          <Button
            type="button"
            variant="outline"
            disabled
            title="MVP 後に対応予定"
          >
            <Upload />
            SBI CSV 取込
          </Button>
          <Button type="button" onClick={handleAdd}>
            <Plus />
            取引を追加
          </Button>
        </div>
      </header>

      {tradesPending ? (
        <Skeleton className="h-[88px] w-full" />
      ) : (
        <TradeStats
          realizedPnl={summary?.realized_pnl ?? 0}
          feesTotal={feesTotal}
          tradeCount={summary?.trade_count ?? tradeList.length}
          openPositions={openPositions}
        />
      )}

      <StrategyFilterBar
        trades={tradeList}
        strategies={strategies}
        value={filter}
        onChange={setFilter}
      />

      {tradesPending ? (
        <Skeleton className="h-[200px] w-full" />
      ) : (
        <TradesTable
          trades={shown}
          strategies={strategies}
          stocks={stocks}
          showStrategy={filter === 'all'}
          onEdit={handleEdit}
          onDelete={handleDelete}
        />
      )}

      <p className="font-mono text-[11px] text-[color:var(--color-text-tertiary)]">
        # SBI 証券からの自動取込は MVP 後に対応予定。それまでは CSV 取込 /
        手入力で記録します。
      </p>

      <TradeFormDialog
        open={formOpen}
        onOpenChange={(v) => {
          setFormOpen(v)
          if (!v) setEditing(null)
        }}
        initial={editing}
        strategies={strategies}
        stocks={stocks}
        defaultStrategyId={filter !== 'all' ? filter : undefined}
      />
    </div>
  )
}
