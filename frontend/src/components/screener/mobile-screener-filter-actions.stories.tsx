import type { Meta, StoryObj } from '@storybook/react-vite'

import { MobileScreenerFilterActions } from '#components/screener/mobile-screener-filter-actions'

const meta = {
  title: 'Screener/MobileScreenerFilterActions',
  component: MobileScreenerFilterActions,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="bg-background text-foreground">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof MobileScreenerFilterActions>

export default meta
type Story = StoryObj<typeof meta>

export const WithAgentRequest: Story = {
  name: '絞り込みとエージェント依頼を選べます。',
  args: { onAskAgent: () => {} },
}

export const FilterOnly: Story = {
  name: 'エージェント依頼がないときは絞り込みだけを表示します。',
  args: {},
}

export const WithValidationError: Story = {
  name: '条件の入力にエラーがある状態を表示します。',
  args: {
    validationError: '下限は上限以下にしてください。',
    onAskAgent: () => {},
  },
}
