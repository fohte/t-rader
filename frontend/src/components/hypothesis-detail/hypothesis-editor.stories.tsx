import type { Meta, StoryObj } from '@storybook/react-vite'

import { HypothesisEditorView } from '#components/hypothesis-detail/hypothesis-editor'

const meta = {
  title: 'HypothesisDetail/HypothesisEditor',
  component: HypothesisEditorView,
  decorators: [
    (Story) => (
      <div className="max-w-4xl bg-background p-5 font-sans text-foreground">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof HypothesisEditorView>

export default meta
type Story = StoryObj<typeof meta>

const args = {
  title: '売上の傾向が今後も続く',
  body: '次回の確認時に、直近の傾向が続いているかを検証する。',
  onTitleChange: () => {},
  onBodyChange: () => {},
  onSave: () => {},
  isDirty: false,
  isSaving: false,
  validationError: null,
  saveError: null,
}

export const Default: Story = { args }

export const Dirty: Story = {
  args: { ...args, isDirty: true },
}

export const Saving: Story = {
  args: { ...args, isDirty: true, isSaving: true },
}

export const ValidationError: Story = {
  args: { ...args, isDirty: true, validationError: 'title と body は必須です' },
}

export const SaveError: Story = {
  args: { ...args, isDirty: true, saveError: '保存に失敗しました' },
}
