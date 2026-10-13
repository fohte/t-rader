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
      paths: ['/notes/$noteId', '/strategies/$id/runs/$taskId'],
    },
  )
}

function makeProps(
  status: FloatingChatStatus,
  overrides: Partial<FloatingChatViewProps> = {},
): FloatingChatViewProps {
  return {
    open: true,
    strategyId: 'example-strategy-a',
    taskStrategyId: null,
    strategies: [
      { id: 'example-strategy-a', name: '例示戦略 A' },
      { id: 'example-strategy-b', name: '例示戦略 B' },
    ],
    submittedPrompt: null,
    seed: null,
    input: '',
    resultText: null,
    status,
    notes: [],
    purposes: ['example-purpose', 'another-example-purpose'],
    selectedPurpose: '',
    currentTaskId: null,
    onOpen: NOOP,
    onClose: NOOP,
    onInputChange: NOOP,
    onStrategyChange: NOOP,
    onPurposeChange: NOOP,
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
  name: 'keeps the floating chat panel closed.',
  render: () => (
    <RouterProvider
      router={createFloatingChatRouter(
        makeProps({ kind: 'idle' }, { open: false }),
      )}
    />
  ),
}

export const Idle: Story = {
  name: 'shows the open chat panel before a prompt is entered.',
  render: () => (
    <RouterProvider
      router={createFloatingChatRouter(makeProps({ kind: 'idle' }))}
    />
  ),
}

export const MobileViewport: Story = {
  name: 'fills the phone viewport when the chat is open.',
  globals: { viewport: { value: 'mobileChat' } },
  parameters: {
    viewport: {
      options: {
        mobileChat: {
          name: 'Mobile chat',
          styles: { width: '390px', height: '700px' },
          type: 'mobile',
        },
      },
    },
    screenshot: {
      viewport: { width: 390, height: 700 },
    },
  },
  render: () => (
    <RouterProvider
      router={createFloatingChatRouter(makeProps({ kind: 'idle' }))}
    />
  ),
}

export const SelectedPurpose: Story = {
  name: 'shows a selected purpose in the chat composer.',
  render: () => (
    <RouterProvider
      router={createFloatingChatRouter(
        makeProps({ kind: 'idle' }, { selectedPurpose: 'example-purpose' }),
      )}
    />
  ),
}

export const NoStrategySelected: Story = {
  name: 'allows drafting a prompt before a strategy is selected.',
  render: () => (
    <RouterProvider
      router={createFloatingChatRouter(
        makeProps(
          { kind: 'idle' },
          { strategyId: null, submittedPrompt: null },
        ),
      )}
    />
  ),
}

export const CurrentStrategyNotInList: Story = {
  name: 'shows the current-strategy option for an unlisted selection.',
  render: () => (
    <RouterProvider
      router={createFloatingChatRouter(
        makeProps({ kind: 'idle' }, { strategyId: 'example-current-strategy' }),
      )}
    />
  ),
}

export const Polling: Story = {
  name: 'shows a submitted prompt while its task is still running.',
  render: () => (
    <RouterProvider
      router={createFloatingChatRouter(
        makeProps(
          { kind: 'polling', phase: 'running' },
          {
            input: '',
            submittedPrompt: '架空銘柄の足元評価',
          },
        ),
      )}
    />
  ),
}

export const PollingWithRunLink: Story = {
  name: 'shows a running task with a link to its execution details.',
  render: () => (
    <RouterProvider
      router={createFloatingChatRouter(
        makeProps(
          { kind: 'polling', phase: 'running' },
          {
            input: '',
            submittedPrompt: '架空銘柄の足元評価',
            currentTaskId: 'example-task',
            taskStrategyId: 'example-strategy-a',
          },
        ),
      )}
    />
  ),
}

export const Completed: Story = {
  name: 'shows notes created or updated by a completed task.',
  render: () => (
    <RouterProvider
      router={createFloatingChatRouter(
        makeProps(
          { kind: 'completed' },
          {
            taskStrategyId: 'example-strategy-a',
            submittedPrompt: '架空銘柄の直近の変化をまとめて',
            resultText:
              '前回より受注が増えています。\n\n**次の決算で確認します。**',
            notes: [
              {
                id: 'example-note-a',
                title: '架空企業の受注メモ',
                updated_at: '2026-06-26T07:00:00Z',
              },
              {
                id: 'example-note-b',
                title: '需給の変化をまとめたメモ',
                updated_at: '2026-06-25T04:00:00Z',
              },
            ],
          },
        ),
      )}
    />
  ),
}

export const CompletedWithoutNotes: Story = {
  name: 'shows a completed task with no linked notes.',
  render: () => (
    <RouterProvider
      router={createFloatingChatRouter(
        makeProps(
          { kind: 'completed' },
          {
            taskStrategyId: 'example-strategy-a',
            submittedPrompt: '架空銘柄の見立てを教えて',
            resultText: '分析結果の本文です。',
          },
        ),
      )}
    />
  ),
}

export const CompletedWithoutReply: Story = {
  name: 'shows the empty reply fallback after a task completes.',
  render: () => (
    <RouterProvider
      router={createFloatingChatRouter(
        makeProps(
          { kind: 'completed' },
          {
            taskStrategyId: 'example-strategy-a',
            submittedPrompt: '架空銘柄の見立てを教えて',
            resultText: '',
          },
        ),
      )}
    />
  ),
}

export const Failed: Story = {
  name: 'shows an error summary after a task fails.',
  render: () => (
    <RouterProvider
      router={createFloatingChatRouter(
        makeProps({ kind: 'failed', error_summary: 'agent crashed: timeout' }),
      )}
    />
  ),
}

export const Error: Story = {
  name: 'shows an error when the strategy agent is unavailable.',
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
