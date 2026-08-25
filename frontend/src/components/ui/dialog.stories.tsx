import type { Meta, StoryObj } from '@storybook/react-vite'

import { Button } from '#components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '#components/ui/dialog'
import { Input } from '#components/ui/input'

const meta = {
  title: 'UI/Dialog',
  component: Dialog,
} satisfies Meta<typeof Dialog>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  render: () => (
    <Dialog>
      <DialogTrigger render={<Button variant="outline">開く</Button>} />
      <DialogContent>
        <DialogHeader>
          <DialogTitle>タイトル</DialogTitle>
          <DialogDescription>説明文</DialogDescription>
        </DialogHeader>
        <DialogFooter showCloseButton />
      </DialogContent>
    </Dialog>
  ),
}

export const AutoFocusInput: Story = {
  render: () => (
    <Dialog defaultOpen>
      <DialogTrigger render={<Button variant="outline">開く</Button>} />
      <DialogContent>
        <DialogHeader>
          <DialogTitle>入力欄への初期フォーカス</DialogTitle>
          <DialogDescription>
            開いた直後、input の初期値が全選択される
          </DialogDescription>
        </DialogHeader>
        <Input autoFocus defaultValue="1000000" />
        <DialogFooter showCloseButton />
      </DialogContent>
    </Dialog>
  ),
}

export const InitialFocusDisabled: Story = {
  render: () => (
    <Dialog defaultOpen>
      <DialogTrigger render={<Button variant="outline">開く</Button>} />
      <DialogContent initialFocus={false}>
        <DialogHeader>
          <DialogTitle>初期フォーカスなし</DialogTitle>
          <DialogDescription>
            input にフォーカスも文字選択も発生しない
          </DialogDescription>
        </DialogHeader>
        <Input defaultValue="1000000" />
        <DialogFooter showCloseButton />
      </DialogContent>
    </Dialog>
  ),
}
