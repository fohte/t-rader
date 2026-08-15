import type { Meta, StoryObj } from '@storybook/react-vite'

import { Button } from '#components/ui/button'
import { Popover, PopoverContent, PopoverTrigger } from '#components/ui/popover'

const meta = {
  title: 'UI/Popover',
  component: Popover,
} satisfies Meta<typeof Popover>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  render: () => (
    <Popover>
      <PopoverTrigger asChild>
        <Button variant="outline">クリックしてください</Button>
      </PopoverTrigger>
      <PopoverContent>
        <p>ポップオーバーの内容</p>
      </PopoverContent>
    </Popover>
  ),
}
