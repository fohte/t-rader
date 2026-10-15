import { Popover, PopoverContent, PopoverTrigger } from '#components/ui/popover'

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
          <button
            type="button"
            aria-label={`${displayValue} の根拠を開く`}
            title="根拠を見る"
            className="inline-flex cursor-pointer items-baseline border-b border-dotted border-primary/60 bg-transparent p-0 font-mono font-medium tabular-nums text-foreground hover:border-primary hover:text-primary"
          >
            {displayValue}
          </button>
        }
      />
      <PopoverContent
        align="start"
        className="w-80 space-y-2 p-3 text-xs"
        style={{ maxWidth: 'calc(100vw - 1.5rem)' }}
      >
        <p className="font-mono font-semibold text-foreground">{title}</p>
        {state.status === 'loading' ? (
          <p role="status" className="text-muted-foreground">
            実行記録を読み込み中...
          </p>
        ) : state.status === 'error' ? (
          <p role="alert" className="text-primary">
            実行記録を取得できませんでした
          </p>
        ) : state.status === 'not-found' ? (
          <p role="status" className="text-muted-foreground">
            実行記録が見つかりません
          </p>
        ) : (
          <div className="space-y-2">
            <div
              className="grid gap-x-3 gap-y-1"
              style={{ gridTemplateColumns: '4rem minmax(0, 1fr)' }}
            >
              <span className="text-muted-foreground">実行時刻</span>
              <span className="font-mono tabular-nums">
                {state.evaluation.evaluated_at}
              </span>
              <span className="text-muted-foreground">出どころ</span>
              <span>自作指標の実行記録</span>
            </div>
            <div className="space-y-1">
              <p className="text-muted-foreground">入力</p>
              <pre className="max-h-36 overflow-auto whitespace-pre-wrap break-all border border-border bg-surface p-2 font-mono text-2xs">
                {formatJson(state.evaluation.args)}
              </pre>
            </div>
            <details className="border-t border-border pt-2">
              <summary className="cursor-pointer text-muted-foreground hover:text-foreground">
                コードを表示
              </summary>
              <pre className="mt-2 max-h-48 overflow-auto whitespace-pre-wrap break-all border border-border bg-surface p-2 font-mono text-2xs">
                {state.evaluation.code}
              </pre>
            </details>
          </div>
        )}
      </PopoverContent>
    </Popover>
  )
}
