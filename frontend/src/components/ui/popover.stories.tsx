import { Button } from '@fohte/ui/button'
import type { Meta, StoryObj } from '@storybook/react-vite'

import { Popover, PopoverContent, PopoverTrigger } from '#components/ui/popover'

const meta = {
  title: 'UI/Popover',
  component: Popover,
} satisfies Meta<typeof Popover>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  name: 'shows the button that opens a popover.',
  render: () => (
    <Popover>
      <PopoverTrigger
        render={<Button variant="outline">クリックしてください</Button>}
      />
      <PopoverContent>
        <p>ポップオーバーの内容</p>
      </PopoverContent>
    </Popover>
  ),
}
