import { Link } from '@tanstack/react-router'

import { formatYen } from '#components/trades/format'
import type { components } from '#lib/api/schema.gen'
import { buildSnippet } from '#lib/note-utils'
import {
  sortAnnotationsNewestFirst,
  sortTradesNewestFirst,
} from '#lib/stock-page-model'
import { cn } from '#lib/utils'

type Annotation = components['schemas']['Annotation']
type Trade = components['schemas']['TradeListItem']

const dateFormatter = new Intl.DateTimeFormat('ja-JP', {
  month: '2-digit',
  day: '2-digit',
  timeZone: 'Asia/Tokyo',
})

type ActivityTab = 'annotations' | 'trades'
type ListState = 'ready' | 'loading' | 'error'

function formatDate(date: string): string {
  return dateFormatter.format(new Date(date))
}

function getAuthorLabel(kind: string): string {
  return kind === 'human' ? '自分' : 'エージェント'
}

function ActivityTabs({
  activeTab,
  onTabChange,
  annotationCount,
  tradeCount,
}: {
  activeTab: ActivityTab
  onTabChange: (tab: ActivityTab) => void
  annotationCount: string
  tradeCount: string
}) {
  const tabs = [
    {
      id: 'annotations' as const,
      label: 'アノテーション',
      count: annotationCount,
    },
    { id: 'trades' as const, label: '取引', count: tradeCount },
  ]

  return (
    <div
      className="flex overflow-x-auto border-b border-border"
      role="tablist"
      aria-label="銘柄情報"
    >
      {tabs.map((tab) => (
        <button
          key={tab.id}
          id={`stock-page-tab-${tab.id}`}
          type="button"
          role="tab"
          aria-selected={activeTab === tab.id}
          aria-controls={`stock-page-panel-${tab.id}`}
          className={cn(
            'shrink-0 border-b-2 px-3 py-2 font-mono text-xs',
            activeTab === tab.id
              ? 'border-primary text-foreground'
              : 'border-transparent text-muted-foreground hover:text-foreground',
          )}
          onClick={() => {
            onTabChange(tab.id)
          }}
        >
          {tab.label}
          <span className="ml-1.5 text-2xs text-muted-foreground">
            {tab.count}
          </span>
        </button>
      ))}
    </div>
  )
}

function AnnotationRows({
  annotations,
  selectedAnnotationId,
  onSelectAnnotation,
  state,
}: {
  annotations: Annotation[]
  selectedAnnotationId: string | null
  onSelectAnnotation: (id: string) => void
  state: ListState
}) {
  if (state === 'loading') {
    return (
      <p className="px-3.5 py-4 font-mono text-xs text-muted-foreground">
        読み込み中
      </p>
    )
  }
  if (state === 'error') {
    return (
      <p className="px-3.5 py-4 font-mono text-xs text-destructive">
        アノテーションの取得に失敗しました
      </p>
    )
  }
  if (annotations.length === 0) {
    return (
      <p className="px-3.5 py-4 font-mono text-xs text-muted-foreground">
        アノテーションはありません
      </p>
    )
  }

  return (
    <ul>
      {sortAnnotationsNewestFirst(annotations).map((annotation) => {
        const isSelected = annotation.id === selectedAnnotationId
        const timestamp = annotation.timestamp_start ?? annotation.timestamp

        return (
          <li
            key={annotation.id}
            className="flex items-stretch border-b border-border last:border-b-0"
          >
            <button
              type="button"
              aria-pressed={isSelected}
              className={cn(
                'flex min-w-0 flex-1 flex-col gap-1.5 px-3.5 py-3 text-left hover:bg-surface-strong',
                isSelected &&
                  'bg-surface-strong ring-1 ring-inset ring-primary/50',
              )}
              onClick={() => {
                onSelectAnnotation(annotation.id)
              }}
            >
              <span className="flex flex-wrap items-center gap-x-2 gap-y-1 font-mono text-2xs text-muted-foreground">
                <time dateTime={timestamp}>{formatDate(timestamp)}</time>
                <span>{getAuthorLabel(annotation.created_by_kind)}</span>
                {annotation.status === 'unread' && (
                  <span className="border border-primary/50 px-1 text-primary">
                    未レビュー
                  </span>
                )}
              </span>
              <span className="line-clamp-2 text-sm text-foreground">
                {buildSnippet(annotation.text)}
              </span>
            </button>
            <Link
              to="/annotations/$annoId"
              params={{ annoId: annotation.id }}
              className="shrink-0 self-center px-3.5 py-2 font-mono text-2xs text-muted-foreground underline decoration-border underline-offset-4 hover:text-foreground"
            >
              詳細 →
            </Link>
          </li>
        )
      })}
    </ul>
  )
}

