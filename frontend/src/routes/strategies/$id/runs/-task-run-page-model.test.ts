import { describe, expect, it } from 'vitest'

import {
  mapDefinitionSnapshots,
  mapDefinitionSnapshotsState,
  mapPayloadManifestPageState,
  mapPayloadRead,
  mapTaskRunTask,
  nextPayloadManifestPagination,
  payloadManifestPaginationForTask,
  previousPayloadManifestPagination,
} from '#routes/strategies/$id/runs/-task-run-page-model'

describe('task run page model', () => {
  it('maps the task into the history view', () => {
    expect(
      mapTaskRunTask({
        task_id: 'sample-task',
        strategy_id: 'sample-strategy',
        prompt: 'synthetic prompt',
        source: 'sample-source',
        phase: 'completed',
        created_at: '2030-01-02T03:04:05Z',
        updated_at: '2030-01-02T03:06:05Z',
        error_summary: null,
        model_call_limit_reached: false,
        steps: [],
      }),
    ).toEqual({
      taskId: 'sample-task',
      prompt: 'synthetic prompt',
      source: 'sample-source',
      phase: 'completed',
      createdAt: '2030-01-02T03:04:05Z',
      updatedAt: '2030-01-02T03:06:05Z',
      errorSummary: null,
    })
  })

  it('maps every saved definition snapshot into the history view', () => {
    expect(
      mapDefinitionSnapshots({
        task_id: 'sample-task',
        definition_snapshots: [
          {
            definition_key: 'task_start',
            task_id: 'sample-task',
            digest: 'synthetic-task-digest',
            created_at: '2030-01-02T03:04:05Z',
            definition: { graph: 'synthetic graph definition' },
          },
          {
            definition_key: 'custom_indicator:sample-indicator',
            task_id: 'sample-task',
            digest: 'synthetic-indicator-digest',
            created_at: '2030-01-02T03:04:06Z',
            definition: { code: 'synthetic code body' },
          },
        ],
      }),
    ).toEqual([
      {
        definitionKey: 'task_start',
        taskId: 'sample-task',
        digest: 'synthetic-task-digest',
        createdAt: '2030-01-02T03:04:05Z',
        definition: { graph: 'synthetic graph definition' },
      },
      {
        definitionKey: 'custom_indicator:sample-indicator',
        taskId: 'sample-task',
        digest: 'synthetic-indicator-digest',
        createdAt: '2030-01-02T03:04:06Z',
        definition: { code: 'synthetic code body' },
      },
    ])
  })

  it('maps a completed snapshot query to the ready state', () => {
    expect(mapDefinitionSnapshotsState(false, false)).toEqual('ready')
  })

  it('maps a bounded manifest page and selected payload to list state', () => {
    expect(
      mapPayloadManifestPageState(
        {
          task_id: 'sample-task',
          cursor: 'sample-cursor',
          next_cursor: 'sample-next-cursor',
          total_count: 7,
          items: [
            {
              id: 'sample-payload',
              task_id: 'sample-task',
              kind: 'input',
              source: 'sample-source',
              status: 'sealed',
              item_count: 3,
              byte_count: 24,
              digest: 'synthetic-payload-digest',
              incomplete_reason: null,
              created_at: '2030-01-02T03:04:07Z',
              chunk_count: 1,
              root_is_array: true,
              schema: {},
              source_metadata: {},
            },
          ],
        },
        {
          isPending: false,
          isError: false,
          selectedPayloadId: 'sample-payload',
          hasPreviousPage: true,
        },
      ),
    ).toEqual({
      status: 'ready',
      items: [
        {
          id: 'sample-payload',
          kind: 'input',
          source: 'sample-source',
          status: 'sealed',
          itemCount: 3,
          byteCount: 24,
          digest: 'synthetic-payload-digest',
          incompleteReason: null,
          createdAt: '2030-01-02T03:04:07Z',
        },
      ],
      totalCount: 7,
      nextCursor: 'sample-next-cursor',
      selectedPayloadId: 'sample-payload',
      hasPreviousPage: true,
    })
  })

  it('maps payload manifest and bounded page data without loading extra content', () => {
    expect(
      mapPayloadRead(
        {
          payloadId: 'sample-payload',
          label: 'input: sample-source',
          pointer: '/value/items',
          cursor: 0,
          limit: 2,
        },
        {
          manifest: {
            id: 'sample-payload',
            task_id: 'sample-task',
            kind: 'input',
            source: 'sample-source',
            status: 'sealed',
            item_count: 3,
            byte_count: 24,
            digest: 'synthetic-payload-digest',
            incomplete_reason: null,
            created_at: '2030-01-02T03:04:07Z',
            chunk_count: 2,
            root_is_array: false,
            schema: {},
            source_metadata: {},
          },
          page: {
            payload_id: 'sample-payload',
            pointer: '/value/items',
            cursor: 0,
            total_count: 3,
            next_cursor: 2,
            digest: 'synthetic-page-digest',
            data: [{ value: 'synthetic-a' }, { value: 'synthetic-b' }],
          },
        },
        false,
      ),
    ).toEqual({
      status: 'ready',
      manifest: {
        status: 'sealed',
        itemCount: 3,
        incompleteReason: null,
      },
      page: {
        data: [{ value: 'synthetic-a' }, { value: 'synthetic-b' }],
        totalCount: 3,
        nextCursor: 2,
      },
    })
  })

  it('restores the prior manifest page', () => {
    const initial = {
      taskId: 'sample-task-a',
      cursor: undefined,
      previousCursors: [] as (string | undefined)[],
    }
    const pageTwo = nextPayloadManifestPagination(
      initial,
      'sample-task-a',
      'sample-cursor-a',
    )
    const pageThree = nextPayloadManifestPagination(
      pageTwo,
      'sample-task-a',
      'sample-cursor-b',
    )
    expect(
      previousPayloadManifestPagination(pageThree, 'sample-task-a'),
    ).toEqual({
      taskId: 'sample-task-a',
      cursor: 'sample-cursor-a',
      previousCursors: [undefined],
    })
  })

  it('resets manifest cursor history when switching tasks', () => {
    const initial = {
      taskId: 'sample-task-a',
      cursor: undefined,
      previousCursors: [] as (string | undefined)[],
    }
    const pageTwo = nextPayloadManifestPagination(
      initial,
      'sample-task-a',
      'sample-cursor-a',
    )
    const pageThree = nextPayloadManifestPagination(
      pageTwo,
      'sample-task-a',
      'sample-cursor-b',
    )

    expect(
      payloadManifestPaginationForTask(pageThree, 'sample-task-b'),
    ).toEqual({
      taskId: 'sample-task-b',
      cursor: undefined,
      previousCursors: [],
    })
  })
})
