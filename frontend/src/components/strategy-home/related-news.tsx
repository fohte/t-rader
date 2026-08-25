import { $api } from '#lib/api/client'
import type { components } from '#lib/api/schema.gen'

type NewsItem = components['schemas']['StrategyNewsItem']

interface RelatedNewsProps {
  strategyId: string
}

export function RelatedNews({ strategyId }: RelatedNewsProps) {
  const { data, isPending } = $api.useQuery(
    'get',
    '/api/strategies/{id}/news',
    {
      params: { path: { id: strategyId } },
    },
  )

  return <RelatedNewsView items={data ?? null} isPending={isPending} />
}

interface RelatedNewsViewProps {
  items: NewsItem[] | null
  isPending: boolean
}

export function RelatedNewsView({ items, isPending }: RelatedNewsViewProps) {
  return (
    <section className="border border-border bg-card">
      <div className="flex items-baseline justify-between border-b border-border px-3.5 py-2">
        <h3 className="font-mono text-xs font-bold uppercase tracking-wider text-foreground">
          関連ニュース
        </h3>
      </div>
      {renderBody(items, isPending)}
    </section>
  )
}

function renderBody(items: NewsItem[] | null, isPending: boolean) {
  if (isPending) {
    return (
      <div className="px-3.5 py-3 font-mono text-xs text-muted-foreground">
        loading...
      </div>
    )
  }
  if (items == null || items.length === 0) {
    return (
      <div className="px-3.5 py-3 font-mono text-xs text-muted-foreground">
        —
      </div>
    )
  }
  return (
    <ul className="divide-y divide-border">
      {items.map((n) => (
        <li key={n.id} className="px-3.5 py-2.5">
          <a
            href={n.url}
            target="_blank"
            rel="noopener noreferrer"
            className="block text-sm leading-snug text-foreground hover:text-primary"
          >
            {n.title}
          </a>
          <div className="mt-1 flex flex-wrap items-center gap-1.5 font-mono text-2xs uppercase tracking-wider text-muted-foreground">
            <span>{n.source}</span>
            <span>·</span>
            <time dateTime={n.published_at}>{formatTime(n.published_at)}</time>
            {n.matched_refs.length > 0 && (
              <>
                <span>·</span>
                <span className="normal-case text-muted-foreground-strong">
                  {n.matched_refs
                    .map((r) => r.matched_term)
                    .filter((v, i, a) => a.indexOf(v) === i)
                    .join(' / ')}
                </span>
              </>
            )}
          </div>
        </li>
      ))}
    </ul>
  )
}

// Intl.DateTimeFormat の生成は重いのでモジュールスコープで 1 度だけ確保する
const DATE_FORMATTER = new Intl.DateTimeFormat('ja-JP', {
  month: '2-digit',
  day: '2-digit',
  hour: '2-digit',
  minute: '2-digit',
})

function formatTime(iso: string): string {
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return iso
  return DATE_FORMATTER.format(d)
}
