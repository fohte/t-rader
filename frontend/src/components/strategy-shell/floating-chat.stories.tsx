import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import {
  type FloatingChatStatus,
  FloatingChatView,
  type FloatingChatViewProps,
} from '#components/strategy-shell/floating-chat-view'
import { createStoryRouter } from '#storybook/story-router'

const NOOP = (): void => {}

function createFloatingChatRouter(props: FloatingChatViewProps) {
  return createStoryRouter(
    () => (
      <div className="h-screen bg-background p-4">
        <p className="font-mono text-sm text-muted-foreground-strong">
          right-bottom: floating chat preview
        </p>
        <FloatingChatView {...props} />
      </div>
    ),
    {
      paths: ['/strategies/$id/notes/$noteId', '/strategies/$id/runs/$taskId'],
    },
  )
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
      router={createFloatingChatRouter(
        makeProps({ kind: 'idle' }, { open: false }),
      )}
    />
  ),
}

export const Idle: Story = {
  render: () => (
    <RouterProvider
      router={createFloatingChatRouter(makeProps({ kind: 'idle' }))}
    />
  ),
}

export const Polling: Story = {
  render: () => (
    <RouterProvider
      router={createFloatingChatRouter(
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
      router={createFloatingChatRouter(
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
      router={createFloatingChatRouter(
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
      router={createFloatingChatRouter(
        makeProps({ kind: 'failed', error_summary: 'agent crashed: timeout' }),
      )}
    />
  ),
}

export const Error: Story = {
  render: () => (
    <RouterProvider
      router={createFloatingChatRouter(
        makeProps({
          kind: 'error',
          message: '戦略 Agent が ready ではありません',
        }),
      )}
    />
  ),
}
