import type { Meta, StoryObj } from '@storybook/react-vite'

import { NoteEvaluationEvidenceCardView } from '#components/note-detail/note-evaluation-evidence-card-view'

const meta = {
  title: 'NoteDetail/NoteEvaluationEvidenceCardView',
  component: NoteEvaluationEvidenceCardView,
  args: {
    open: true,
    onOpenChange: () => undefined,
    displayValue: '256.75',
    outputName: 'result',
    state: {
      status: 'ready',
      evaluation: {
        indicator_name: 'デモ指標',
        args: { period: 12, base: 100 },
        code: 'return inputs.base + inputs.period',
        evaluated_at: '2030-01-02T03:04:05Z',
      },
    },
  },
  parameters: { layout: 'padded' },
} satisfies Meta<typeof NoteEvaluationEvidenceCardView>

export default meta
type Story = StoryObj<typeof meta>

export const Ready: Story = {
  name: 'shows the saved evaluation time, inputs, and collapsed code.',
}

export const Loading: Story = {
  name: 'shows a loading message while the evaluation record is requested.',
  args: { state: { status: 'loading' } },
}

export const Failed: Story = {
  name: 'shows an error when the evaluation record cannot be retrieved.',
  args: { state: { status: 'error' } },
}

export const Missing: Story = {
  name: 'shows a message when the evaluation record no longer exists.',
  args: { state: { status: 'not-found' } },
}
