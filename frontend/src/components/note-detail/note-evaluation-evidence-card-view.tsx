import { Button } from '@fohte/ui/button'

import { Popover, PopoverContent, PopoverTrigger } from '#components/ui/popover'
import { formatIngestDateTime } from '#lib/format-ingest-status'

interface NoteEvaluationEvidenceRecord {
  indicator_name: string
  args: unknown
  code: string
  evaluated_at: string
}

export type NoteEvaluationEvidenceState =
  | { status: 'loading' }
  | { status: 'error' }
  | { status: 'not-found' }
  | { status: 'ready'; evaluation: NoteEvaluationEvidenceRecord }

interface NoteEvaluationEvidenceCardViewProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  displayValue: string
  outputName: string
  state: NoteEvaluationEvidenceState
}

function formatJson(value: unknown): string {
  return JSON.stringify(value, null, 2)
}

export function NoteEvaluationEvidenceCardView({
  open,
  onOpenChange,
  displayValue,
  outputName,
  state,
}: NoteEvaluationEvidenceCardViewProps) {
  const title =
    state.status === 'ready'
      ? `${state.evaluation.indicator_name} · ${outputName}`
      : `評価結果 · ${outputName}`

  return (
    <Popover open={open} onOpenChange={onOpenChange}>
      <PopoverTrigger
        openOnHover
        render={
          <Button
            type="button"
            variant="plain"
            aria-label={`${displayValue} の根拠を開く`}
            title="根拠を見る"
            className="inline-flex items-baseline"
          >
            {displayValue}
          </Button>
        }
      />
      <PopoverContent
        align="start"
        padding="md"
        className="w-80 space-y-2 text-xs"
      >
        <p className="font-mono font-semibold text-foreground">{title}</p>
        {state.status === 'loading' ? (
          <p role="status" className="text-muted-foreground">
            実行記録を読み込み中...
          </p>
        ) : state.status === 'error' ? (
          <p role="alert" className="text-destructive">
            実行記録を取得できませんでした
          </p>
        ) : state.status === 'not-found' ? (
          <p role="status" className="text-muted-foreground">
            実行記録が見つかりません
          </p>
        ) : (
          <div className="space-y-2">
            <dl className="space-y-1">
              <div className="flex gap-x-3">
                <dt className="w-16 shrink-0 text-muted-foreground">
                  実行時刻
                </dt>
                <dd className="min-w-0 font-mono tabular-nums">
                  {formatIngestDateTime(state.evaluation.evaluated_at)}
                </dd>
              </div>
              <div className="flex gap-x-3">
                <dt className="w-16 shrink-0 text-muted-foreground">
                  出どころ
                </dt>
                <dd className="min-w-0">自作指標の実行記録</dd>
              </div>
            </dl>
            <div className="space-y-1">
              <p className="text-muted-foreground">入力</p>
              <pre className="max-h-36 overflow-auto whitespace-pre-wrap break-all border border-border bg-bg-tertiary p-2 font-mono text-2xs">
                {formatJson(state.evaluation.args)}
              </pre>
            </div>
            <details className="border-t border-border pt-2">
              <summary className="cursor-pointer text-muted-foreground hover:text-foreground">
                コードを表示
              </summary>
              <pre className="mt-2 max-h-48 overflow-auto whitespace-pre-wrap break-all border border-border bg-bg-tertiary p-2 font-mono text-2xs">
                {state.evaluation.code}
              </pre>
            </details>
          </div>
        )}
      </PopoverContent>
    </Popover>
  )
}
