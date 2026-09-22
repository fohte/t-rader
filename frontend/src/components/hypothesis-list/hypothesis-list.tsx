import { Link } from '@tanstack/react-router'

import { HypothesisStatusPill } from '#components/strategy-home/hypothesis-status-pill'

export interface HypothesisListItem {
  hypothesisId: string
  title: string
  status: string
  updatedAt: string
}

interface HypothesisListProps {
  hypotheses: HypothesisListItem[]
}

export function HypothesisList({ hypotheses }: HypothesisListProps) {
  return (
    <section className="border border-border bg-card">
      <div className="flex items-baseline justify-between border-b border-border px-3.5 py-2">
        <h2 className="font-mono text-xs font-bold uppercase tracking-wider text-foreground">
          一覧
        </h2>
        <span className="font-mono text-2xs text-muted-foreground">
          {hypotheses.length}
        </span>
      </div>
      {hypotheses.length === 0 ? (
        <div className="px-3.5 py-3 font-mono text-xs text-muted-foreground">
          —
        </div>
      ) : (
        <div>
          {hypotheses.map((hypothesis) => (
            <Link
              key={hypothesis.hypothesisId}
              to="/hypotheses/$hypothesisId"
              params={{ hypothesisId: hypothesis.hypothesisId }}
              className="flex flex-col gap-1 border-b border-border px-3.5 py-2.5 last:border-b-0 hover:bg-surface-strong"
            >
              <span className="line-clamp-2 text-sm text-foreground">
                {hypothesis.title}
              </span>
              <span className="flex flex-wrap items-center gap-2 font-mono text-2xs">
                <HypothesisStatusPill status={hypothesis.status} />
                <span className="ml-auto text-muted-foreground">
                  {hypothesis.updatedAt}
                </span>
              </span>
            </Link>
          ))}
        </div>
      )}
    </section>
  )
}
