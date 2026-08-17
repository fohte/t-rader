import { createFileRoute } from '@tanstack/react-router'

import { TaskRunListView } from '#components/strategy-shell/task-run-list-view'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/strategies/$id/runs/')({
  component: TaskRunListPage,
})

function TaskRunListPage() {
  const { id } = Route.useParams()
  const { data } = $api.useQuery('get', '/api/strategies/{id}/tasks', {
    params: { path: { id } },
  })

  return (
    <TaskRunListView
      strategyId={id}
      tasks={
        data?.map((t) => ({
          taskId: t.task_id,
          prompt: t.prompt,
          source: t.source,
          phase: t.phase,
          createdAt: t.created_at,
        })) ?? null
      }
    />
  )
}
