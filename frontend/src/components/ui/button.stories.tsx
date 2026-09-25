import type { Meta, StoryObj } from '@storybook/react-vite'
import { Mail } from 'lucide-react'

import { Button } from '#components/ui/button'

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
  name: 'shows a standard button.',
  args: {
    children: 'ボタン',
  },
}

export const Destructive: Story = {
  name: 'shows a destructive button for a delete action.',
  args: {
    variant: 'destructive',
    children: '削除',
  },
}

export const Outline: Story = {
  name: 'shows an outlined button for a secondary action.',
  args: {
    variant: 'outline',
    children: 'アウトライン',
  },
}

export const Secondary: Story = {
  name: 'shows a filled button for a secondary action.',
  args: {
    variant: 'secondary',
    children: 'セカンダリ',
  },
}

export const Ghost: Story = {
  name: 'shows a low-emphasis ghost button.',
  args: {
    variant: 'ghost',
    children: 'ゴースト',
  },
}

export const LinkVariant: Story = {
  name: 'renders a button as a text link.',
  args: {
    variant: 'link',
    children: 'リンク',
  },
}

export const Small: Story = {
  name: 'shows a compact button size.',
  args: {
    size: 'sm',
    children: '小さいボタン',
  },
}

export const Large: Story = {
  name: 'shows an enlarged button size.',
  args: {
    size: 'lg',
    children: '大きいボタン',
  },
}

export const WithIcon: Story = {
  name: 'pairs an icon with the button label.',
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
  name: 'shows an outlined button with only an icon.',
  args: {
    variant: 'outline',
    size: 'icon',
    children: <Mail />,
  },
}

export const Disabled: Story = {
  name: 'shows a button in its disabled state.',
  args: {
    disabled: true,
    children: '無効',
  },
}
