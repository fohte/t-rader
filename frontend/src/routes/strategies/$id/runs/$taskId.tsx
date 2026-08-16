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

  const agentGraphQuery = $api.useQuery(
    'get',
    '/api/strategies/{id}/agent-graph',
    { params: { path: { id } } },
  )
  const configQuery = $api.useQuery('get', '/api/config')
  const configPhases = useMemo(
    () => parseAgentGraphPhases(agentGraphQuery.data?.content ?? ''),
    [agentGraphQuery.data?.content],
  )

  const steps = readTaskSteps(task?.steps)

  const notesQuery = $api.useQuery(
    'get',
    '/api/notes',
    { params: { query: { strategy_id: id } } },
    { enabled: task != null },
  )
  // floating-chat.tsx の generatedNotes とは別実装。あちらはクライアント時刻起点 +
  // skew 許容 (継続中タスクの投入直後を拾うため)。ここは backend が確定させた
  // created_at/updated_at のみで完結する境界指定で足りる。
  const generatedNotesCount = useMemo(() => {
    if (task == null) return 0
    return (notesQuery.data ?? []).filter(
      (n) => n.created_at >= task.created_at && n.created_at <= task.updated_at,
    ).length
  }, [task, notesQuery.data])

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
