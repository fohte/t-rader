import { createFileRoute } from '@tanstack/react-router'
import { useMemo } from 'react'

import {
  parseAgentGraphPhases,
  readTaskSteps,
} from '#components/strategy-shell/task-execution-tree'
import { TaskRunView } from '#components/strategy-shell/task-run-view'
import { $api } from '#lib/api/client'

const POLL_INTERVAL_MS = 2000

export const Route = createFileRoute('/strategies/$id/runs/$taskId')({
  component: TaskRunPage,
})

function TaskRunPage() {
  const { id, taskId } = Route.useParams()

  const taskQuery = $api.useQuery(
    'get',
    '/api/strategies/{id}/tasks/{task_id}',
    { params: { path: { id, task_id: taskId } } },
    {
      refetchInterval: (query) => {
        const phase = query.state.data?.phase
        return phase === 'pending' || phase === 'running'
          ? POLL_INTERVAL_MS
          : false
      },
    },
  )
  const task = taskQuery.data
  const purpose = task?.purpose ?? null

  const purposeAgentGraphQuery = $api.useQuery(
    'get',
    '/api/agent-configs/{purpose}/agent-graph',
    { params: { path: { purpose: purpose ?? '' } } },
    { enabled: purpose != null },
  )
  const agentGraphContent = purposeAgentGraphQuery.data?.content
  const configQuery = $api.useQuery('get', '/api/config')
  const configPhases = useMemo(
    () => parseAgentGraphPhases(agentGraphContent ?? ''),
    [agentGraphContent],
  )

  const steps = readTaskSteps(task?.steps)

  const notesQuery = $api.useQuery(
    'get',
    '/api/strategies/{id}/tasks/{task_id}/notes',
    { params: { path: { id, task_id: taskId } } },
    {
      enabled: task != null,
      refetchInterval: () =>
        task?.phase === 'pending' || task?.phase === 'running'
          ? POLL_INTERVAL_MS
          : false,
    },
  )
  const generatedNotesCount = notesQuery.data?.length ?? 0

  return (
    <TaskRunView
      strategyId={id}
      task={
        task == null
          ? null
          : {
              taskId: task.task_id,
              prompt: task.prompt,
              source: task.source,
              phase: task.phase,
              createdAt: task.created_at,
              updatedAt: task.updated_at,
              errorSummary: task.error_summary ?? null,
            }
      }
      steps={steps}
      configPhases={configPhases}
      generatedNotesCount={generatedNotesCount}
      traceUrlTemplate={configQuery.data?.trace_url_template ?? undefined}
      taskLoadError={taskQuery.isError}
    />
  )
}
