import type { Meta, StoryObj } from '@storybook/react-vite'

import {
  EMPTY_FORM,
  TriggerForm,
} from '#components/strategy-settings/trigger-form'

const meta = {
  title: 'StrategySettings/TriggerForm',
  component: TriggerForm,
} satisfies Meta<typeof TriggerForm>

export default meta
type Story = StoryObj<typeof meta>

export const CronCreation: Story = {
  name: 'the form creates a cron trigger.',
  args: {
    mode: 'create',
    form: {
      ...EMPTY_FORM,
      schedule: '0 9 * * 1-5',
      promptTemplate: 'synthetic prompt',
    },
    agentConfigs: [{ purpose: 'synthetic-purpose' }],
    onChange: () => {},
    formError: null,
    isSaving: false,
    onSubmit: () => {},
    onCancel: null,
  },
}

export const PurposeSelected: Story = {
  name: 'the form uses a selected agent configuration.',
  args: {
    mode: 'create',
    form: {
      ...EMPTY_FORM,
      purpose: 'synthetic-purpose',
      schedule: '0 9 * * 1-5',
      promptTemplate: 'synthetic prompt',
    },
    agentConfigs: [{ purpose: 'synthetic-purpose' }],
    onChange: () => {},
    formError: null,
    isSaving: false,
    onSubmit: () => {},
    onCancel: null,
  },
}

export const HookEditing: Story = {
  name: 'the form edits a hook trigger with a locked kind.',
  args: {
    mode: 'edit',
    form: {
      ...EMPTY_FORM,
      kind: 'hook',
      hookSlug: 'synthetic-hook',
      eventMatch: `{
  "event": "synthetic-value"
}`,
      promptTemplate: 'synthetic prompt',
    },
    agentConfigs: [{ purpose: 'synthetic-purpose' }],
    onChange: () => {},
    formError: null,
    isSaving: false,
    onSubmit: () => {},
    onCancel: null,
  },
}
