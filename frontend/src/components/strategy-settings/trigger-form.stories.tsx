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

export const EventCreation: Story = {
  name: 'the form creates an event trigger.',
  args: {
    mode: 'create',
    form: {
      ...EMPTY_FORM,
      kind: 'event',
      eventType: 'sample_process.finished',
      eventMatch: `{
  "status": {
    "eq": "complete"
  }
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

export const EventEditing: Story = {
  name: 'the form edits an event trigger with a locked kind.',
  args: {
    mode: 'edit',
    form: {
      ...EMPTY_FORM,
      kind: 'event',
      eventType: 'sample_process.finished',
      eventMatch: `{
  "status": {
    "exists": true
  }
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

export const Saving: Story = {
  name: 'the form disables purpose selection while saving.',
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
    isSaving: true,
    onSubmit: () => {},
    onCancel: () => {},
  },
}

export const ValidationError: Story = {
  name: 'the form shows a validation error.',
  args: {
    mode: 'create',
    form: {
      ...EMPTY_FORM,
      promptTemplate: 'synthetic prompt',
    },
    agentConfigs: [],
    onChange: () => {},
    formError: 'schedule (cron 式) は必須です',
    isSaving: false,
    onSubmit: () => {},
    onCancel: () => {},
  },
}

export const KindMenuOpen: Story = {
  name: 'the kind menu shows all trigger types.',
  args: {
    mode: 'create',
    form: {
      ...EMPTY_FORM,
      schedule: '15 10 * * 2',
      promptTemplate: 'synthetic prompt',
    },
    defaultKindSelectOpen: true,
    agentConfigs: [],
    onChange: () => {},
    formError: null,
    isSaving: false,
    onSubmit: () => {},
    onCancel: null,
  },
}
