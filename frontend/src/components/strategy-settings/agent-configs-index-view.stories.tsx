import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentProps } from 'react'

import { AgentConfigsIndexView } from '#components/strategy-settings/agent-configs-index-view'
import { createStoryRouter } from '#storybook/story-router'

const meta = {
  title: 'StrategySettings/AgentConfigs/IndexView',
  component: AgentConfigsIndexView,
} satisfies Meta<typeof AgentConfigsIndexView>

export default meta
type Story = StoryObj<typeof meta>

function renderView(args: ComponentProps<typeof AgentConfigsIndexView>) {
  const router = createStoryRouter(() => <AgentConfigsIndexView {...args} />, {
    paths: ['/settings/agent-configs', '/settings/agent-configs/$purpose'],
    initialPath: '/settings/agent-configs',
  })
  return <RouterProvider router={router} />
}

const callbacks = {
  onPurposeChange: () => {},
  onCreate: () => {},
  onDelete: () => {},
}

export const Loading: Story = {
  name: 'shows a loading placeholder while settings are fetched.',
  args: {
    purposes: [],
    isPending: true,
    newPurpose: '',
    isCreating: false,
    createError: null,
    ...callbacks,
  },
  render: renderView,
}

export const Empty: Story = {
  name: 'shows the empty state before the first setting is added.',
  args: {
    purposes: [],
    isPending: false,
    newPurpose: '',
    isCreating: false,
    createError: null,
    ...callbacks,
  },
  render: renderView,
}

export const Populated: Story = {
  name: 'shows existing settings with separate edit and delete actions.',
  args: {
    purposes: ['sample-purpose', 'demo-purpose'],
    isPending: false,
    newPurpose: 'next-purpose',
    isCreating: false,
    createError: null,
    ...callbacks,
  },
  render: renderView,
}

export const CreateError: Story = {
  name: 'shows a validation message when a setting cannot be added.',
  args: {
    purposes: [],
    isPending: false,
    newPurpose: 'sample-purpose',
    isCreating: false,
    createError: '設定を追加できませんでした',
    ...callbacks,
  },
  render: renderView,
}
