import { Link } from '@tanstack/react-router'

import {
  buildTraceUrl,
  type TaskStep,
} from '#components/strategy-shell/task-execution-tree/model'

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null
}

// output に note_id (文字列) があればノートへのリンクを出す。write_note が返した id を
// そのまま output に含める、という設定側の慣習を前提にした構造的な検出。
function findNoteId(output: unknown): string | null {
  if (!isRecord(output)) return null
  const value = output['note_id']
  return typeof value === 'string' && value !== '' ? value : null
}

export function StepDetail({
  strategyId,
  step,
  traceUrlTemplate,
}: {
  strategyId: string
  step: TaskStep
  traceUrlTemplate?: string
}): React.ReactElement {
  const traceUrl = buildTraceUrl(traceUrlTemplate, step.trace_id, step.span_id)
  const noteId = findNoteId(step.output)

  return (
    <div className="mb-2 ml-4 space-y-2 border-l border-[color:var(--color-border-strategy)] py-1 pl-3 text-[11px] text-[color:var(--color-text-secondary)]">
      {step.item !== undefined && <JsonBlock label="input" value={step.item} />}
      {step.output !== undefined && (
        <JsonBlock label="output" value={step.output} />
      )}
      {step.status === 'failed' && step.error != null && step.error !== '' && (
        <pre className="whitespace-pre-wrap">{step.error}</pre>
      )}
      {noteId != null && (
        <Link
          to="/strategies/$id/notes/$noteId"
          params={{ id: strategyId, noteId }}
          className="block text-[color:var(--color-accent-strategy)] hover:underline"
        >
          → ノートを開く
        </Link>
      )}
      {traceUrl != null && (
        <a
          href={traceUrl}
          target="_blank"
          rel="noreferrer"
          className="block text-[color:var(--color-accent-strategy)] hover:underline"
        >
          → トレースを開く
        </a>
      )}
    </div>
  )
}

function JsonBlock({
  label,
  value,
}: {
  label: string
  value: unknown
}): React.ReactElement {
  return (
    <div>
      <div className="mb-1 text-[10px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
        {label}
      </div>
      <pre className="overflow-x-auto whitespace-pre-wrap break-all">
        {JSON.stringify(value, null, 2)}
      </pre>
    </div>
  )
}
