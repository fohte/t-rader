import { TabsPanel } from '@fohte/ui/tabs'
import type { Meta, StoryObj } from '@storybook/react-vite'

import {
  type StrategyListTab,
  StrategyListTabs,
} from '#components/strategy-home/strategy-list-tabs'

function renderTabs(activeTab: StrategyListTab) {
  return (
    <StrategyListTabs activeTab={activeTab} onChange={() => undefined}>
      <TabsPanel value="list">架空の戦略一覧</TabsPanel>
      <TabsPanel value="runs">架空の実行履歴</TabsPanel>
    </StrategyListTabs>
  )
}

const meta = {
  title: 'StrategyHome/StrategyListTabs',
  component: StrategyListTabs,
  args: { onChange: () => undefined },
} satisfies Meta<typeof StrategyListTabs>

export default meta
type Story = StoryObj<typeof meta>

export const ListSelected: Story = {
  name: 'the strategy list tab is selected.',
  args: { activeTab: 'list' },
  render: (args) => renderTabs(args.activeTab),
}

export const RunsSelected: Story = {
  name: 'the run history tab is selected.',
  args: { activeTab: 'runs' },
  render: (args) => renderTabs(args.activeTab),
}

export const Mobile: Story = {
  name: 'the tabs fit a narrow mobile container.',
  args: { activeTab: 'runs' },
  render: (args) => (
    <div className="w-full max-w-sm bg-background px-3 pt-5">
      {renderTabs(args.activeTab)}
    </div>
  ),
}
