import { Button } from '@fohte/ui/button'
import { Tabs, TabsList, TabsPanel, TabsTab } from '@fohte/ui/tabs'
import { Link } from '@tanstack/react-router'

import { StockPageFinancials } from '#components/stock-page/stock-page-financials'
import { StatusPill } from '#components/strategy-home/status-pill'
import { formatYen } from '#components/trades/format'
import type { components } from '#lib/api/schema.gen'
import { buildSnippet, formatRelative } from '#lib/note-utils'
import {
  sortAnnotationsNewestFirst,
  sortNotesNewestFirst,
  sortTradesNewestFirst,
  type StockPageListState,
} from '#lib/stock-page-model'
import { cn } from '#lib/utils'

type Annotation = components['schemas']['Annotation']
type Note = components['schemas']['Note']
type Trade = components['schemas']['TradeListItem']
type FinancialSummary = components['schemas']['StockFinancialSummaryResponse']
type GroupMembership = components['schemas']['StockGroupMembershipResponse']

const dateFormatter = new Intl.DateTimeFormat('ja-JP', {
  month: '2-digit',
  day: '2-digit',
  timeZone: 'Asia/Tokyo',
})

type ActivityTab = 'annotations' | 'notes' | 'trades' | 'financials'
type ListState = StockPageListState

function isActivityTab(value: unknown): value is ActivityTab {
  return (
    value === 'annotations' ||
    value === 'notes' ||
    value === 'trades' ||
    value === 'financials'
  )
}

function ActivityMessage({
  kind,
  children,
}: {
  kind: 'loading' | 'error' | 'empty'
  children: string
}) {
  return (
    <p
      className={cn(
        'px-3.5 py-4 font-mono text-xs',
        kind === 'error' ? 'text-destructive' : 'text-muted-foreground',
      )}
    >
      {children}
    </p>
  )
}

function getCountLabel(state: ListState, count: number): string {
  if (state === 'loading') return '…'
  if (state === 'error') return '—'
  return String(count)
}

function formatDate(date: string): string {
  return dateFormatter.format(new Date(date))
}

function getAuthorLabel(kind: string): string {
  return kind === 'human' ? '自分' : 'エージェント'
}

function ActivityTabs({
  annotationCount,
  noteCount,
  tradeCount,
}: {
  annotationCount: string
  noteCount: string
  tradeCount: string
}) {
  const tabs = [
    {
      id: 'annotations' as const,
      label: 'アノテーション',
      count: annotationCount,
    },
    { id: 'notes' as const, label: 'ノート', count: noteCount },
    { id: 'trades' as const, label: '取引', count: tradeCount },
    { id: 'financials' as const, label: '財務', count: null },
  ]

  return (
    <div className="overflow-x-auto">
      <TabsList aria-label="銘柄情報">
        {tabs.map((tab) => (
          <TabsTab key={tab.id} value={tab.id}>
            {tab.label}
            {tab.count != null && (
              <span className="ml-1.5 text-2xs text-muted-foreground">
                {tab.count}
              </span>
            )}
          </TabsTab>
        ))}
      </TabsList>
    </div>
  )
}

