import { useMemo } from 'react'

import { $api } from '#lib/api/client'

interface PredictionsPanelProps {
  noteId: string
}

const DIRECTION_LABEL: Record<string, string> = {
  outperform: '上回る',
  underperform: '下回る',
}

export function PredictionsPanel({ noteId }: PredictionsPanelProps) {
  const { data: predictions } = $api.useQuery(
    'get',
    '/api/notes/{id}/predictions',
    { params: { path: { id: noteId } } },
  )
  const { data: stocks = [] } = $api.useQuery('get', '/api/refs/stocks')
  const stockNameById = useMemo(
    () => new Map(stocks.map((s) => [s.id, s.name])),
    [stocks],
  )

  if (predictions == null || predictions.length === 0) return null

  const stockLabel = (id: string): string => {
    const name = stockNameById.get(id)
    return name != null ? `${id} (${name})` : id
  }

  return (
    <section className="border border-border bg-card">
      <header className="border-b border-border px-3.5 py-2">
        <h3 className="font-mono text-xs font-bold uppercase tracking-wider text-foreground">
          予測
        </h3>
      </header>
      <div className="divide-y divide-border">
        {predictions.map((p) => (
          <div
            key={p.prediction_id}
            className="space-y-1 px-3.5 py-3 font-mono text-2xs"
          >
            <div className="text-muted-foreground-strong">
              {stockLabel(p.target_stock_id)} が{' '}
              {stockLabel(p.benchmark_stock_id)} を{' '}
              <span
                className={
                  p.direction === 'outperform' ? 'text-up' : 'text-down'
                }
              >
                {DIRECTION_LABEL[p.direction] ?? p.direction}
              </span>
            </div>
            <div className="text-muted-foreground">
              確率 {Math.round(p.probability * 100)}% · {p.base_date} 〜{' '}
              {p.due_date}
            </div>
          </div>
        ))}
      </div>
    </section>
  )
}
