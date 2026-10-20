import type { Meta, StoryObj } from '@storybook/react-vite'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'

import { MarkdownEditor } from '#components/strategy-settings/markdown-editor'
import { mockResolveRef } from '#storybook/mock-resolve-ref'

const queryClient = new QueryClient()

const NAMES: Record<string, string> = {
  'stock:sample-card-73': '参照カード',
  'indicator:sample-field-46': '補助値',
}

const meta = {
  title: 'StrategySettings/MarkdownEditor',
  component: MarkdownEditor,
  parameters: {
    msw: { handlers: [mockResolveRef(NAMES)] },
  },
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <Story />
      </QueryClientProvider>
    ),
  ],
} satisfies Meta<typeof MarkdownEditor>

export default meta
type Story = StoryObj<typeof meta>

const SAMPLE = `# 作業メモ

表示内容を Markdown で記録する。

- 更新項目: 一覧の見出し
- 確認状態: 読みやすい
- 参照先: [[indicator:sample-field-46]]

> [[stock:sample-card-73]] は表示確認用のサンプルです。
`

export const Default: Story = {
  name: 'edits a work note with markdown and linked references.',
  args: {
    initialValue: SAMPLE,
    onSave: () => {},
  },
}

export const Saving: Story = {
  name: 'shows the editor while changes are being saved.',
  args: {
    initialValue: SAMPLE,
    onSave: () => {},
    isSaving: true,
  },
}

export const WithError: Story = {
  name: 'shows a save error beneath the editor.',
  args: {
    initialValue: SAMPLE,
    onSave: () => {},
    saveError: '保存に失敗しました',
  },
}

export const Empty: Story = {
  name: 'shows the editor before any guidance is entered.',
  args: {
    initialValue: '',
    onSave: () => {},
  },
}
