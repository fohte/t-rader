import { Button } from '@fohte/ui/button'
import { Link } from '@tanstack/react-router'

import {
  type AgentGraphOutputSchema,
  buildTraceUrl,
  type ExecutionPayloadReference,
  findNoteId,
  formatDuration,
  listEnumEntries,
  listStepPayloadReferences,
  type TaskStep,
} from '#components/strategy-shell/task-execution-tree/model'

export function StepDetail({
  step,
  outputSchema,
  phaseDefinition,
  traceUrlTemplate,
  onSelectPayload,
}: {
  step: TaskStep
  outputSchema?: AgentGraphOutputSchema
  phaseDefinition?: Record<string, unknown>
  traceUrlTemplate?: string
  onSelectPayload?: (reference: ExecutionPayloadReference) => void
}): React.ReactElement {
  const traceUrl = buildTraceUrl(traceUrlTemplate, step.trace_id, step.span_id)
  const noteId = findNoteId(step.output)
  const duration = formatDuration(step.started_at, step.finished_at)
  const enumEntries = listEnumEntries(outputSchema, step.output)
  const payloadReferences = listStepPayloadReferences(step)
  const phaseKind =
    step.kind ??
    (phaseDefinition?.['kind'] === 'operation' ? 'operation' : undefined)

  return (
    <div className="mb-2 ml-4 space-y-2 border-l border-border py-1 pl-3 text-2xs text-muted-foreground-strong">
      <div className="grid grid-cols-(--grid-cols-step-detail) gap-x-2.5 gap-y-1">
        <span>フェーズ</span>
        <b className="font-semibold text-foreground">{step.label}</b>
        {phaseKind != null && (
          <>
            <span>種別</span>
            <b className="font-semibold text-foreground">
              {phaseKind === 'operation' &&
              typeof phaseDefinition?.['operation'] === 'string'
                ? phaseDefinition['operation']
                : phaseKind}
            </b>
          </>
        )}
        <span>モデル</span>
        <b className="font-semibold text-foreground">{step.model ?? '—'}</b>
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
      {step.input_ref !== undefined && (
        <JsonBlock label="input_ref" value={step.input_ref} />
      )}
      {step.output_ref !== undefined && (
        <JsonBlock label="output_ref" value={step.output_ref} />
      )}
      {phaseDefinition !== undefined && (
        <JsonBlock label="固定したフェーズ定義" value={phaseDefinition} />
      )}
      {onSelectPayload != null && payloadReferences.length > 0 && (
        <div className="flex flex-wrap gap-2">
          {payloadReferences.map((reference, index) => (
            <Button
              key={`${reference.payloadId}:${reference.pointer}:${String(index)}`}
              type="button"
              variant="plain"
              size="sm"
              onClick={() => {
                onSelectPayload(reference)
              }}
            >
              → {reference.label}を開く
            </Button>
          ))}
        </div>
      )}
      {step.status === 'failed' && step.error != null && step.error !== '' && (
        <pre className="whitespace-pre-wrap text-destructive">{step.error}</pre>
      )}
      {noteId != null && (
        <Link
          to="/notes/$noteId"
          params={{ noteId }}
          className="block text-foreground hover:underline"
        >
          → ノートを開く
        </Link>
      )}
      {traceUrl != null && (
        <a
          href={traceUrl}
          target="_blank"
          rel="noreferrer"
          className="block text-foreground hover:underline"
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
