import type { Meta, StoryObj } from '@storybook/react-vite'
import { Mail } from 'lucide-react'

import { Button } from '@/components/ui/button'

const meta = {
  title: 'UI/Button',
  component: Button,
  argTypes: {
    variant: {
      control: 'select',
      options: [
        'default',
        'destructive',
        'outline',
        'secondary',
        'ghost',
        'link',
      ],
    },
    size: {
      control: 'select',
      options: ['default', 'xs', 'sm', 'lg', 'icon', 'icon-xs', 'icon-sm'],
    },
    disabled: { control: 'boolean' },
  },
} satisfies Meta<typeof Button>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  args: {
    children: 'ボタン',
  },
}

export const Destructive: Story = {
  args: {
    variant: 'destructive',
    children: '削除',
  },
}

export const Outline: Story = {
  args: {
    variant: 'outline',
    children: 'アウトライン',
  },
}

export const Secondary: Story = {
  args: {
    variant: 'secondary',
    children: 'セカンダリ',
  },
}

export const Ghost: Story = {
  args: {
    variant: 'ghost',
    children: 'ゴースト',
  },
}

export const LinkVariant: Story = {
  args: {
    variant: 'link',
    children: 'リンク',
  },
}

export const Small: Story = {
  args: {
    size: 'sm',
    children: '小さいボタン',
  },
}

export const Large: Story = {
  args: {
    size: 'lg',
    children: '大きいボタン',
  },
}

export const WithIcon: Story = {
  args: {
    children: (
      <>
        <Mail />
        メール送信
      </>
    ),
  },
}

export const IconOnly: Story = {
  args: {
    variant: 'outline',
    size: 'icon',
    children: <Mail />,
  },
}

export const Disabled: Story = {
  args: {
    disabled: true,
    children: '無効',
  },
}
