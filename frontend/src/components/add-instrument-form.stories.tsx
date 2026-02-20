import type { Meta, StoryObj } from '@storybook/react-vite'
import type { FormEvent } from 'react'

import { AddInstrumentFormView } from '@/components/add-instrument-form'

const meta = {
  title: 'Components/AddInstrumentForm',
  component: AddInstrumentFormView,
} satisfies Meta<typeof AddInstrumentFormView>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  args: {
    instrumentId: '',
    name: '',
    onInstrumentIdChange: () => {},
    onNameChange: () => {},
    onSubmit: (e: FormEvent) => e.preventDefault(),
    isSubmitting: false,
    error: null,
  },
}

export const Filled: Story = {
  args: {
    instrumentId: '7203',
    name: 'トヨタ自動車',
    onInstrumentIdChange: () => {},
    onNameChange: () => {},
    onSubmit: (e: FormEvent) => e.preventDefault(),
    isSubmitting: false,
    error: null,
  },
}

export const Submitting: Story = {
  args: {
    instrumentId: '7203',
    name: 'トヨタ自動車',
    onInstrumentIdChange: () => {},
    onNameChange: () => {},
    onSubmit: (e: FormEvent) => e.preventDefault(),
    isSubmitting: true,
    error: null,
  },
}

export const WithError: Story = {
  args: {
    instrumentId: '7203',
    name: 'トヨタ自動車',
    onInstrumentIdChange: () => {},
    onNameChange: () => {},
    onSubmit: (e: FormEvent) => e.preventDefault(),
    isSubmitting: false,
    error: 'この銘柄は既にウォッチリストに追加されています',
  },
}
