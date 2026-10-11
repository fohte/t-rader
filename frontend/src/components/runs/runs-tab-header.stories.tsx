import type { Meta, StoryObj } from '@storybook/react-vite'

import { RunsTabHeader } from '#components/runs/runs-tab-header'

const meta = {
  title: 'Runs/RunsTabHeader',
  component: RunsTabHeader,
} satisfies Meta<typeof RunsTabHeader>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  name: 'the run history tab shows its title and description.',
}
