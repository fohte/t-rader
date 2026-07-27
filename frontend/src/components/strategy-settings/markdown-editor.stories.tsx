import type { Meta, StoryObj } from '@storybook/react-vite'

import { MarkdownEditor } from '#components/strategy-settings/markdown-editor'

const meta = {
  title: 'StrategySettings/MarkdownEditor',
  component: MarkdownEditor,
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
  args: {
    initialValue: SAMPLE,
    onSave: () => {},
  },
}

export const Saving: Story = {
  args: {
    initialValue: SAMPLE,
    onSave: () => {},
    isSaving: true,
  },
}

export const WithError: Story = {
  args: {
    initialValue: SAMPLE,
    onSave: () => {},
    saveError: '保存に失敗しました',
  },
}

export const Empty: Story = {
  args: {
    initialValue: '',
    onSave: () => {},
  },
}
