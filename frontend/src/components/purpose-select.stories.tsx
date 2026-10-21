import type { Meta, StoryObj } from '@storybook/react-vite'

import { PurposeSelect } from '#components/purpose-select'

const NOOP = (): void => {}

const meta = {
  title: 'Components/PurposeSelect',
  component: PurposeSelect,
} satisfies Meta<typeof PurposeSelect>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  name: 'shows the default configuration when no purpose is selected.',
  args: {
    purposes: ['purpose-alpha', 'purpose-beta'],
    selectedPurpose: '',
    onPurposeChange: NOOP,
    disabled: false,
  },
}

export const Selected: Story = {
  name: 'shows a selected configuration among the available choices.',
  args: {
    purposes: ['purpose-alpha', 'purpose-beta'],
    selectedPurpose: 'purpose-alpha',
    onPurposeChange: NOOP,
    disabled: false,
  },
}

export const Disabled: Story = {
  name: 'disables changing the selected configuration.',
  args: {
    purposes: ['purpose-alpha', 'purpose-beta'],
    selectedPurpose: 'purpose-alpha',
    onPurposeChange: NOOP,
    disabled: true,
  },
}
