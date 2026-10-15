import type {
  CommandPaletteGroup,
  CommandPaletteItem,
} from '@fohte/ui/command-palette'
import type { Meta, StoryObj } from '@storybook/react-vite'

import {
  QuickSwitcherItem,
  QuickSwitcherView,
} from '#components/strategy-shell/quick-switcher-view'

function item(key: string, label: string, detail?: string): CommandPaletteItem {
  return {
    key,
    select: () => undefined,
    render: ({ isSelected, onMouseMove }) => (
      <QuickSwitcherItem
        label={label}
        detail={detail}
        isSelected={isSelected}
        onMouseMove={onMouseMove}
        onSelect={() => undefined}
      />
    ),
  }
}

const initialGroups: CommandPaletteGroup[] = [
  {
    id: 'screens',
    title: '画面',
    items: [
      item('dashboard', 'ダッシュボード'),
      item('portfolio', '資産 · 保有'),
      item('notes', 'ノート'),
    ],
  },
  {
    id: 'strategies',
    title: '戦略',
    items: [item('strategy:demo-strategy', 'デモ戦略', 'demo-strategy')],
  },
]

const meta = {
  title: 'StrategyShell/QuickSwitcher',
  component: QuickSwitcherView,
  args: {
    open: true,
    onOpenChange: () => undefined,
    shortcutLabel: '⌘K',
    value: '',
    onValueChange: () => undefined,
    groups: initialGroups,
    loading: false,
  },
} satisfies Meta<typeof QuickSwitcherView>

export default meta
type Story = StoryObj<typeof meta>

export const InitialResults: Story = {
  name: 'shows screens and strategies before a query is entered.',
}

export const SearchResults: Story = {
  name: 'shows matching stock and indicator results for a query.',
  args: {
    value: 'デモ',
    groups: [
      {
        id: 'stocks',
        title: '銘柄',
        items: [item('stock:demo-code', 'デモ商事', 'demo-code')],
      },
      {
        id: 'indicators',
        title: '指標',
        items: [item('indicator:demo-indicator', 'デモ指標', 'demo-indicator')],
      },
    ],
  },
}

export const Loading: Story = {
  name: 'shows a loading indicator while results are being fetched.',
  args: { groups: [], loading: true },
}

export const NoResults: Story = {
  name: 'shows a message when a query has no matching results.',
  args: {
    value: '未登録',
    groups: [],
    emptyMessage: '該当する項目はありません。',
  },
}

export const PartialResultsWithSearchError: Story = {
  name: 'shows matching screens when reference searches fail.',
  args: {
    value: '設定',
    groups: [
      {
        id: 'screens',
        title: '画面',
        items: [item('settings', '全体の設定')],
      },
    ],
    emptyMessage: '一部の検索に失敗しました。',
  },
}

export const SearchButtonOnMobile: Story = {
  name: 'shows only the search icon in the mobile header.',
  args: { open: false },
  globals: { viewport: { value: 'quickSwitcherMobile' } },
  parameters: {
    viewport: {
      options: {
        quickSwitcherMobile: {
          name: 'Quick switcher mobile',
          styles: { width: '390px', height: '700px' },
          type: 'mobile',
        },
      },
    },
    screenshot: {
      viewport: { width: 390, height: 700 },
    },
  },
}
