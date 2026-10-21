import { Checkbox } from '@fohte/ui/checkbox'
import type { Meta, StoryObj } from '@storybook/react-vite'

import { SelectableListRow } from '#components/strategy-settings/selectable-list-row'

const meta = {
  title: 'StrategySettings/SelectableListRow',
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

export const Available: Story = {
  name: 'the row is available for selection.',
  render: () => (
    <ul className="w-96 border border-border">
      <SelectableListRow
        selected={false}
        onSelect={() => {}}
        deleteLabel="sample item を削除"
        onDelete={() => {}}
      >
        <span className="min-w-0 flex-1 truncate">sample-item</span>
      </SelectableListRow>
    </ul>
  ),
}

export const Selected: Story = {
  name: 'the row marks the selected item.',
  render: () => (
    <ul className="w-96 border border-border">
      <SelectableListRow
        selected
        onSelect={() => {}}
        deleteLabel="sample item を削除"
        onDelete={() => {}}
      >
        <span className="min-w-0 flex-1 truncate">sample-item</span>
      </SelectableListRow>
    </ul>
  ),
}

export const WithToggle: Story = {
  name: 'the row includes an independent toggle.',
  render: () => (
    <ul className="w-96 border border-border">
      <SelectableListRow
        selected
        onSelect={() => {}}
        deleteLabel="sample item を削除"
        onDelete={() => {}}
        trailingControl={
          <Checkbox
            className="mx-1"
            aria-label="sample item を有効化"
            checked
            onCheckedChange={() => {}}
          />
        }
      >
        <span className="min-w-0 flex-1 truncate">sample-item</span>
      </SelectableListRow>
    </ul>
  ),
}
