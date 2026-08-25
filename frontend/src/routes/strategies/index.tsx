import { createFileRoute, Link } from '@tanstack/react-router'
import { Plus } from 'lucide-react'
import { useMemo, useState } from 'react'

import { CreateStrategyDialog } from '#components/strategy-home/create-strategy-dialog'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'
import { formatRelative } from '#lib/note-utils'

export const Route = createFileRoute('/strategies/')({
  component: StrategyListPage,
})

function StrategyListPage() {
  const [creating, setCreating] = useState(false)
  const { data: strategies, isPending } = $api.useQuery(
    'get',
    '/api/strategies',
  )
  const { data: unreadNotes } = $api.useQuery('get', '/api/notes', {
    params: { query: { status: 'unread' } },
  })

  const unreadByStrategy = useMemo(() => {
    const m = new Map<string, number>()
    for (const n of unreadNotes ?? []) {
      m.set(n.strategy_id, (m.get(n.strategy_id) ?? 0) + 1)
    }
    return m
  }, [unreadNotes])

  return (
    <div className="font-sans text-foreground">
      <div className="mb-8 max-w-180">
        <h1 className="mb-3 text-2xl font-bold tracking-tight">
          <span className="font-mono font-bold text-primary">&gt;</span>{' '}
          戦略を選ぶ
        </h1>
        <p className="text-sm leading-relaxed text-muted-foreground-strong">
          各戦略は永続ワークスペース。LLM
          がアナリスト役として監視対象を広げ、ノートとアノテーションを産出します。あなたは時々開いてレビューします。
        </p>
      </div>

      {isPending ? (
        <div className="grid grid-cols-1 gap-3.5 sm:grid-cols-2 lg:grid-cols-3">
          <Skeleton className="h-47" />
          <Skeleton className="h-47" />
          <Skeleton className="h-47" />
        </div>
      ) : (
        <>
          <div className="mb-2.5 flex items-baseline gap-2 font-mono text-2xs uppercase tracking-wider text-muted-foreground">
            <span className="text-primary">&gt;</span>
            <span>strategies</span>
            <span className="text-muted-foreground-strong">
              {strategies?.length ?? 0} 件
            </span>
          </div>
          <div className="grid grid-cols-1 gap-3.5 sm:grid-cols-2 lg:grid-cols-3">
            {(strategies ?? []).map((s) => {
              const unread = unreadByStrategy.get(s.id) ?? 0
              return (
                <Link
                  key={s.id}
                  to="/strategies/$id"
                  params={{ id: s.id }}
                  className="flex min-h-47 cursor-pointer flex-col gap-3.5 border border-border bg-card p-4 transition-colors hover:border-muted-foreground"
                >
                  <div className="flex items-start justify-between gap-2.5">
                    <div className="min-w-0">
                      <div className="truncate font-mono text-base font-bold leading-tight">
                        {s.name}
                      </div>
                    </div>
                    {unread > 0 && (
                      <span className="inline-grid h-5 min-w-5 flex-shrink-0 place-items-center bg-primary px-1.5 font-mono text-2xs text-white">
                        {unread}
                      </span>
                    )}
                  </div>
                  {s.description != null && s.description !== '' && (
                    <p className="line-clamp-3 text-sm leading-relaxed text-muted-foreground-strong">
                      {s.description}
                    </p>
                  )}
                  <div className="mt-auto flex items-center justify-between border-t border-border pt-3 font-mono text-2xs">
                    <span className="text-muted-foreground">
                      更新 {formatRelative(s.updated_at)}
                    </span>
                    <span className="text-muted-foreground-strong">
                      {unread > 0 ? (
                        <>
                          <span className="font-bold text-primary">
                            {unread}
                          </span>{' '}
                          件 未レビュー
                        </>
                      ) : (
                        'レビュー済み'
                      )}
                    </span>
                  </div>
                </Link>
              )
            })}
            <button
              type="button"
              onClick={() => {
                setCreating(true)
              }}
              className="flex min-h-47 cursor-pointer flex-col items-center justify-center gap-2 border border-dashed border-border bg-transparent p-4 text-center text-muted-foreground hover:border-primary hover:text-primary"
            >
              <Plus className="size-6" />
              <div className="font-mono text-sm">新しい戦略を作る</div>
              <div className="font-mono text-2xs">シード関心は後から追加</div>
            </button>
          </div>
        </>
      )}

      <CreateStrategyDialog open={creating} onOpenChange={setCreating} />
    </div>
  )
}
