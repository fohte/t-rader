import type {
  AgentGraphPhaseSummary,
  TaskStep,
} from '#components/strategy-shell/task-execution-tree'

export const CONFIG_PHASES: AgentGraphPhaseSummary[] = [
  { key: 'prepare', label: '準備一覧', model: 'simulated-model-amber' },
  {
    key: 'inspect',
    label: '項目確認',
    model: 'simulated-model-teal',
    output: {
      result: { enum: ['consistent', 'needs_review', 'undetermined'] },
      details: { type: 'string' },
      note_id: { type: 'string' },
    },
  },
  { key: 'finalize', label: '結果整理', model: 'simulated-model-violet' },
]

export const PREPARATION_STEP: TaskStep = {
  phase_key: 'prepare',
  execution_step_id: 'exec-prepare-0001',
  label: '準備一覧',
  model: 'simulated-model-amber',
  status: 'completed',
  output: {
    items: ['受付番号の形式', '見出しの表記ゆれ', '項目間の重複'],
  },
  started_at: '2026-08-15T09:00:00.000Z',
  finished_at: '2026-08-15T09:00:12.400Z',
  trace_id: 'trace-prepare-0001',
  span_id: 'span-prepare-0001',
}

export const COMPLETED_STEP: TaskStep = {
  phase_key: 'sample-phase',
  execution_step_id: 'sample-step-001',
  label: 'サンプル処理',
  model: 'simulated-model-cobalt',
  status: 'completed',
  item: { record: 'sample-row' },
  item_label: 'サンプル項目',
  output: { state: 'sample-result', note_id: 'sample-note-001' },
  started_at: '2026-08-15T09:00:00.000Z',
  finished_at: '2026-08-15T09:00:08.000Z',
  trace_id: 'sample-trace-001',
  span_id: 'sample-span-001',
}

export function inspectStep(
  title: string,
  {
    status,
    finishedAt,
    result,
    noteId,
  }: {
    status: TaskStep['status']
    finishedAt?: string
    result?: 'consistent' | 'needs_review' | 'undetermined'
    noteId?: string
  },
): TaskStep {
  return {
    phase_key: 'inspect',
    execution_step_id: `exec-inspect-${title}`,
    label: '項目確認',
    model: 'simulated-model-teal',
    status,
    item: { title },
    item_label: title,
    output:
      status === 'completed'
        ? {
            result: result ?? 'consistent',
            details: `${title}を確認した結果`,
            ...(noteId != null ? { note_id: noteId } : {}),
          }
        : undefined,
    started_at: '2026-08-15T09:00:13.000Z',
    finished_at: finishedAt,
    trace_id: `trace-inspect-${title}`,
    span_id: `span-inspect-${title}`,
  }
}
