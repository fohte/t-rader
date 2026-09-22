import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { HypothesisList } from '#components/hypothesis-list/hypothesis-list'
import { createStoryRouter } from '#storybook/story-router'

const meta = {
  title: 'HypothesisList/HypothesisList',
  component: HypothesisList,
} satisfies Meta<typeof HypothesisList>

export default meta
type Story = StoryObj<typeof meta>

const hypotheses = [
  {
    hypothesisId: 'example-hypothesis-one',
    title: '今後も売上の傾向が続く',
    status: 'unverified',
    updatedAt: '3 分前',
  },
  {
    hypothesisId: 'example-hypothesis-two',
    title: '継続的な改善が利益率を支える',
    status: 'supported',
    updatedAt: '昨日',
  },
]

function renderList(items: typeof hypotheses) {
  const router = createStoryRouter(
    () => <HypothesisList hypotheses={items} />,
    { paths: ['/hypotheses/$hypothesisId'] },
  )

  return <RouterProvider router={router} />
}

export const WithHypotheses: Story = {
  args: { hypotheses },
  render: (args) => renderList(args.hypotheses),
}

export const Empty: Story = {
  args: { hypotheses: [] },
  render: (args) => renderList(args.hypotheses),
}
