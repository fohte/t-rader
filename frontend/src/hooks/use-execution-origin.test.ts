import { cleanup, renderHook } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { useExecutionOrigin } from '#hooks/use-execution-origin'
import type { ExecutionOrigin } from '#lib/execution-origin'

const { useQueryMock } = vi.hoisted(() => ({ useQueryMock: vi.fn() }))

vi.mock('#lib/api/client', () => ({ $api: { useQuery: useQueryMock } }))

const stepQueryPath = '/api/strategy-task-steps/{step_id}'
const stepOrigin = {
  strategy_id: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
  task_id: 'cccccccc-cccc-4ccc-8ccc-cccccccccccc',
}
const strategy = { id: stepOrigin.strategy_id, name: '架空戦略' }
const resolvedOrigin: ExecutionOrigin = {
  strategyId: strategy.id,
  strategyName: strategy.name,
  taskId: stepOrigin.task_id,
}

afterEach(() => {
  cleanup()
  useQueryMock.mockReset()
})

describe('useExecutionOrigin', () => {
  it.each([
    {
      state: 'the step and strategy resolve',
      stepQuery: { data: stepOrigin, isPending: false, isError: false },
      strategyQuery: {
        data: [strategy],
        isPending: false,
        isError: false,
      },
      expected: resolvedOrigin,
    },
    {
      state: 'the step endpoint returns 404',
      stepQuery: {
        data: undefined,
        error: { status: 404 },
        isPending: false,
        isError: true,
      },
      strategyQuery: { data: [], isPending: false, isError: false },
      expected: null,
    },
    {
      state: 'the strategy is absent from the strategy list',
      stepQuery: { data: stepOrigin, isPending: false, isError: false },
      strategyQuery: { data: [], isPending: false, isError: false },
      expected: null,
    },
    {
      state: 'the step request is pending',
      stepQuery: { data: undefined, isPending: true, isError: false },
      strategyQuery: { data: undefined, isPending: true, isError: false },
      expected: null,
    },
    {
      state: 'the strategy list is pending',
      stepQuery: { data: stepOrigin, isPending: false, isError: false },
      strategyQuery: { data: undefined, isPending: true, isError: false },
      expected: null,
    },
  ])(
    'returns the expected origin when $state',
    ({ stepQuery, strategyQuery, expected }) => {
      useQueryMock.mockImplementation((_method: string, path: string) =>
        path === stepQueryPath ? stepQuery : strategyQuery,
      )

      const { result } = renderHook(() =>
        useExecutionOrigin('llm', 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa'),
      )

      expect(result.current).toEqual(expected)
    },
  )
})
