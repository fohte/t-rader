import { Link } from '@tanstack/react-router'

import type { components } from '#lib/api/schema.gen'
import { formatRelative } from '#lib/note-utils'

type Strategy = components['schemas']['Strategy']

export function StrategyCard({ strategy }: { strategy: Strategy }) {
  return (
    <Link
      to="/strategies/$id"
      params={{ id: strategy.id }}
      className="flex min-h-47 cursor-pointer flex-col gap-3.5 border border-border bg-card p-4 transition-colors hover:border-muted-foreground"
    >
      <div className="flex items-start justify-between gap-2.5">
        <div className="min-w-0">
          <div className="truncate font-mono text-base font-bold leading-tight">
            {strategy.name}
          </div>
        </div>
      </div>
      {strategy.description != null && strategy.description !== '' && (
        <p className="line-clamp-3 text-sm leading-relaxed text-muted-foreground-strong">
          {strategy.description}
        </p>
      )}
      <div className="mt-auto flex items-center justify-between border-t border-border pt-3 font-mono text-2xs">
        <span className="text-muted-foreground">
          更新 {formatRelative(strategy.updated_at)}
        </span>
        <span className="text-muted-foreground-strong">詳細を見る →</span>
      </div>
    </Link>
  )
}
