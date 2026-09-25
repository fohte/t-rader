import type { Meta, StoryObj } from '@storybook/react-vite'

import { Input } from '#components/ui/input'

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
  name: 'shows an empty text field with placeholder text.',
  args: {
    placeholder: 'テキストを入力...',
  },
}

export const WithValue: Story = {
  name: 'shows a text field populated with a value.',
  args: {
    defaultValue: '入力済みテキスト',
  },
}

export const Password: Story = {
  name: 'shows a password field with masked input.',
  args: {
    type: 'password',
    placeholder: 'パスワード',
  },
}

export const Disabled: Story = {
  name: 'shows a disabled text field.',
  args: {
    disabled: true,
    placeholder: '無効な入力欄',
  },
}

export const WithFile: Story = {
  name: 'shows a file picker input.',
  args: {
    type: 'file',
  },
}
