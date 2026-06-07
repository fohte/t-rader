import { createFileRoute, Link } from '@tanstack/react-router'
import { Plus } from 'lucide-react'
import { useMemo, useState } from 'react'

import { CreateStrategyDialog } from '@/components/strategy-home/create-strategy-dialog'
import { Skeleton } from '@/components/ui/skeleton'
import { $api } from '@/lib/api/client'
import { formatRelative } from '@/lib/note-utils'

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
    <div className="font-sans text-[color:var(--color-text-primary)]">
      <div className="mb-8 max-w-[720px]">
        <h1 className="mb-3 text-[26px] font-bold tracking-tight">
          <span className="font-mono font-bold text-[color:var(--color-accent-strategy)]">
            &gt;
          </span>{' '}
          戦略を選ぶ
        </h1>
        <p className="text-[14px] leading-relaxed text-[color:var(--color-text-secondary)]">
          各戦略は永続ワークスペース。LLM
          がアナリスト役として監視対象を広げ、ノートとアノテーションを産出します。あなたは時々開いてレビューします。
        </p>
      </div>

      {isPending ? (
        <div className="grid grid-cols-1 gap-3.5 sm:grid-cols-2 lg:grid-cols-3">
          <Skeleton className="h-[188px]" />
          <Skeleton className="h-[188px]" />
          <Skeleton className="h-[188px]" />
        </div>
      ) : (
        <>
          <div className="mb-2.5 flex items-baseline gap-2 font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
            <span className="text-[color:var(--color-accent-strategy)]">
              &gt;
            </span>
            <span>strategies</span>
            <span className="text-[color:var(--color-text-secondary)]">
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
                  className="flex min-h-[188px] cursor-pointer flex-col gap-3.5 border border-[color:var(--color-border-strategy)] bg-[color:var(--panel)] p-4 transition-colors hover:border-[color:var(--color-text-tertiary)]"
                >
                  <div className="flex items-start justify-between gap-2.5">
                    <div className="min-w-0">
                      <div className="truncate font-mono text-[16px] font-bold leading-tight">
                        {s.name}
                      </div>
                    </div>
                    {unread > 0 && (
                      <span className="inline-grid h-5 min-w-[20px] flex-shrink-0 place-items-center bg-[color:var(--color-accent-strategy)] px-1.5 font-mono text-[11px] text-white">
                        {unread}
                      </span>
                    )}
                  </div>
                  {s.description != null && s.description !== '' && (
                    <p className="line-clamp-3 text-[13px] leading-relaxed text-[color:var(--color-text-secondary)]">
                      {s.description}
                    </p>
                  )}
                  <div className="mt-auto flex items-center justify-between border-t border-[color:var(--color-hairline)] pt-3 font-mono text-[11px]">
                    <span className="text-[color:var(--color-text-tertiary)]">
                      更新 {formatRelative(s.updated_at)}
                    </span>
                    <span className="text-[color:var(--color-text-secondary)]">
                      {unread > 0 ? (
                        <>
                          <span className="font-bold text-[color:var(--color-accent-strategy)]">
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
              className="flex min-h-[188px] cursor-pointer flex-col items-center justify-center gap-2 border border-dashed border-[color:var(--color-border-strategy)] bg-transparent p-4 text-center text-[color:var(--color-text-tertiary)] hover:border-[color:var(--color-accent-strategy)] hover:text-[color:var(--color-accent-strategy)]"
            >
              <Plus className="size-6" />
              <div className="font-mono text-[13px]">新しい戦略を作る</div>
              <div className="font-mono text-[11px]">
                シード関心は後から追加
              </div>
            </button>
          </div>
        </>
      )}

      <CreateStrategyDialog open={creating} onOpenChange={setCreating} />
    </div>
  )
}
