import { createFileRoute } from '@tanstack/react-router'
import { Plus } from 'lucide-react'
import { useState } from 'react'

import { CreateStrategyDialog } from '#components/strategy-home/create-strategy-dialog'
import { StrategyCard } from '#components/strategy-home/strategy-card'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/strategies/')({
  component: StrategyListPage,
})

function StrategyListPage() {
  const [creating, setCreating] = useState(false)
  const { data: strategies, isPending } = $api.useQuery(
    'get',
    '/api/strategies',
  )
  return (
    <div className="font-sans text-foreground">
      <div className="mb-8 max-w-180">
        <h1 className="mb-3 text-2xl font-bold tracking-tight">
          <span className="font-mono font-bold text-primary">&gt;</span>{' '}
          戦略を選ぶ
        </h1>
        <p className="text-sm leading-relaxed text-muted-foreground-strong">
          各戦略は運用資金の区分です。取引の帰属先・投資可能額・リスク上限を管理します。分析は口座全体を対象に行います。
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
            {(strategies ?? []).map((strategy) => (
              <StrategyCard key={strategy.id} strategy={strategy} />
            ))}
            <button
              type="button"
              onClick={() => {
                setCreating(true)
              }}
              className="flex min-h-47 cursor-pointer flex-col items-center justify-center gap-2 border border-dashed border-border bg-transparent p-4 text-center text-muted-foreground hover:border-primary hover:text-primary"
            >
              <Plus className="size-6" />
              <div className="font-mono text-sm">新しい戦略を作る</div>
            </button>
          </div>
        </>
      )}

      <CreateStrategyDialog open={creating} onOpenChange={setCreating} />
    </div>
  )
}
