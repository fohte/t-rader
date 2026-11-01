import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import {
  createMemoryHistory,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it } from 'vitest'

import { resetFetch, stubFetch } from '#test/stub-fetch.test-helper'

const strategyId = 'sample-strategy'
const taskAId = 'sample-task-a'
const taskBId = 'sample-task-b'
const payloadId = '00000000-0000-4000-8000-000000000011'

afterEach(() => {
  cleanup()
  resetFetch()
})

function taskResponse(taskId: string) {
  return {
    task_id: taskId,
    strategy_id: strategyId,
    prompt: taskId === taskAId ? '架空 task A の依頼' : '架空 task B の依頼',
    source: 'frontend',
    phase: 'completed',
    created_at: '2030-01-02T03:04:05Z',
    updated_at: '2030-01-02T03:05:05Z',
    error_summary: null,
    steps:
      taskId === taskAId
        ? [
            {
              phase_key: 'phase-a',
              execution_step_id: 'sample-step-a',
              label: '架空 task A step',
              model: null,
              kind: 'operation',
              status: 'completed',
              item: { marker: 'task-a-only-detail' },
              input_ref: {
                payload_inputs: [
                  {
                    payload_id: payloadId,
                    field: 'sample-field',
                    pointer: '/items',
                    cursor: 0,
                    limit: 2,
                  },
                ],
              },
              started_at: '2030-01-02T03:04:05Z',
              finished_at: '2030-01-02T03:04:06Z',
              trace_id: 'sample-trace-a',
              span_id: 'sample-span-a',
            },
          ]
        : [],
  }
}

function payloadResponse(taskId: string) {
  return {
    manifest: {
      id: payloadId,
      task_id: taskId,
      kind: 'input',
      source: 'sample-source',
      status: 'sealed',
      item_count: 2,
      byte_count: 32,
      digest: 'synthetic-payload-digest',
      incomplete_reason: null,
      created_at: '2030-01-02T03:04:06Z',
      chunk_count: 1,
      root_is_array: true,
      schema: {},
      source_metadata: {},
    },
    page: {
      payload_id: payloadId,
      pointer: '/items',
      cursor: 0,
      total_count: 2,
      next_cursor: null,
      digest: 'synthetic-page-digest',
      data: ['synthetic-a', 'synthetic-b'],
    },
  }
}

function setupFetch() {
  const payloadReads: { taskId: string; payloadId: string }[] = []
  stubFetch((input, init) => {
    const request = input instanceof Request ? input : new Request(input, init)
    const { pathname } = new URL(request.url)
    const taskMatch = pathname.match(
      /^\/api\/strategies\/[^/]+\/tasks\/([^/]+)$/,
    )
    const taskId = taskMatch?.[1]
    if (taskId != null) {
      return Promise.resolve(Response.json(taskResponse(taskId)))
    }
    const payloadMatch = pathname.match(
      /^\/api\/strategies\/[^/]+\/tasks\/([^/]+)\/execution-artifacts\/payloads\/([^/]+)$/,
    )
    const payloadTaskId = payloadMatch?.[1]
    const requestedPayloadId = payloadMatch?.[2]
    if (payloadTaskId != null && requestedPayloadId != null) {
      payloadReads.push({
        taskId: payloadTaskId,
        payloadId: requestedPayloadId,
      })
      return Promise.resolve(Response.json(payloadResponse(payloadTaskId)))
    }
    const artifactsMatch = pathname.match(
      /^\/api\/strategies\/[^/]+\/tasks\/([^/]+)\/execution-artifacts$/,
    )
    if (artifactsMatch != null) {
      return Promise.resolve(
        Response.json({ task_id: artifactsMatch[1], definition_snapshots: [] }),
      )
    }
    const manifestsMatch = pathname.match(
      /^\/api\/strategies\/[^/]+\/tasks\/([^/]+)\/execution-artifacts\/payloads$/,
    )
    if (manifestsMatch != null) {
      return Promise.resolve(
        Response.json({
          task_id: manifestsMatch[1],
          cursor: null,
          next_cursor: null,
          total_count: 0,
          items: [],
        }),
      )
    }
    if (pathname === '/api/config') {
      return Promise.resolve(Response.json({}))
    }
    return Promise.resolve(Response.json([]))
  })
  return payloadReads
}

describe('TaskRunPage', () => {
  it('task を切り替えると旧 payload と step 詳細を新 task に引き継がない', async () => {
    const payloadReads = setupFetch()
    const { routeTree } = await import('#routeTree.gen')
    const router = createRouter({
      routeTree,
      history: createMemoryHistory({
        initialEntries: [`/strategies/${strategyId}/runs/${taskAId}`],
      }),
    })
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    })
    render(
      <QueryClientProvider client={queryClient}>
        <RouterProvider router={router} />
      </QueryClientProvider>,
    )

    const user = userEvent.setup()
    await user.click(await screen.findByText('架空 task A step'))
    await screen.findByText('{ "marker": "task-a-only-detail" }')
    await user.click(screen.getByText('→ input: sample-fieldを開く'))
    await screen.findByText('[ "synthetic-a", "synthetic-b" ]')

    await router.navigate({
      to: '/strategies/$id/runs/$taskId',
      params: { id: strategyId, taskId: taskBId },
    })
    await screen.findByRole('heading', { name: '架空 task B の依頼' })

    const actual = {
      taskHeading: screen.getByRole('heading', { level: 1 }).textContent,
      oldStepDetail:
        screen.queryByText('{ "marker": "task-a-only-detail" }')?.textContent ??
        null,
      payloadReads,
      selectedPayload:
        screen.queryByRole('region', { name: '実行 payload' })?.textContent ??
        null,
    }
    expect(JSON.stringify(actual)).toBe(
      '{"taskHeading":"架空 task B の依頼","oldStepDetail":null,"payloadReads":[{"taskId":"sample-task-a","payloadId":"00000000-0000-4000-8000-000000000011"}],"selectedPayload":null}',
    )
  }, 30_000)
})
