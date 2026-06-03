import type { Meta, StoryObj } from '@storybook/react-vite'

import { FloatingChat } from '@/components/strategy-shell/floating-chat'

const meta = {
  title: 'StrategyShell/FloatingChat',
  component: FloatingChat,
  parameters: { layout: 'fullscreen' },
  decorators: [
    (Story) => (
      <div className="h-screen bg-[color:var(--color-bg-primary)] p-4">
        <p className="font-mono text-sm text-[color:var(--color-text-secondary)]">
          右下のフローティングアイコンをクリックして開きます
        </p>
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof FloatingChat>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {}
