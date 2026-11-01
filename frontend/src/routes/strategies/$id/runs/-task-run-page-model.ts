import type { ExecutionPayloadManifestPageState } from '#components/strategy-shell/execution-payload-manifest-list'
import type { ExecutionPayloadReference } from '#components/strategy-shell/task-execution-tree'
import type { TaskRunDefinitionSnapshot } from '#components/strategy-shell/task-run-definition-snapshot-panel'
import type { TaskRunViewProps } from '#components/strategy-shell/task-run-view'
import type { components } from '#lib/api/schema.gen'

type StrategyTaskResponse = components['schemas']['StrategyTaskStatusResponse']
type ExecutionArtifactsResponse =
  components['schemas']['ExecutionArtifactsResponse']
type ExecutionPayloadManifestPageResponse =
  components['schemas']['ExecutionPayloadManifestPageResponse']
type ExecutionPayloadResponse =
  components['schemas']['ExecutionPayloadResponse']

export interface PayloadManifestPagination {
  taskId: string
  cursor: string | undefined
  previousCursors: (string | undefined)[]
}

export function payloadManifestPaginationForTask(
  pagination: PayloadManifestPagination,
  taskId: string,
): PayloadManifestPagination {
  return pagination.taskId === taskId
    ? pagination
    : { taskId, cursor: undefined, previousCursors: [] }
}

export function nextPayloadManifestPagination(
  pagination: PayloadManifestPagination,
  taskId: string,
  cursor: string,
): PayloadManifestPagination {
  const current = payloadManifestPaginationForTask(pagination, taskId)
  return {
    taskId,
    cursor,
    previousCursors: [...current.previousCursors, current.cursor],
  }
}

export function previousPayloadManifestPagination(
  pagination: PayloadManifestPagination,
  taskId: string,
): PayloadManifestPagination {
  const current = payloadManifestPaginationForTask(pagination, taskId)
  if (current.previousCursors.length === 0) return current

  const previousCursors = [...current.previousCursors]
  const cursor = previousCursors.pop()
  return { taskId, cursor, previousCursors }
}

export function mapTaskRunTask(
  task: StrategyTaskResponse | undefined,
): TaskRunViewProps['task'] {
  return task == null
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

export function mapDefinitionSnapshots(
  artifacts: ExecutionArtifactsResponse | undefined,
): TaskRunDefinitionSnapshot[] {
  return (artifacts?.definition_snapshots ?? []).map((snapshot) => ({
    definitionKey: snapshot.definition_key,
    taskId: snapshot.task_id,
    digest: snapshot.digest,
    createdAt: snapshot.created_at,
    definition: snapshot.definition,
  }))
}

export function mapDefinitionSnapshotsState(
  isPending: boolean,
  isError: boolean,
): NonNullable<TaskRunViewProps['definitionSnapshotsState']> {
  return isPending ? 'loading' : isError ? 'error' : 'ready'
}

export function mapPayloadManifestPageState(
  page: ExecutionPayloadManifestPageResponse | undefined,
  options: {
    isPending: boolean
    isError: boolean
    selectedPayloadId: string | null
    hasPreviousPage: boolean
  },
): ExecutionPayloadManifestPageState {
  if (options.isPending) return { status: 'loading' }
  if (options.isError) return { status: 'error' }

  return {
    status: 'ready',
    items: (page?.items ?? []).map((manifest) => ({
      id: manifest.id,
      kind: manifest.kind,
      source: manifest.source,
      status: manifest.status,
      itemCount: manifest.item_count,
      byteCount: manifest.byte_count,
      digest: manifest.digest ?? null,
      incompleteReason: manifest.incomplete_reason ?? null,
      createdAt: manifest.created_at,
    })),
    totalCount: page?.total_count ?? 0,
    nextCursor: page?.next_cursor ?? null,
    selectedPayloadId: options.selectedPayloadId,
    hasPreviousPage: options.hasPreviousPage,
  }
}

export function mapPayloadRead(
  selectedPayload: ExecutionPayloadReference | null,
  payload: ExecutionPayloadResponse | undefined,
  isError: boolean,
): NonNullable<TaskRunViewProps['payloadRead']> | undefined {
  if (selectedPayload == null) return undefined
  if (payload == null) return { status: isError ? 'error' : 'loading' }

  return {
    status: 'ready',
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
}
