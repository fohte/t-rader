import type { Meta, StoryObj } from '@storybook/react-vite'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'

import { MarkdownEditor } from '#components/strategy-settings/markdown-editor'
import { mockResolveRef } from '#storybook/mock-resolve-ref'

const queryClient = new QueryClient()

const NAMES: Record<string, string> = {
  'stock:7203': 'トヨタ自動車',
  'indicator:USDJPY': 'USD/JPY',
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

const SAMPLE = `# AGENTS.md

この戦略の方針と制約を Markdown で記述する。

- 投資ホライズン: 1-3 ヶ月
- 集中: 半導体・電子部品
- 想定リスク: USDJPY ボラティリティ

> [[indicator:USDJPY]] と [[stock:7203]] は要監視。
`

export const Default: Story = {
  name: 'edits strategy guidance with markdown and linked references.',
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
