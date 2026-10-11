import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { DetailBackLink } from '#components/detail-back-link'
import { createStoryRouter } from '#storybook/story-router'

function createDetailBackLinkRouter(
  component: () => React.ReactNode,
  initialPath: string,
) {
  return createStoryRouter(component, {
    paths: ['/notes', '/annotations', '/notes/$noteId', '/annotations/$annoId'],
    initialPath,
  })
}

const meta = {
  title: 'Navigation/DetailBackLink',
  component: DetailBackLink,
  parameters: { layout: 'fullscreen' },
  render: (args) => (
    <RouterProvider
      router={createDetailBackLinkRouter(
        () => (
          <DetailBackLink {...args} />
        ),
        args.tab === 'annotations'
          ? '/annotations/sample-annotation-id'
          : '/notes/sample-note-id',
      )}
    />
  ),
} satisfies Meta<typeof DetailBackLink>

export default meta
type Story = StoryObj<typeof meta>

export const NoteDetail: Story = {
  name: 'returns from a note detail page to the note list.',
  args: { label: 'ノート一覧に戻る' },
}

export const AnnotationDetail: Story = {
  name: 'returns from an annotation detail page to the annotations tab.',
  args: { tab: 'annotations', label: 'アノテーション一覧に戻る' },
}
