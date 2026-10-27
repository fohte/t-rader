import type { Meta, StoryObj } from '@storybook/react-vite'
import type { ComponentProps } from 'react'

import { WarningDispositionReview } from '#components/note-detail/warning-disposition-review'

const warningDispositions: ComponentProps<
  typeof WarningDispositionReview
>['warningDispositions'] = [
  {
    warning_id: 'price:42%25:1',
    value: '42%',
    location: { kind: 'body', start: 13, end: 16 },
    reason: '架空の計算例として記載したため',
  },
  {
    warning_id: 'relative:-3.5%25:1',
    value: '-3.5%',
    location: {
      kind: 'graph',
      path: 'graphs[0].nodes[0].label',
      start: 0,
      end: 5,
    },
    reason: '図の説明用に置いた架空の値のため',
  },
]

const meta = {
  title: 'NoteDetail/WarningDispositionReview',
  component: WarningDispositionReview,
  args: {
    versionNo: 4,
    warningDispositions,
    confirmation: { checked: false, onChange: () => {} },
  },
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="max-w-lg bg-background p-3 text-foreground">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof WarningDispositionReview>

export default meta
type Story = StoryObj<typeof meta>

export const ConfirmationRequired: Story = {
  name: 'shows each ignored warning with its value, location, reason, and confirmation.',
}

export const Confirmed: Story = {
  name: 'shows that every ignored warning has been confirmed for approval.',
  args: { confirmation: { checked: true, onChange: () => {} } },
}
