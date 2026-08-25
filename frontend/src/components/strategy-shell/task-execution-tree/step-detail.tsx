import { Link } from '@tanstack/react-router'

import {
  type AgentGraphOutputSchema,
  buildTraceUrl,
  findNoteId,
  formatDuration,
  listEnumEntries,
  type TaskStep,
} from '#components/strategy-shell/task-execution-tree/model'

export function StepDetail({
  strategyId,
  step,
  outputSchema,
  traceUrlTemplate,
}: {
  strategyId: string
  step: TaskStep
  outputSchema?: AgentGraphOutputSchema
  traceUrlTemplate?: string
}): React.ReactElement {
  const traceUrl = buildTraceUrl(traceUrlTemplate, step.trace_id, step.span_id)
  const noteId = findNoteId(step.output)
  const duration = formatDuration(step.started_at, step.finished_at)
  const enumEntries = listEnumEntries(outputSchema, step.output)

  return (
    <div className="mb-2 ml-4 space-y-2 border-l border-border py-1 pl-3 text-2xs text-muted-foreground-strong">
      <div className="grid grid-cols-(--grid-cols-step-detail) gap-x-2.5 gap-y-1">
        <span>フェーズ</span>
        <b className="font-semibold text-foreground">{step.label}</b>
        <span>モデル</span>
        <b className="font-semibold text-foreground">{step.model}</b>
        <span>所要</span>
        <b className="font-semibold text-foreground">{duration ?? '—'}</b>
        {enumEntries.flatMap((entry) => [
          <span key={`${entry.label}-label`}>{entry.label}</span>,
          <b
            key={`${entry.label}-value`}
            className="font-semibold text-foreground"
          >
            {entry.value}
          </b>,
        ])}
      </div>
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
          className="block text-primary hover:underline"
        >
          → ノートを開く
        </Link>
      )}
      {traceUrl != null && (
        <a
          href={traceUrl}
          target="_blank"
          rel="noreferrer"
          className="block text-primary hover:underline"
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
      <div className="mb-1 text-2xs uppercase tracking-wider text-muted-foreground">
        {label}
      </div>
      <pre className="overflow-x-auto whitespace-pre-wrap break-all">
        {JSON.stringify(value, null, 2)}
      </pre>
    </div>
  )
}
