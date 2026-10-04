import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { AnnotationsList } from '#components/annotations/annotations-list'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type Annotation = components['schemas']['Annotation']

const annotations: Annotation[] = [
  {
    id: '00000000-0000-0000-0000-000000000001',
    target_kind: 'stock',
    target_symbol: 'sample-code',
    timestamp: '2026-01-02T00:00:00Z',
    price: 1250,
    text: '検証用の注釈です。',
    linked_note_id: null,
    status: 'unread',
    created_by_kind: 'llm',
    execution_step_id: null,
    execution_task_id: null,
    created_at: '2026-01-02T00:00:00Z',
    updated_at: '2026-01-02T00:00:00Z',
  },
]

const meta = {
  title: 'Annotations/AnnotationsList',
  component: AnnotationsList,
  decorators: [
    (Story) => (
      <RouterProvider
        router={createStoryRouter(
          () => (
            <Story />
          ),
          { paths: ['/annotations/$annoId'] },
        )}
      />
    ),
  ],
  args: { annotations },
} satisfies Meta<typeof AnnotationsList>

export default meta
type Story = StoryObj<typeof meta>

export const WithAnnotations: Story = {
  name: 'shows annotations from the account wide list.',
}

export const Empty: Story = {
  name: 'shows the empty state when there are no annotations.',
  args: { annotations: [] },
}
