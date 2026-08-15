import type { Meta, StoryObj } from '@storybook/react-vite'
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router'

import {
  type FloatingChatStatus,
  FloatingChatView,
  type FloatingChatViewProps,
} from '#components/strategy-shell/floating-chat-view'

const NOOP = (): void => {}

function createStoryRouter(props: FloatingChatViewProps) {
  const rootRoute = createRootRoute({
    component: () => (
      <div className="h-screen bg-[color:var(--color-bg-primary)] p-4">
        <p className="font-mono text-sm text-[color:var(--color-text-secondary)]">
          right-bottom: floating chat preview
        </p>
        <FloatingChatView {...props} />
      </div>
    ),
  })
  const noteRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/strategies/$id/notes/$noteId',
    component: () => null,
  })
  return createRouter({
    routeTree: rootRoute.addChildren([noteRoute]),
    history: createMemoryHistory({ initialEntries: ['/'] }),
  })
}

function makeProps(
  status: FloatingChatStatus,
  overrides: Partial<FloatingChatViewProps> = {},
): FloatingChatViewProps {
  return {
    open: true,
    strategyId: 'semi-swing',
    seed: null,
    input: '',
    status,
    notes: [],
    steps: [],
    configPhases: [],
    onOpen: NOOP,
    onClose: NOOP,
    onInputChange: NOOP,
    onSubmit: NOOP,
    ...overrides,
  }
}

const meta = {
  title: 'StrategyShell/FloatingChat',
  parameters: { layout: 'fullscreen' },
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

export const Closed: Story = {
  render: () => (
    <RouterProvider
      router={createStoryRouter(makeProps({ kind: 'idle' }, { open: false }))}
    />
  ),
}

export const Idle: Story = {
  render: () => (
    <RouterProvider router={createStoryRouter(makeProps({ kind: 'idle' }))} />
  ),
}

export const Polling: Story = {
  render: () => (
    <RouterProvider
      router={createStoryRouter(
        makeProps(
          { kind: 'polling', phase: 'running' },
          { input: 'SUMCO の足元評価' },
        ),
      )}
    />
  ),
}

export const PollingWithSteps: Story = {
  render: () => (
    <RouterProvider
      router={createStoryRouter(
        makeProps(
          { kind: 'polling', phase: 'running' },
          {
            input: 'SUMCO の足元評価',
            configPhases: [
              { key: 'plan', label: '調査計画', model: 'claude-opus-4' },
              {
                key: 'investigate',
                label: '個別調査',
                model: 'deepseek-v4-flash',
              },
              { key: 'merge', label: '統合', model: 'claude-sonnet-4' },
            ],
            steps: [
              {
                phase_key: 'plan',
                label: '調査計画',
                model: 'claude-opus-4',
                status: 'completed',
                started_at: '2026-06-26T07:00:00.000Z',
                finished_at: '2026-06-26T07:00:12.400Z',
                trace_id: 'trace-plan-0001',
                span_id: 'span-plan-0001',
              },
              {
                phase_key: 'investigate',
                label: '個別調査',
                model: 'deepseek-v4-flash',
                status: 'completed',
                item: { title: '為替影響の再評価' },
                item_label: '為替影響の再評価',
                output: { conclusion: '影響は限定的' },
                started_at: '2026-06-26T07:00:12.400Z',
                finished_at: '2026-06-26T07:00:20.500Z',
                trace_id: 'a1b2c3',
                span_id: 'd4e5f6',
              },
              {
                phase_key: 'investigate',
                label: '個別調査',
                model: 'deepseek-v4-flash',
                status: 'running',
                item: { title: '需給要因の点検' },
                item_label: '需給要因の点検',
                started_at: '2026-06-26T07:00:20.500Z',
                trace_id: 'trace-investigate-0002',
                span_id: 'span-investigate-0002',
              },
            ],
          },
        ),
      )}
    />
  ),
}

export const Completed: Story = {
  render: () => (
    <RouterProvider
      router={createStoryRouter(
        makeProps(
          { kind: 'completed' },
          {
            notes: [
              {
                id: 'N1',
                title: 'SUMCO レンジ回帰の確度評価',
                updated_at: '2026-06-26T07:00:00Z',
              },
            ],
          },
        ),
      )}
    />
  ),
}

export const Failed: Story = {
  render: () => (
    <RouterProvider
      router={createStoryRouter(
        makeProps({ kind: 'failed', error_summary: 'agent crashed: timeout' }),
      )}
    />
  ),
}

export const Error: Story = {
  render: () => (
    <RouterProvider
      router={createStoryRouter(
        makeProps({
          kind: 'error',
          message: '戦略 Agent が ready ではありません',
        }),
      )}
    />
  ),
}
