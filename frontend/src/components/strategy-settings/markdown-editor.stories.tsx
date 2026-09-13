import type { Meta, StoryObj } from '@storybook/react-vite'

import { MarkdownEditor } from '#components/strategy-settings/markdown-editor'
import type { RefResolveStub } from '#lib/refs.test-helper'
import { RefResolveQueryDecorator } from '#storybook/ref-resolve-mock'

const STUBS: RefResolveStub[] = [
  { kind: 'stock', id: '7203', name: 'トヨタ自動車' },
  { kind: 'indicator', id: 'USDJPY', name: 'USD/JPY' },
]

const meta = {
  title: 'StrategySettings/MarkdownEditor',
  component: MarkdownEditor,
  decorators: [
    (Story) => (
      <RefResolveQueryDecorator stubs={STUBS}>
        <Story />
      </RefResolveQueryDecorator>
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
