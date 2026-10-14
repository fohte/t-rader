import { Link } from '@tanstack/react-router'

import { taskPhaseLabel } from '#components/task-phase-label'

type TradeRequestedAnalysisNote = {
  id: string
  title: string
  versionId: string
}

export type TradeRequestedAnalysis = {
  strategyId: string
  taskId: string
  prompt: string
  phase: string
  createdAt: string
  notes: TradeRequestedAnalysisNote[]
  notesState: 'loading' | 'error' | 'success'
}

interface TradeRequestedAnalysesProps {
  analyses: TradeRequestedAnalysis[]
  state: 'loading' | 'error' | 'success'
}

const dateFormatter = new Intl.DateTimeFormat('sv-SE', {
  timeZone: 'Asia/Tokyo',
  year: 'numeric',
  month: '2-digit',
  day: '2-digit',
})

export function TradeRequestedAnalyses({
  analyses,
  state,
}: TradeRequestedAnalysesProps) {
  return (
    <section
      aria-labelledby="trade-requested-analyses-heading"
      aria-busy={state === 'loading'}
      className="space-y-3"
    >
      <h2
        id="trade-requested-analyses-heading"
        className="font-mono text-2xs uppercase tracking-wider text-muted-foreground"
      >
        <span aria-hidden="true" className="mr-2 text-primary">
          &gt;
        </span>
        この取引について頼んだ分析
      </h2>

      {state === 'loading' ? (
        <p
          role="status"
          className="border border-border bg-card px-3 py-4 text-sm text-muted-foreground"
        >
          分析を読み込み中です。
        </p>
      ) : state === 'error' ? (
        <p
          role="alert"
          className="border border-border bg-card px-3 py-4 text-sm text-primary"
        >
          頼んだ分析を読み込めませんでした。
        </p>
      ) : analyses.length === 0 ? (
        <p className="border border-border bg-card px-3 py-4 text-sm text-muted-foreground">
          この取引について頼んだ分析はありません。
        </p>
      ) : (
        <ul className="divide-y divide-border border border-border bg-card">
          {analyses.map((analysis) => (
            <li
              key={analysis.strategyId + '-' + analysis.taskId}
              className="flex flex-col gap-2 px-3 py-3 sm:flex-row sm:items-center sm:justify-between"
            >
              <div className="min-w-0 space-y-1">
                <p className="truncate text-sm text-foreground">
                  {analysis.prompt}
                </p>
                <div className="flex flex-wrap items-center gap-x-3 font-mono text-2xs text-muted-foreground">
                  <span
                    className={
                      analysis.phase === 'failed'
                        ? 'text-primary'
                        : analysis.phase === 'running' ||
                            analysis.phase === 'pending'
                          ? 'text-amber-500'
                          : ''
                    }
                  >
                    {taskPhaseLabel(analysis.phase)}
                  </span>
                  <time dateTime={analysis.createdAt}>
                    {dateFormatter.format(new Date(analysis.createdAt))}
                  </time>
                </div>
              </div>

              <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs">
                {analysis.notesState === 'loading' ? (
                  <span role="status" className="text-muted-foreground">
                    ノートを読み込み中です。
                  </span>
                ) : analysis.notesState === 'error' ? (
                  <span role="alert" className="text-primary">
                    ノートを読み込めませんでした。
                  </span>
                ) : analysis.notes.length === 0 ? (
                  <span className="text-muted-foreground">
                    ノートはありません
                  </span>
                ) : (
                  analysis.notes.map((note) => (
                    <Link
                      key={note.id}
                      to="/notes/$noteId"
                      params={{ noteId: note.id }}
                      search={{ version_id: note.versionId }}
                      className="text-foreground underline decoration-border underline-offset-2 hover:text-primary"
                    >
                      {note.title}
                    </Link>
                  ))
                )}
                <Link
                  to="/strategies/$id/runs/$taskId"
                  params={{ id: analysis.strategyId, taskId: analysis.taskId }}
                  className="shrink-0 text-primary hover:underline"
                >
                  実行詳細 →
                </Link>
              </div>
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}
