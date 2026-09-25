import { Link } from '@tanstack/react-router'

import { Skeleton } from '#components/ui/skeleton'
import type { components } from '#lib/api/schema.gen'

type NoteLinkItem = components['schemas']['NoteLinkItem']

interface NoteLinksPanelViewProps {
  outgoing: NoteLinkItem[]
  incoming: NoteLinkItem[]
  isPending: boolean
  isError: boolean
}

export function NoteLinksPanelView({
  outgoing,
  incoming,
  isPending,
  isError,
}: NoteLinksPanelViewProps) {
  return (
    <section className="border border-border bg-card">
      <header className="border-b border-border px-3.5 py-2">
        <h3 className="font-mono text-xs font-bold uppercase tracking-wider text-foreground">
          ノートリンク
        </h3>
      </header>
      {isPending ? (
        <div className="space-y-2 px-3.5 py-3">
          <Skeleton className="h-4 w-full" />
          <Skeleton className="h-4 w-3/4" />
        </div>
      ) : isError ? (
        <p className="px-3.5 py-3 font-mono text-xs text-primary">
          ノートリンクを読み込めませんでした
        </p>
      ) : (
        <div className="divide-y divide-border">
          <LinkList title="このバージョンから" items={outgoing} isOutgoing />
          <LinkList title="このノートへの参照" items={incoming} />
        </div>
      )}
    </section>
  )
}

function LinkList({
  title,
  items,
  isOutgoing = false,
}: {
  title: string
  items: NoteLinkItem[]
  isOutgoing?: boolean
}) {
  return (
    <section>
      <h4 className="px-3.5 pt-2.5 font-mono text-2xs uppercase tracking-wider text-muted-foreground">
        {title}
      </h4>
      {items.length === 0 ? (
        <p className="px-3.5 py-2 font-mono text-2xs text-muted-foreground">
          {isOutgoing
            ? 'このバージョンからのリンクはありません'
            : '現行バージョンからの参照はありません'}
        </p>
      ) : (
        <ul className="divide-y divide-border/70">
          {items.map((item) => (
            <li
              key={`${item.note_id}:${item.version_id ?? 'current'}`}
              className="flex items-center justify-between gap-2 px-3.5 py-2"
            >
              <Link
                to="/notes/$noteId"
                params={{ noteId: item.note_id }}
                search={{ version_id: item.version_id ?? undefined }}
                className="min-w-0 truncate text-xs text-foreground hover:text-primary hover:underline"
              >
                {item.title ?? 'タイトルなし'}
              </Link>
              <span className="shrink-0 font-mono text-2xs text-muted-foreground">
                {isOutgoing && item.version_id == null
                  ? '現行バージョン'
                  : item.version_no == null
                    ? ''
                    : `v${String(item.version_no)}`}
              </span>
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}
