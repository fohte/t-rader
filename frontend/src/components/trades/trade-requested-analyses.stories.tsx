import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import {
  TradeRequestedAnalyses,
  type TradeRequestedAnalysis,
} from '#components/trades/trade-requested-analyses'
import { createStoryRouter } from '#storybook/story-router'

const completedAnalysis: TradeRequestedAnalysis = {
  strategyId: 'fake-strategy',
  taskId: 'fake-task-completed',
  prompt: '架空銘柄の分析を依頼する',
  phase: 'completed',
  createdAt: '2099-02-03T00:00:00Z',
  notes: [
    {
      id: 'fake-note',
      title: '架空銘柄の分析ノート',
      versionId: 'fake-note-version',
    },
  ],
  notesState: 'success',
}

const analyses: TradeRequestedAnalysis[] = [
  completedAnalysis,
  {
    strategyId: 'fake-strategy',
    taskId: 'fake-task-running',
    prompt: '別の観点から確認する',
    phase: 'running',
    createdAt: '2099-02-02T00:00:00Z',
    notes: [],
    notesState: 'success',
  },
  {
    strategyId: 'fake-strategy',
    taskId: 'fake-task-pending',
    prompt: '追加の情報を整理する',
    phase: 'pending',
    createdAt: '2099-02-01T00:00:00Z',
    notes: [],
    notesState: 'loading',
  },
  {
    strategyId: 'fake-strategy',
    taskId: 'fake-task-failed',
    prompt: '異なる条件で分析する',
    phase: 'failed',
    createdAt: '2099-01-31T00:00:00Z',
    notes: [],
    notesState: 'error',
  },
]

function withRouter(Story: ComponentType) {
  return (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <Story />
        ),
        {
          paths: ['/notes/$noteId', '/strategies/$id/runs/$taskId'],
        },
      )}
    />
  )
}

const meta = {
  title: 'Trades/TradeRequestedAnalyses',
  component: TradeRequestedAnalyses,
  decorators: [withRouter],
  args: { analyses, state: 'success' },
} satisfies Meta<typeof TradeRequestedAnalyses>

export default meta
type Story = StoryObj<typeof meta>

export const RequestedAnalyses: Story = {
  name: 'shows task states, written notes, and run detail links.',
}

export const Empty: Story = {
  name: 'shows an empty message when the trade has no requested analyses.',
  args: { analyses: [] },
}

export const Loading: Story = {
  name: 'shows a loading message while requested analyses are loading.',
  args: { analyses: [], state: 'loading' },
}

export const Error: Story = {
  name: 'shows an error when requested analyses cannot be loaded.',
  args: { analyses: [], state: 'error' },
}

export const NoteLoading: Story = {
  name: 'shows a loading message while a task note is loading.',
  args: {
    analyses: [
      {
        ...completedAnalysis,
        notes: [],
        notesState: 'loading',
      },
    ],
  },
}

export const NoteError: Story = {
  name: 'shows an error when a task note cannot be loaded.',
  args: {
    analyses: [
      {
        ...completedAnalysis,
        notes: [],
        notesState: 'error',
      },
    ],
  },
}
