import { $api } from '#lib/api/client'
import { type ExecutionOrigin, toExecutionStepId } from '#lib/execution-origin'

export function useExecutionOrigin(
  createdByKind: string | null | undefined,
  rawStepId: string | null | undefined,
): ExecutionOrigin | null {
  const stepId =
    createdByKind === 'llm' ? toExecutionStepId(rawStepId) : undefined
  const { data: stepOrigin } = $api.useQuery(
    'get',
    '/api/strategy-task-steps/{step_id}',
    { params: { path: { step_id: stepId ?? '' } } },
    { enabled: stepId != null },
  )
  const { data: strategies = [] } = $api.useQuery(
    'get',
    '/api/strategies',
    undefined,
    { enabled: stepOrigin != null },
  )
  const strategy = strategies.find(
    (candidate) => candidate.id === stepOrigin?.strategy_id,
  )

  return stepOrigin == null || strategy == null
    ? null
    : {
        strategyId: strategy.id,
        strategyName: strategy.name,
        taskId: stepOrigin.task_id,
      }
}
