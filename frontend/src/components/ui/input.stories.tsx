import type { Meta, StoryObj } from '@storybook/react-vite'

import { Input } from '@/components/ui/input'

const meta = {
  title: 'UI/Input',
  component: Input,
  argTypes: {
    type: {
      control: 'select',
      options: ['text', 'email', 'password', 'number', 'search'],
    },
    disabled: { control: 'boolean' },
    placeholder: { control: 'text' },
  },
} satisfies Meta<typeof Input>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  args: {
    placeholder: 'テキストを入力...',
  },
}

export const WithValue: Story = {
  args: {
    defaultValue: '入力済みテキスト',
  },
}

export const Password: Story = {
  args: {
    type: 'password',
    placeholder: 'パスワード',
  },
}

export const Disabled: Story = {
  args: {
    disabled: true,
    placeholder: '無効な入力欄',
  },
}

export const WithFile: Story = {
  args: {
    type: 'file',
  },
}
