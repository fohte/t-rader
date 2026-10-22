import { createFileRoute } from '@tanstack/react-router'
import { useEffect, useMemo, useState } from 'react'

import {
  type ExecutionPayloadReference,
  parseAgentGraphPhases,
  readTaskStartAgentGraph,
  readTaskSteps,
} from '#components/strategy-shell/task-execution-tree'
import { TaskRunView } from '#components/strategy-shell/task-run-view'
import { $api } from '#lib/api/client'

const POLL_INTERVAL_MS = 2000

function isTaskActive(phase: string | undefined): boolean {
  return phase === 'pending' || phase === 'running'
}

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
        return isTaskActive(phase) ? POLL_INTERVAL_MS : false
      },
    },
  )
  const task = taskQuery.data
  const taskPhase = task?.phase

  const artifactsQuery = $api.useQuery(
    'get',
    '/api/strategies/{id}/tasks/{task_id}/execution-artifacts',
    { params: { path: { id, task_id: taskId } } },
    {
      enabled: task != null,
      refetchInterval: () =>
        isTaskActive(taskPhase) ? POLL_INTERVAL_MS : false,
    },
  )
  const configQuery = $api.useQuery('get', '/api/config')
  const [selectedPayload, setSelectedPayload] =
    useState<ExecutionPayloadReference | null>(null)
  const payloadQuery = $api.useQuery(
    'get',
    '/api/strategies/{id}/tasks/{task_id}/execution-artifacts/payloads/{payload_id}',
    {
      params: {
        path: {
          id,
          task_id: taskId,
          payload_id: selectedPayload?.payloadId ?? '',
        },
        query: {
          pointer: selectedPayload?.pointer ?? '',
          cursor: selectedPayload?.cursor ?? 0,
          ...(selectedPayload?.limit == null
            ? {}
            : { limit: selectedPayload.limit }),
        },
      },
    },
    {
      enabled: selectedPayload != null,
      refetchInterval: () =>
        selectedPayload != null && isTaskActive(taskPhase)
          ? POLL_INTERVAL_MS
          : false,
    },
  )
  const artifacts = artifactsQuery.data
  const taskStartSnapshot = artifacts?.definition_snapshots.find(
    (snapshot) => snapshot.definition_key === 'task_start',
  )
  const agentGraphContent = readTaskStartAgentGraph(
    taskStartSnapshot?.definition,
  )
  const configPhases = useMemo(
    () => parseAgentGraphPhases(agentGraphContent ?? ''),
    [agentGraphContent],
  )
  const payload = payloadQuery.data
  const executionDefinition =
    taskStartSnapshot == null
      ? {
          status: artifactsQuery.isPending
            ? ('loading' as const)
            : artifactsQuery.isError
              ? ('error' as const)
              : ('missing' as const),
        }
      : {
          status: 'ready' as const,
          digest: taskStartSnapshot.digest,
          createdAt: taskStartSnapshot.created_at,
          definition: taskStartSnapshot.definition,
        }
  const payloadRead =
    selectedPayload == null
      ? undefined
      : payload == null
        ? {
            status: payloadQuery.isError
              ? ('error' as const)
              : ('loading' as const),
          }
        : {
            status: 'ready' as const,
            manifest: {
              status: payload.manifest.status,
              itemCount: payload.manifest.item_count,
              incompleteReason: payload.manifest.incomplete_reason ?? null,
            },
            page:
              payload.page == null
                ? null
                : {
                    data: payload.page.data,
                    totalCount: payload.page.total_count,
                    nextCursor: payload.page.next_cursor ?? null,
                  },
          }

  const steps = readTaskSteps(task?.steps)

  const notesQuery = $api.useQuery(
    'get',
    '/api/strategies/{id}/tasks/{task_id}/notes',
    { params: { path: { id, task_id: taskId } } },
    {
      enabled: task != null,
      refetchInterval: () =>
        isTaskActive(taskPhase) ? POLL_INTERVAL_MS : false,
    },
  )
  useEffect(() => {
    if (taskPhase == null || isTaskActive(taskPhase)) return
    void notesQuery.refetch()
  }, [taskPhase, notesQuery.refetch])

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
      executionDefinition={executionDefinition}
      selectedPayload={selectedPayload}
      payloadRead={payloadRead}
      onSelectPayload={(reference) => {
        setSelectedPayload({
          ...reference,
          cursor: reference.cursor ?? 0,
        })
      }}
      onNextPayloadPage={(cursor) => {
        setSelectedPayload((current) =>
          current == null ? current : { ...current, cursor },
        )
      }}
      notes={notesQuery.data ?? []}
      notesState={
        notesQuery.isPending
          ? 'loading'
          : notesQuery.isError
            ? 'error'
            : 'success'
      }
      traceUrlTemplate={configQuery.data?.trace_url_template ?? undefined}
      taskLoadError={taskQuery.isError}
    />
  )
}
