import type { Meta, StoryObj } from '@storybook/react-vite'

import { NoteVersionFallback } from '#components/note-detail/note-version-fallback'

const meta = {
  title: 'NoteDetail/NoteVersionFallback',
  component: NoteVersionFallback,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="max-w-3xl bg-background text-foreground">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof NoteVersionFallback>

export default meta
type Story = StoryObj<typeof meta>

export const Loading: Story = { args: { state: 'loading' } }

export const Missing: Story = { args: { state: 'missing' } }

export const Error: Story = { args: { state: 'error' } }