function NoteRows({ notes, state }: { notes: Note[]; state: ListState }) {
  if (state === 'loading') {
    return <ActivityMessage kind="loading">読み込み中</ActivityMessage>
  }
  if (state === 'error') {
    return (
      <ActivityMessage kind="error">ノートの取得に失敗しました</ActivityMessage>
    )
  }
  if (notes.length === 0) {
    return (
      <ActivityMessage kind="empty">
        この銘柄に触れたノートはありません
      </ActivityMessage>
    )
  }

  return (
    <ul>
      {sortNotesNewestFirst(notes).map((note) => (
        <li key={note.id} className="border-b border-border last:border-b-0">
          <Link
            to="/notes/$noteId"
            params={{ noteId: note.id }}
            className="flex min-w-0 flex-col gap-1.5 px-3.5 py-3 hover:bg-surface-strong"
          >
            <span className="line-clamp-2 text-sm text-foreground">
              {note.title}
            </span>
            <span className="flex flex-wrap items-center gap-x-2 gap-y-1 font-mono text-2xs text-muted-foreground">
              <span>{getAuthorLabel(note.created_by_kind)}</span>
              {note.status === 'unread' && <StatusPill status="unread" />}
              <time className="ml-auto" dateTime={note.updated_at}>
                {formatRelative(note.updated_at)}
              </time>
            </span>
          </Link>
        </li>
      ))}
    </ul>
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
    return <ActivityMessage kind="loading">読み込み中</ActivityMessage>
  }
  if (state === 'error') {
    return (
      <ActivityMessage kind="error">
        アノテーションの取得に失敗しました
      </ActivityMessage>
    )
  }
  if (annotations.length === 0) {
    return (
      <ActivityMessage kind="empty">アノテーションはありません</ActivityMessage>
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
            <Button
              type="button"
              variant={isSelected ? 'default' : 'ghost'}
              aria-pressed={isSelected}
              className="h-auto min-w-0 flex-1 flex-col items-start gap-1.5 whitespace-normal px-3.5 py-3 text-left"
              onClick={() => {
                onSelectAnnotation(annotation.id)
              }}
            >
              <span className="flex flex-wrap items-center gap-x-2 gap-y-1 font-mono text-2xs text-muted-foreground">
                <time dateTime={timestamp}>{formatDate(timestamp)}</time>
                <span>{getAuthorLabel(annotation.created_by_kind)}</span>
                {annotation.status === 'unread' && (
                  <StatusPill status="unread" />
                )}
              </span>
              <span className="line-clamp-2 text-sm text-foreground">
                {buildSnippet(annotation.text)}
              </span>
            </Button>
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
    return <ActivityMessage kind="loading">読み込み中</ActivityMessage>
  }
  if (state === 'error') {
    return (
      <ActivityMessage kind="error">取引の取得に失敗しました</ActivityMessage>
    )
  }
  if (trades.length === 0) {
    return <ActivityMessage kind="empty">取引はありません</ActivityMessage>
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
  notes,
  trades,
  activeTab,
  onTabChange,
  selectedAnnotationId,
  onSelectAnnotation,
  annotationState = 'ready',
  noteState = 'ready',
  tradeState = 'ready',
  financialSummaries,
  financialSummariesState,
  groups,
  groupsState,
}: {
  annotations: Annotation[]
  notes: Note[]
  trades: Trade[]
  activeTab: ActivityTab
  onTabChange: (tab: ActivityTab) => void
  selectedAnnotationId: string | null
  onSelectAnnotation: (id: string) => void
  annotationState?: ListState
  noteState?: ListState
  tradeState?: ListState
  financialSummaries: FinancialSummary[]
  financialSummariesState: ListState
  groups: GroupMembership[]
  groupsState: ListState
}) {
  const annotationCount = getCountLabel(annotationState, annotations.length)
  const noteCount = getCountLabel(noteState, notes.length)
  const tradeCount = getCountLabel(tradeState, trades.length)

  return (
    <Tabs
      value={activeTab}
      onValueChange={(value: unknown) => {
        if (isActivityTab(value)) onTabChange(value)
      }}
    >
      <section className="border border-border bg-card" aria-label="銘柄の一覧">
        <ActivityTabs
          annotationCount={annotationCount}
          noteCount={noteCount}
          tradeCount={tradeCount}
        />
        <TabsPanel value={activeTab}>
          {activeTab === 'annotations' ? (
            <AnnotationRows
              annotations={annotations}
              selectedAnnotationId={selectedAnnotationId}
              onSelectAnnotation={onSelectAnnotation}
              state={annotationState}
            />
          ) : activeTab === 'notes' ? (
            <NoteRows notes={notes} state={noteState} />
          ) : activeTab === 'trades' ? (
            <TradeRows trades={trades} state={tradeState} />
          ) : (
            <StockPageFinancials
              summaries={financialSummaries}
              summariesState={financialSummariesState}
              groups={groups}
              groupsState={groupsState}
            />
          )}
        </TabsPanel>
      </section>
    </Tabs>
  )
}
