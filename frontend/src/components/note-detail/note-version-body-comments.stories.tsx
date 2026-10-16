import type { Meta, StoryObj } from '@storybook/react-vite'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { http, HttpResponse } from 'msw'

import { NoteBodyCard } from '#components/note-detail/note-body-card'
import { NoteVersionBodyComments } from '#components/note-detail/note-version-body-comments'
import type { NoteVersionComment } from '#lib/api/note-version-types'

const versionId = 'sample-version'
const comment: NoteVersionComment = {
  id: 'sample-body-comment',
  target_kind: 'note_version',
  target_id: versionId,
  parent_id: null,
  body: 'この箇所の根拠を補足してください。',
  author_kind: 'human',
  author_label: 'レビュー担当者',
  resolved: false,
  anchor_text: '対象の段落です。',
  anchor_side: 'new',
  start_line: 3,
  end_line: 3,
  created_at: '2026-01-12T03:00:00Z',
}

const queryClient = new QueryClient()

const meta = {
  title: 'NoteDetail/NoteVersionBodyComments',
  component: NoteVersionBodyComments,
  parameters: {
    layout: 'padded',
    msw: {
      handlers: [
        http.get('/api/comments', () => HttpResponse.json([comment])),
        http.post('/api/comments', () => HttpResponse.json(comment)),
        http.patch('/api/comments/:id', () => HttpResponse.json(comment)),
      ],
    },
  },
  args: {
    versionId,
    source: '# サンプル本文\n\n対象の段落です。',
  },
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <div className="max-w-3xl bg-background p-5 text-foreground">
          <NoteBodyCard>
            <Story />
          </NoteBodyCard>
        </div>
      </QueryClientProvider>
    ),
  ],
} satisfies Meta<typeof NoteVersionBodyComments>

export default meta
type Story = StoryObj<typeof meta>

export const CommentThreadVisible: Story = {
  name: 'shows the comments for the selected note version.',
}