function TradeRows({ trades, state }: { trades: Trade[]; state: ListState }) {
  if (state === 'loading') {
    return (
      <p className="px-3.5 py-4 font-mono text-xs text-muted-foreground">
        読み込み中
      </p>
    )
  }
  if (state === 'error') {
    return (
      <p className="px-3.5 py-4 font-mono text-xs text-destructive">
        取引の取得に失敗しました
      </p>
    )
  }
  if (trades.length === 0) {
    return (
      <p className="px-3.5 py-4 font-mono text-xs text-muted-foreground">
        取引はありません
      </p>
    )
  }

  const sortedTrades = sortTradesNewestFirst(trades)

  return (
    <>
      <ul className="divide-y divide-border sm:hidden">
        {sortedTrades.map((trade) => (
          <li
            key={trade.id}
            className="flex items-start justify-between gap-4 px-3.5 py-3"
          >
            <div className="space-y-1">
              <time
                dateTime={trade.date}
                className="font-mono text-2xs text-muted-foreground"
              >
                {formatDate(`${trade.date}T00:00:00+09:00`)}
              </time>
              <p className="font-mono text-sm text-foreground">
                <TradeSide side={trade.side} />
                <span className="ml-2 text-muted-foreground">
                  {trade.qty.toLocaleString('ja-JP')} 株
                </span>
              </p>
            </div>
            <span className="font-mono text-sm tabular-nums">
              {formatYen(trade.price)}
            </span>
          </li>
        ))}
      </ul>
      <div className="hidden overflow-x-auto sm:block">
        <table className="w-full font-mono text-xs">
          <thead>
            <tr className="border-b border-border text-2xs text-muted-foreground">
              <th className="px-3.5 py-2 text-left font-normal">日付</th>
              <th className="px-3.5 py-2 text-left font-normal">売買</th>
              <th className="px-3.5 py-2 text-right font-normal">数量</th>
              <th className="px-3.5 py-2 text-right font-normal">単価</th>
            </tr>
          </thead>
          <tbody>
            {sortedTrades.map((trade) => (
              <tr
                key={trade.id}
                className="border-b border-border last:border-b-0"
              >
                <td className="px-3.5 py-2.5 tabular-nums text-muted-foreground-strong">
                  {formatDate(`${trade.date}T00:00:00+09:00`)}
                </td>
                <td className="px-3.5 py-2.5">
                  <TradeSide side={trade.side} />
                </td>
                <td className="px-3.5 py-2.5 text-right tabular-nums">
                  {trade.qty.toLocaleString('ja-JP')} 株
                </td>
                <td className="px-3.5 py-2.5 text-right tabular-nums">
                  {formatYen(trade.price)}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </>
  )
}

function TradeSide({ side }: { side: string }) {
  const isBuy = side === 'buy'
  return (
    <span
      className={cn(
        'inline-block border px-1 text-2xs',
        isBuy ? 'border-up text-up' : 'border-down text-down',
      )}
    >
      {isBuy ? '買' : '売'}
    </span>
  )
}

export function StockPageActivity({
  annotations,
  trades,
  activeTab,
  onTabChange,
  selectedAnnotationId,
  onSelectAnnotation,
  annotationState = 'ready',
  tradeState = 'ready',
}: {
  annotations: Annotation[]
  trades: Trade[]
  activeTab: ActivityTab
  onTabChange: (tab: ActivityTab) => void
  selectedAnnotationId: string | null
  onSelectAnnotation: (id: string) => void
  annotationState?: ListState
  tradeState?: ListState
}) {
  const annotationCount =
    annotationState === 'loading'
      ? '…'
      : annotationState === 'error'
        ? '—'
        : String(annotations.length)
  const tradeCount =
    tradeState === 'loading'
      ? '…'
      : tradeState === 'error'
        ? '—'
        : String(trades.length)

  return (
    <section className="border border-border bg-card" aria-label="銘柄の一覧">
      <ActivityTabs
        activeTab={activeTab}
        onTabChange={onTabChange}
        annotationCount={annotationCount}
        tradeCount={tradeCount}
      />
      <div
        id={`stock-page-panel-${activeTab}`}
        role="tabpanel"
        aria-labelledby={`stock-page-tab-${activeTab}`}
      >
        {activeTab === 'annotations' ? (
          <AnnotationRows
            annotations={annotations}
            selectedAnnotationId={selectedAnnotationId}
            onSelectAnnotation={onSelectAnnotation}
            state={annotationState}
          />
        ) : (
          <TradeRows trades={trades} state={tradeState} />
        )}
      </div>
    </section>
  )
}
