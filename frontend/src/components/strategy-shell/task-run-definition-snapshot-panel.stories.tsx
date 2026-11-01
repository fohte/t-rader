import type { Meta, StoryObj } from '@storybook/react-vite'

import { TaskRunDefinitionSnapshotPanel } from '#components/strategy-shell/task-run-definition-snapshot-panel'

const meta = {
  title: 'StrategyShell/TaskRunDefinitionSnapshotPanel',
  component: TaskRunDefinitionSnapshotPanel,
} satisfies Meta<typeof TaskRunDefinitionSnapshotPanel>

export default meta
type Story = StoryObj<typeof meta>

const SNAPSHOTS = [
  {
    definitionKey: 'task_start',
    taskId: 'task-0001',
    digest: 'synthetic-task-digest',
    createdAt: '2026-08-16T09:00:00.000Z',
    definition: { agent_config: { agent_graph: 'phases: []' } },
  },
  {
    definitionKey: 'custom_indicator:sample-indicator',
    taskId: 'task-0001',
    digest: 'synthetic-indicator-digest',
    createdAt: '2026-08-16T08:59:59.000Z',
    definition: {
      name: 'sample-indicator',
      code: 'synthetic code body',
      metadata: { revision: 'synthetic-r1' },
    },
  },
]

export const Default: Story = {
  name: 'shows the task-start snapshot and its metadata.',
  args: {
    snapshots: SNAPSHOTS,
    state: 'ready',
  },
}

export const CustomDefinition: Story = {
  name: 'shows a selected custom definition with its body expanded.',
  args: {
    snapshots: SNAPSHOTS,
    state: 'ready',
    initialSelectedKey: 'custom_indicator:sample-indicator',
    definitionBodyOpen: true,
  },
}

export const Loading: Story = {
  name: 'shows the loading state while snapshots are fetched.',
  args: {
    snapshots: [],
    state: 'loading',
  },
}

export const Error: Story = {
  name: 'shows an error when snapshots cannot be fetched.',
  args: {
    snapshots: [],
    state: 'error',
  },
}

export const Missing: Story = {
  name: 'shows that no fixed definition was recorded.',
  args: {
    snapshots: [],
    state: 'ready',
  },
}
