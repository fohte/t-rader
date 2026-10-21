import { createFileRoute } from '@tanstack/react-router'
import { useMemo, useState } from 'react'

import { AssetTabs } from '#components/assets/asset-tabs'
import { ImportSbiDialog } from '#components/trades/import-sbi-dialog'
import {
  type StrategyFilter,
  StrategyFilterBar,
} from '#components/trades/strategy-filter-bar'
import { TradeDeleteError } from '#components/trades/trade-delete-error'
import { TradeFormDialog } from '#components/trades/trade-form-dialog'
import { TradeNotesDialog } from '#components/trades/trade-notes-dialog'
import { TradeStats } from '#components/trades/trade-stats'
import { TradesPageHeader } from '#components/trades/trades-page-header'
import { TradesPageLoadingState } from '#components/trades/trades-page-loading-state'
import { TradesTable } from '#components/trades/trades-table'
import { useInvalidateTrades } from '#components/trades/use-invalidate-trades'
import { $api } from '#lib/api/client'
import type { components } from '#lib/api/schema.gen'

type Trade = components['schemas']['TradeListItem']

export const Route = createFileRoute('/trades/')({
  component: TradesPage,
})

function TradesPage() {
  const invalidateTrades = useInvalidateTrades()
  const [filter, setFilter] = useState<StrategyFilter>('all')
  const [onlyUnlinked, setOnlyUnlinked] = useState(false)
  const [formOpen, setFormOpen] = useState(false)
  const [importOpen, setImportOpen] = useState(false)
  const [editing, setEditing] = useState<Trade | null>(null)
  const [notesTrade, setNotesTrade] = useState<Trade | null>(null)

  const { data: trades, isPending: tradesPending } = $api.useQuery(
    'get',
    '/api/trades',
  )
  const { data: strategies = [] } = $api.useQuery('get', '/api/strategies')
  const { data: stocks = [] } = $api.useQuery('get', '/api/refs/stocks')
  const { data: summary } = $api.useQuery('get', '/api/trades/summary')

  const [deleteError, setDeleteError] = useState<string | null>(null)
  const deleteMutation = $api.useMutation('delete', '/api/trades/{id}', {
    onSuccess: () => {
      invalidateTrades()
    },
    onError: () => {
      setDeleteError('取引の削除に失敗しました')
    },
  })

  const tradeList = trades ?? []
  const strategyTrades = useMemo(
    () =>
      filter === 'all'
        ? tradeList
        : tradeList.filter((trade) => trade.strategy_id === filter),
    [tradeList, filter],
  )
  const unlinkedTrades = useMemo(
    () => strategyTrades.filter((trade) => trade.note_count === 0),
    [strategyTrades],
  )
  const shown = onlyUnlinked ? unlinkedTrades : strategyTrades

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
    setDeleteError(null)
    deleteMutation.mutate({ params: { path: { id: t.id } } })
  }

  return (
    <div className="space-y-5 font-sans text-foreground">
      <AssetTabs activeTab="trades" />

      <TradesPageHeader
        onImport={() => {
          setImportOpen(true)
        }}
        onAdd={handleAdd}
      />

      {tradesPending ? (
        <TradesPageLoadingState kind="summary" />
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
        unlinkedCount={unlinkedTrades.length}
        onlyUnlinked={onlyUnlinked}
        onOnlyUnlinkedChange={setOnlyUnlinked}
      />

      {deleteError != null && <TradeDeleteError message={deleteError} />}

      {tradesPending ? (
        <TradesPageLoadingState kind="table" />
      ) : (
        <TradesTable
          trades={shown}
          strategies={strategies}
          stocks={stocks}
          showStrategy={filter === 'all'}
          onEdit={handleEdit}
          onDelete={handleDelete}
          onManageNotes={setNotesTrade}
          emptyMessage={
            onlyUnlinked ? '未紐付けの取引はありません。' : undefined
          }
        />
      )}

      <ImportSbiDialog
        open={importOpen}
        onOpenChange={setImportOpen}
        strategies={strategies}
      />

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

      <TradeNotesDialog
        open={notesTrade != null}
        onOpenChange={(open) => {
          if (!open) setNotesTrade(null)
        }}
        trade={notesTrade}
      />
    </div>
  )
}
