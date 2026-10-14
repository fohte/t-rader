import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import {
  TaskRunNotes,
  type TaskRunNotesState,
} from '#components/strategy-shell/task-run-notes'
import { createStoryRouter } from '#storybook/story-router'

function createTaskRunNotesRouter(
  state: TaskRunNotesState,
  notes: { id: string; title: string }[],
) {
  return createStoryRouter(
    () => (
      <div className="min-h-screen bg-background p-6">
        <TaskRunNotes notes={notes} state={state} />
      </div>
    ),
    { paths: ['/notes/$noteId'] },
  )
}

const meta = {
  title: 'StrategyShell/TaskRunNotes',
  parameters: { layout: 'fullscreen' },
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

export const WithNotes: Story = {
  name: 'shows notes that link to their detail pages.',
  render: () => (
    <RouterProvider
      router={createTaskRunNotesRouter('success', [
        { id: 'fake-note-001', title: 'サンプル企業の調査メモ' },
        { id: 'fake-note-002', title: '架空グループの確認事項' },
      ])}
    />
  ),
}

export const Empty: Story = {
  name: 'shows a message when the task produced no notes.',
  render: () => (
    <RouterProvider router={createTaskRunNotesRouter('success', [])} />
  ),
}

export const Loading: Story = {
  name: 'shows a loading message while notes are fetched.',
  render: () => (
    <RouterProvider router={createTaskRunNotesRouter('loading', [])} />
  ),
}

export const Error: Story = {
  name: 'shows an error message when notes could not be fetched.',
  render: () => (
    <RouterProvider router={createTaskRunNotesRouter('error', [])} />
  ),
}
