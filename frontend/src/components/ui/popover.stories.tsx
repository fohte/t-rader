import { Button } from '@fohte/ui/button'
import { List, ListItem } from '@fohte/ui/list'
import type { Meta, StoryObj } from '@storybook/react-vite'

import { Popover, PopoverContent, PopoverTrigger } from '#components/ui/popover'

const meta = {
  title: 'UI/Popover',
  component: Popover,
} satisfies Meta<typeof Popover>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  name: 'opens a centered popover with standard padding.',
  render: () => (
    <div className="grid min-h-screen place-items-center">
      <Popover defaultOpen>
        <PopoverTrigger
          render={<Button variant="outline">ポップオーバー</Button>}
        />
        <PopoverContent>
          <p>説明文を表示します。</p>
        </PopoverContent>
      </Popover>
    </div>
  ),
}

export const CandidateList: Story = {
  name: 'shows candidate rows without additional popover padding.',
  render: () => (
    <Popover defaultOpen>
      <PopoverTrigger render={<Button variant="outline">候補一覧</Button>} />
      <PopoverContent padding="none">
        <List role="listbox">
          <ListItem>架空の候補 A</ListItem>
          <ListItem>架空の候補 B</ListItem>
        </List>
      </PopoverContent>
    </Popover>
  ),
}

export const ExplicitPosition: Story = {
  name: 'places the popover above and aligns its end with the trigger.',
  render: () => (
    <div className="grid min-h-screen place-items-center">
      <Popover defaultOpen>
        <PopoverTrigger render={<Button variant="outline">位置指定</Button>} />
        <PopoverContent
          side="top"
          sideOffset={8}
          align="end"
          alignOffset={4}
          padding="sm"
        >
          <p>位置を明示した内容</p>
        </PopoverContent>
      </Popover>
    </div>
  ),
}
