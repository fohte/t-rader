import type { Meta, StoryObj } from '@storybook/react-vite'
import { History, NotebookPen } from 'lucide-react'

import { PlaceholderPage } from '@/components/placeholder-page'

const meta = {
  title: 'Components/PlaceholderPage',
  component: PlaceholderPage,
} satisfies Meta<typeof PlaceholderPage>

export default meta
type Story = StoryObj<typeof meta>

export const TradeHistory: Story = {
  args: {
    title: 'トレード履歴',
    description: '売買記録の一覧と振り返りができます',
    icon: History,
  },
}

export const Notes: Story = {
  args: {
    title: 'ノート',
    description: '日次・週次の振り返りや分析メモを記録できます',
    icon: NotebookPen,
  },
}
