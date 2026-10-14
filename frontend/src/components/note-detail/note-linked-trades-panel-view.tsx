import { Link } from '@tanstack/react-router'

import { StockNameLink } from '#components/stocks/stock-name-link'
import { Skeleton } from '#components/ui/skeleton'
import type { components } from '#lib/api/schema.gen'

type NoteLinkedTrade = components['schemas']['NoteLinkedTrade']

type NoteLinkedTradesPanelViewProps =
  | { state: 'loading' }
  | { state: 'error' }
  | {
      state: 'ready'
      trades: NoteLinkedTrade[]
      stockNameById: Map<string, string>
    }

export function NoteLinkedTradesPanelView(
  props: NoteLinkedTradesPanelViewProps,
) {
  if (props.state === 'ready' && props.trades.length === 0) return null

  return (
    <section
      className="border border-border bg-card"
      aria-labelledby="note-linked-trades-heading"
    >
      <header className="border-b border-border px-3.5 py-2">
        <h2
          id="note-linked-trades-heading"
          className="font-mono text-xs font-bold uppercase tracking-wider text-foreground"
        >
          紐付いた取引
        </h2>
      </header>
      {props.state === 'loading' ? (
        <div className="space-y-2 px-3.5 py-3">
          <Skeleton className="h-4 w-full" />
          <Skeleton className="h-4 w-3/4" />
        </div>
      ) : props.state === 'error' ? (
        <p role="alert" className="px-3.5 py-3 font-mono text-xs text-primary">
          紐付いた取引を読み込めませんでした。
        </p>
      ) : (
        <ul className="divide-y divide-border">
          {props.trades.map((trade) => {
            const stockName = props.stockNameById.get(trade.symbol) ?? null
            const sideLabel = trade.side === 'buy' ? '買い' : '売り'

            return (
              <li key={trade.id} className="space-y-1 px-3.5 py-2.5">
                <div className="flex items-center justify-between gap-2 font-mono text-xs">
                  <Link
                    to="/trades/$tradeId"
                    params={{ tradeId: trade.id }}
                    className="flex items-center gap-2 text-muted-foreground-strong hover:text-primary hover:underline"
                  >
                    <time dateTime={trade.date} className="tabular-nums">
                      {trade.date}
                    </time>
                    <SideBadge side={trade.side} label={sideLabel} />
                  </Link>
                  <Link
                    to="/trades/$tradeId"
                    params={{ tradeId: trade.id }}
                    aria-label={`${trade.date} ${stockName ?? trade.symbol} ${sideLabel}の取引詳細`}
                    className="shrink-0 text-2xs text-primary hover:underline"
                  >
                    詳細 →
                  </Link>
                </div>
                <div className="truncate text-xs">
                  <StockNameLink
                    name={stockName}
                    symbol={trade.symbol}
                    missingName="link-symbol"
                  />
                </div>
              </li>
            )
          })}
        </ul>
      )}
    </section>
  )
}

function SideBadge({ side, label }: { side: string; label: string }) {
  const className =
    side === 'buy' ? 'border-up text-up' : 'border-down text-down'

  return (
    <span
      className={`inline-grid h-5 min-w-6 place-items-center border px-1 text-2xs ${className}`}
    >
      {label}
    </span>
  )
}
