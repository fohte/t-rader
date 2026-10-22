import type { Meta, StoryObj } from '@storybook/react-vite'

import { CiteBadge } from '#components/graph/cite-badge'

const meta = {
  title: 'Graph/CiteBadge',
  component: CiteBadge,
  args: {
    number: 3,
    cite: '架空の出典テキスト',
  },
  decorators: [
    (Story) => (
      <div className="flex h-60 w-120 items-center justify-center">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof CiteBadge>

export default meta
type Story = StoryObj<typeof meta>

export const Closed: Story = {
  name: 'shows the citation number before its source is opened.',
}

export const Open: Story = {
  name: 'shows the source text while the citation popup is open.',
  args: { defaultOpen: true },
}
