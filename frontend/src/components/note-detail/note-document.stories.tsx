import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import { useRef } from 'react'

import { NoteDocument } from '#components/note-detail/note-document'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type NoteLinkItem = components['schemas']['NoteLinkItem']

function NoteDocumentStory({
  source,
  noteLinks,
  onQuoteSelection,
}: {
  source: string
  noteLinks: NoteLinkItem[]
  onQuoteSelection: (text: string) => void
}) {
  const bodyRef = useRef<HTMLDivElement>(null)
  return (
    <NoteDocument
      source={source}
      noteLinks={noteLinks}
      onQuoteSelection={onQuoteSelection}
      bodyRef={bodyRef}
    />
  )
}

const meta = {
  title: 'NoteDetail/NoteDocument',
  component: NoteDocumentStory,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <RouterProvider
        router={createStoryRouter(
          () => (
            <div className="max-w-3xl bg-background p-5 text-foreground">
              <Story />
            </div>
          ),
          { paths: ['/notes/$noteId'] },
        )}
      />
    ),
  ],
  args: {
    source:
      'この判断は [[note:00000000-0000-0000-0000-000000000101]] の版を参照しています。',
    noteLinks: [
      {
        note_id: '00000000-0000-0000-0000-000000000101',
        version_id: '00000000-0000-0000-0000-000000000201',
        version_no: 2,
        title: '架空銘柄の購入判断',
      },
    ],
    onQuoteSelection: () => {},
  },
} satisfies Meta<typeof NoteDocumentStory>

export default meta
type Story = StoryObj<typeof meta>

export const WithPinnedNoteLink: Story = {}
