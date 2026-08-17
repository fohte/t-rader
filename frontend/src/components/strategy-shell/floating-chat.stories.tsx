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
  const runRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/strategies/$id/runs/$taskId',
    component: () => null,
  })
  return createRouter({
    routeTree: rootRoute.addChildren([noteRoute, runRoute]),
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
    currentTaskId: null,
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

export const PollingWithRunLink: Story = {
  render: () => (
    <RouterProvider
      router={createStoryRouter(
        makeProps(
          { kind: 'polling', phase: 'running' },
          { input: 'SUMCO の足元評価', currentTaskId: 'T1' },
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
