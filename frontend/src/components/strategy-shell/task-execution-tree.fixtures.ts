import type {
  AgentGraphPhaseSummary,
  TaskStep,
} from '#components/strategy-shell/task-execution-tree'

export const CONFIG_PHASES: AgentGraphPhaseSummary[] = [
  { key: 'plan', label: '調査計画', model: 'claude-opus-4' },
  {
    key: 'investigate',
    label: '仮説の調査',
    model: 'deepseek-v4-flash',
    output: {
      verdict: { enum: ['supported', 'rejected', 'inconclusive'] },
      summary: { type: 'string' },
      note_id: { type: 'string' },
    },
  },
  { key: 'merge', label: '統合', model: 'claude-sonnet-4' },
]

export const PLAN_STEP: TaskStep = {
  phase_key: 'plan',
  label: '調査計画',
  model: 'claude-opus-4',
  status: 'completed',
  output: {
    items: ['円安の進行が主因', '半導体サイクルの反転', '個別の材料出尽くし'],
  },
  started_at: '2026-08-15T09:00:00.000Z',
  finished_at: '2026-08-15T09:00:12.400Z',
  trace_id: 'trace-plan-0001',
  span_id: 'span-plan-0001',
}

export function investigateStep(
  title: string,
  {
    status,
    finishedAt,
    verdict,
    noteId,
  }: {
    status: TaskStep['status']
    finishedAt?: string
    verdict?: 'supported' | 'rejected' | 'inconclusive'
    noteId?: string
  },
): TaskStep {
  return {
    phase_key: 'investigate',
    label: '仮説の調査',
    model: 'deepseek-v4-flash',
    status,
    item: { title },
    item_label: title,
    output:
      status === 'completed'
        ? {
            verdict: verdict ?? 'supported',
            summary: `${title}を検証した結果`,
            ...(noteId != null ? { note_id: noteId } : {}),
          }
        : undefined,
    started_at: '2026-08-15T09:00:13.000Z',
    finished_at: finishedAt,
    trace_id: `trace-investigate-${title}`,
    span_id: `span-investigate-${title}`,
  }
}
