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
import {
  mapDefinitionSnapshots,
  mapDefinitionSnapshotsState,
  mapPayloadManifestPageState,
  mapPayloadRead,
  mapTaskRunTask,
  nextPayloadManifestPagination,
  type PayloadManifestPagination,
  payloadManifestPaginationForTask,
  previousPayloadManifestPagination,
} from '#routes/strategies/$id/runs/-task-run-page-model'

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
  const [payloadManifestPagination, setPayloadManifestPagination] =
    useState<PayloadManifestPagination>(() => ({
      taskId,
      cursor: undefined,
      previousCursors: [],
    }))
  const activePayloadManifestPagination = payloadManifestPaginationForTask(
    payloadManifestPagination,
    taskId,
  )
  useEffect(() => {
    setPayloadManifestPagination((current) =>
      payloadManifestPaginationForTask(current, taskId),
    )
  }, [taskId])
  const payloadManifestsQuery = $api.useQuery(
    'get',
    '/api/strategies/{id}/tasks/{task_id}/execution-artifacts/payloads',
    {
      params: {
        path: { id, task_id: taskId },
        query:
          activePayloadManifestPagination.cursor == null
            ? {}
            : { cursor: activePayloadManifestPagination.cursor },
      },
    },
    {
      enabled: task != null,
      refetchInterval: () =>
        isTaskActive(taskPhase) ? POLL_INTERVAL_MS : false,
    },
  )
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
  const payloadManifestState = mapPayloadManifestPageState(
    payloadManifestsQuery.data,
    {
      isPending: payloadManifestsQuery.isPending,
      isError: payloadManifestsQuery.isError,
      selectedPayloadId: selectedPayload?.payloadId ?? null,
      hasPreviousPage:
        activePayloadManifestPagination.previousCursors.length > 0,
    },
  )
  const definitionSnapshots = mapDefinitionSnapshots(artifacts)
  const definitionSnapshotsState = mapDefinitionSnapshotsState(
    artifactsQuery.isPending,
    artifactsQuery.isError,
  )
  const payloadRead = mapPayloadRead(
    selectedPayload,
    payloadQuery.data,
    payloadQuery.isError,
  )

  const taskViewTask = mapTaskRunTask(task)
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
      task={taskViewTask}
      steps={steps}
      configPhases={configPhases}
      definitionSnapshots={definitionSnapshots}
      definitionSnapshotsState={definitionSnapshotsState}
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
      executionPayloadManifests={payloadManifestState}
      onSelectPayloadManifest={(manifest) => {
        setSelectedPayload({
          payloadId: manifest.id,
          label: `${manifest.kind}: ${manifest.source}`,
          pointer: '',
          cursor: 0,
        })
      }}
      onNextPayloadManifestPage={(cursor) => {
        setPayloadManifestPagination((current) =>
          nextPayloadManifestPagination(current, taskId, cursor),
        )
      }}
      onPreviousPayloadManifestPage={() => {
        setPayloadManifestPagination((current) =>
          previousPayloadManifestPagination(current, taskId),
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
