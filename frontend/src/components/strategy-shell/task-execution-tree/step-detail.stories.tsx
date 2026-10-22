import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import {
  StepDetail,
  type TaskStep,
} from '#components/strategy-shell/task-execution-tree'
import { COMPLETED_STEP } from '#components/strategy-shell/task-execution-tree.fixtures'
import { createStoryRouter } from '#storybook/story-router'

const FAILED_STEP = {
  ...COMPLETED_STEP,
  execution_step_id: 'sample-step-002',
  status: 'failed',
  error: 'Sample processing failed',
  output: undefined,
} satisfies TaskStep

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

export const Operation: Story = {
  name: 'the details show a saved-code operation and its payload references.',
  args: {
    step: {
      ...COMPLETED_STEP,
      phase_key: 'calculate',
      execution_step_id: 'sample-step-operation',
      label: '計算',
      kind: 'operation',
      model: null,
      input_ref: {
        payload_inputs: [
          {
            field: 'source',
            payload_id: '00000000-0000-4000-8000-000000000001',
            pointer: '/records',
            cursor: 0,
            limit: 25,
          },
        ],
      },
      output_ref: { id: '00000000-0000-4000-8000-000000000002' },
      output: {
        payloadRef: { id: '00000000-0000-4000-8000-000000000002' },
        pointer: '/value',
      },
    },
    phaseDefinition: {
      key: 'calculate',
      label: '計算',
      kind: 'operation',
      operation: 'evaluate_saved_code',
      code_id: 'sample-code',
      input: {
        source: {
          prior_output: {
            phase: 'prepare',
            pointer: '/records',
            cursor: 0,
            limit: 25,
          },
        },
      },
    },
    onSelectPayload: () => {},
  },
}
