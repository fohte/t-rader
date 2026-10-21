import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import {
  StepDetail,
  type TaskStep,
} from '#components/strategy-shell/task-execution-tree'
import { createStoryRouter } from '#storybook/story-router'

const COMPLETED_STEP: TaskStep = {
  phase_key: 'sample-phase',
  execution_step_id: 'sample-step-001',
  label: 'サンプル処理',
  model: 'simulated-model-cobalt',
  status: 'completed',
  item: { record: 'sample-row' },
  item_label: 'サンプル項目',
  output: { state: 'sample-result', note_id: 'sample-note-001' },
  started_at: '2026-08-15T09:00:00.000Z',
  finished_at: '2026-08-15T09:00:08.000Z',
  trace_id: 'sample-trace-001',
  span_id: 'sample-span-001',
}

const FAILED_STEP: TaskStep = {
  ...COMPLETED_STEP,
  execution_step_id: 'sample-step-002',
  status: 'failed',
  error: 'Sample processing failed',
  output: undefined,
}

function Frame({ children }: { children: React.ReactNode }) {
  const router = createStoryRouter(
    () => <div className="max-w-lg bg-background p-6">{children}</div>,
    { paths: ['/notes/$noteId'] },
  )
  return <RouterProvider router={router} />
}

const meta = {
  title: 'StrategyShell/TaskExecutionTree/StepDetail',
  component: StepDetail,
  parameters: { layout: 'fullscreen' },
  render: (args) => (
    <Frame>
      <StepDetail {...args} />
    </Frame>
  ),
} satisfies Meta<typeof StepDetail>

export default meta
type Story = StoryObj<typeof meta>

export const Completed: Story = {
  name: 'the details show input, output, and related links.',
  args: {
    step: COMPLETED_STEP,
    outputSchema: { state: { enum: ['sample-result'] } },
    traceUrlTemplate: 'https://example.test/trace/{trace_id}?span={span_id}',
  },
}

export const Failed: Story = {
  name: 'the details show an error for a failed step.',
  args: {
    step: FAILED_STEP,
  },
}
