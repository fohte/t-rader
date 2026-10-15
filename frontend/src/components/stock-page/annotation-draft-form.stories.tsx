import type { Meta, StoryObj } from '@storybook/react-vite'

import { AnnotationDraftForm } from '#components/stock-page/annotation-draft-form'

const meta = {
  title: 'StockPage/AnnotationDraftForm',
  component: AnnotationDraftForm,
  args: {
    ariaLabel: 'アノテーションの入力',
    header: (
      <h2 className="font-mono text-xs font-bold text-foreground">
        新しいメモ
      </h2>
    ),
    targetKinds: ['sample-kind', 'sample-kind-2'],
    onSave: () => undefined,
    onCancel: () => undefined,
  },
} satisfies Meta<typeof AnnotationDraftForm>

export default meta
type Story = StoryObj<typeof meta>

export const Empty: Story = {
  name: 'shows empty kind and note fields.',
}

export const Saving: Story = {
  name: 'shows the form while an annotation is being saved.',
  args: { isSaving: true },
}

export const SaveFailed: Story = {
  name: 'shows an error when the annotation could not be saved.',
  args: { errorMessage: '保存に失敗しました' },
}
