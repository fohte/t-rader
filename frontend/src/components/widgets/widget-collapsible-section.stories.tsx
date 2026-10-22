import type { Meta, StoryObj } from '@storybook/react-vite'
import { fn } from 'storybook/test'

import { WidgetCollapsibleSection } from '#components/widgets/widget-collapsible-section'

const meta = {
  title: 'Components/Widgets/WidgetCollapsibleSection',
  component: WidgetCollapsibleSection,
  args: {
    label: '架空データ一覧',
    expanded: true,
    onExpandedChange: fn(),
  },
  render: (args) => (
    <WidgetCollapsibleSection {...args}>
      <ul className="list-none">
        <li className="px-3 py-2 text-foreground">架空の項目 A</li>
      </ul>
    </WidgetCollapsibleSection>
  ),
} satisfies Meta<typeof WidgetCollapsibleSection>

export default meta
type Story = StoryObj<typeof meta>

export const Expanded: Story = {
  name: '見出しの下に一覧を表示する。',
}

export const Collapsed: Story = {
  name: '見出しだけを表示する。',
  args: { expanded: false },
}
