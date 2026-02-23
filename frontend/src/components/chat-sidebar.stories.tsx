import type { Meta, StoryObj } from '@storybook/react-vite'
import { fn } from 'storybook/test'

import { ChatSidebar } from '@/components/chat-sidebar'

const meta = {
  title: 'Components/ChatSidebar',
  component: ChatSidebar,
  args: {
    onClose: fn(),
  },
  parameters: {
    layout: 'fullscreen',
  },
  decorators: [
    (Story) => (
      <div className="flex h-screen">
        <div className="flex-1 bg-muted/30 p-4">
          <p className="text-muted-foreground">メインコンテンツ</p>
        </div>
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof ChatSidebar>

export default meta
type Story = StoryObj<typeof meta>

export const Open: Story = {
  args: {
    isOpen: true,
  },
}

export const Closed: Story = {
  args: {
    isOpen: false,
  },
}
