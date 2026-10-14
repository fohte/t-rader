import { Button } from '@fohte/ui/button'
import { Link } from '@tanstack/react-router'

import { StockNameLink } from '#components/stocks/stock-name-link'
import { formatYen, pnlColorClass } from '#components/trades/format'
import { TradeDetailChartView } from '#components/trades/trade-detail-chart-view'
import type { TradeDetailChartState } from '#components/trades/trade-detail-model'
import { Skeleton } from '#components/ui/skeleton'
import type { components } from '#lib/api/schema.gen'
import type { ChartCurrency } from '#lib/chart-utils'

type TradeDetail = components['schemas']['TradeDetailResponse']
type Note = components['schemas']['Note']

type TradeDetailViewProps =
  | { state: 'loading' }
  | { state: 'error' }
  | {
      state: 'ready'
      trade: TradeDetail
      chartCurrency?: ChartCurrency
      chartState: TradeDetailChartState
      stockName: string | null
      strategyName: string | null
      linkedNotes: Note[]
      notesPending: boolean
      notesError: boolean
      noteLinkError: string | null
      onManageNotes: () => void
      onWriteReflection: () => void
    }

export function TradeDetailView(props: TradeDetailViewProps) {
  if (props.state === 'loading') {
    return (
      <div className="space-y-5" aria-label="取引詳細を読み込み中">
        <DetailBackLink />
        <Skeleton className="h-10 w-64" />
        <Skeleton className="h-28 w-full" />
        <Skeleton className="h-40 w-full" />
      </div>
    )
  }

  if (props.state === 'error') {
    return (
      <div className="space-y-5">
        <DetailBackLink />
        <p
          role="alert"
          className="border border-border bg-card px-4 py-5 text-sm"
        >
          取引を読み込めませんでした。
        </p>
      </div>
    )
  }

  const { trade } = props
  const isBuy = trade.side === 'buy'
  const matchedBuys = trade.matched_buys ?? []

  return (
    <div className="space-y-5 font-sans text-foreground">
      <DetailBackLink />

      <header className="space-y-1">
        <h1 className="flex flex-wrap items-baseline gap-x-2 text-2xl font-bold leading-tight tracking-tight">
          <StockNameLink
            name={props.stockName}
            symbol={trade.symbol}
            missingName="link-symbol"
          />
          <span>の{isBuy ? '買い' : '売り'}</span>
        </h1>
        <p className="text-sm text-muted-foreground-strong">
          {trade.date} · {props.strategyName ?? '戦略不明'}
        </p>
      </header>

      <section aria-labelledby="trade-detail-facts-title" className="space-y-3">
        <h2
          id="trade-detail-facts-title"
          className="font-mono text-2xs uppercase tracking-wider text-muted-foreground"
        >
          <span className="mr-2 text-primary">&gt;</span>
          取引の事実
        </h2>
        <div className="grid grid-cols-2 gap-px border border-border bg-border sm:grid-cols-4">
          <TradeFact label="売買" value={isBuy ? '買い' : '売り'} />
          <TradeFact label="数量" value={trade.qty.toLocaleString() + ' 株'} />
          <TradeFact label="単価" value={formatYen(trade.price)} />
          {isBuy ? (
            <TradeFact
              label="残り株数"
              value={(trade.remaining_qty ?? 0).toLocaleString() + ' 株'}
            />
          ) : (
            <TradeFact
              label="実現損益"
              value={
                trade.realized_pnl == null
                  ? '—'
                  : formatYen(trade.realized_pnl, true)
              }
              valueClassName={
                trade.realized_pnl == null
                  ? undefined
                  : pnlColorClass(trade.realized_pnl)
              }
            />
          )}
        </div>

        {!isBuy && (
          <section
            aria-labelledby="trade-detail-matched-buys-title"
            className="space-y-2"
          >
            <h3
              id="trade-detail-matched-buys-title"
              className="text-sm font-semibold"
            >
              対応する買い
            </h3>
            <div className="divide-y divide-border border border-border bg-card">
              {matchedBuys.length === 0 ? (
                <p className="px-3 py-4 text-sm text-muted-foreground">
                  対応する買いはありません。
                </p>
              ) : (
                matchedBuys.map((buy, index) => (
                  <div
                    key={buy.date + '-' + String(index)}
                    className="grid grid-cols-3 gap-3 px-3 py-2.5 font-mono text-xs"
                  >
                    <span className="tabular-nums">{buy.date}</span>
                    <span className="text-right tabular-nums">
                      {buy.qty.toLocaleString()} 株
                    </span>
                    <span className="text-right tabular-nums">
                      {formatYen(buy.price)}
                    </span>
                  </div>
                ))
              )}
            </div>
          </section>
        )}
      </section>

      <TradeDetailChartView
        trade={trade}
        currency={props.chartCurrency}
        state={props.chartState}
      />

      <section aria-labelledby="trade-detail-notes-title" className="space-y-3">
        <div className="flex flex-wrap items-baseline justify-between gap-3">
          <h2
            id="trade-detail-notes-title"
            className="font-mono text-2xs uppercase tracking-wider text-muted-foreground"
          >
            <span className="mr-2 text-primary">&gt;</span>
            この取引のノート
          </h2>
          <Button
            type="button"
            size="sm"
            variant="outline"
            onClick={props.onManageNotes}
          >
            既存のノートを紐付ける
          </Button>
        </div>

        {props.noteLinkError != null && (
          <p role="alert" className="text-sm text-primary">
            {props.noteLinkError}
          </p>
        )}
        <div className="divide-y divide-border border border-border bg-card">
          {props.notesError ? (
            <p role="alert" className="px-3 py-4 text-sm text-primary">
              ノートを読み込めませんでした。
            </p>
          ) : props.notesPending ? (
            <p className="px-3 py-4 text-sm text-muted-foreground">
              読み込み中...
            </p>
          ) : props.linkedNotes.length === 0 ? (
            <p className="px-3 py-4 text-sm text-muted-foreground">
              この取引に紐付いたノートはありません。
            </p>
          ) : (
            props.linkedNotes.map((note) => (
              <Link
                key={note.id}
                to="/notes/$noteId"
                params={{ noteId: note.id }}
                search={{ version_id: note.version_id }}
                className="flex min-w-0 items-center justify-between gap-3 px-3 py-2.5 text-sm hover:bg-surface-strong hover:text-primary"
              >
                <span className="min-w-0 truncate">{note.title}</span>
                <span className="shrink-0 text-2xs text-muted-foreground">
                  {note.created_by_kind === 'human' ? '自分' : 'エージェント'}
                </span>
              </Link>
            ))
          )}
        </div>
        <Button type="button" onClick={props.onWriteReflection}>
          振り返りを書く
        </Button>
      </section>
    </div>
  )
}

function DetailBackLink() {
  return (
    <Link
      to="/trades"
      className="inline-flex items-center text-sm text-muted-foreground-strong hover:text-foreground hover:underline"
    >
      ← 資産 (取引)
    </Link>
  )
}

function TradeFact({
  label,
  value,
  valueClassName,
}: {
  label: string
  value: string
  valueClassName?: string
}) {
  return (
    <div className="flex min-h-18 flex-col justify-center gap-1 bg-card px-3.5 py-3">
      <span className="font-mono text-2xs uppercase tracking-wider text-muted-foreground">
        {label}
      </span>
      <span
        className={
          'font-mono text-base font-bold tabular-nums ' +
          (valueClassName ?? 'text-foreground')
        }
      >
        {value}
      </span>
    </div>
  )
}
