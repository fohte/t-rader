import type { Meta, StoryObj } from '@storybook/react-vite'

import { ExecutionPayloadManifestList } from '#components/strategy-shell/execution-payload-manifest-list'

const meta = {
  title: 'StrategyShell/ExecutionPayloadManifestList',
  component: ExecutionPayloadManifestList,
  args: {
    onSelectManifest: () => {},
    onNextPage: () => {},
    onPreviousPage: () => {},
  },
} satisfies Meta<typeof ExecutionPayloadManifestList>

export default meta
type Story = StoryObj<typeof meta>

export const UnreferencedIncompletePayload: Story = {
  name: 'an unreferenced incomplete payload can be selected.',
  args: {
    state: {
      status: 'ready',
      items: [
        {
          id: '00000000-0000-4000-8000-000000000011',
          kind: 'input',
          source: 'sample-source',
          status: 'incomplete',
          itemCount: 4,
          byteCount: 256,
          digest: null,
          incompleteReason: 'capture interrupted',
          createdAt: '2030-01-02T03:04:05Z',
        },
        {
          id: '00000000-0000-4000-8000-000000000012',
          kind: 'output',
          source: 'saved-code',
          status: 'sealed',
          itemCount: 2,
          byteCount: 128,
          digest: 'sample-digest',
          incompleteReason: null,
          createdAt: '2030-01-02T03:05:05Z',
        },
      ],
      totalCount: 9,
      nextCursor: 'sample-cursor',
      selectedPayloadId: '00000000-0000-4000-8000-000000000011',
      hasPreviousPage: false,
    },
  },
}

export const Loading: Story = {
  name: 'payload records are loading.',
  args: { state: { status: 'loading' } },
}

export const Error: Story = {
  name: 'payload records could not be loaded.',
  args: { state: { status: 'error' } },
}

export const Empty: Story = {
  name: 'no payload records are available.',
  args: {
    state: {
      status: 'ready',
      items: [],
      totalCount: 0,
      nextCursor: null,
      selectedPayloadId: null,
      hasPreviousPage: false,
    },
  },
}

export const Staging: Story = {
  name: 'a payload capture is still in progress.',
  args: {
    state: {
      status: 'ready',
      items: [
        {
          id: '00000000-0000-4000-8000-000000000013',
          kind: 'input',
          source: 'sample-source',
          status: 'staging',
          itemCount: 0,
          byteCount: 0,
          digest: null,
          incompleteReason: null,
          createdAt: '2030-01-02T03:04:05Z',
        },
      ],
      totalCount: 1,
      nextCursor: null,
      selectedPayloadId: null,
      hasPreviousPage: false,
    },
  },
}

export const PreviousPage: Story = {
  name: 'an earlier page can be opened after moving forward.',
  args: {
    state: {
      status: 'ready',
      items: [
        {
          id: '00000000-0000-4000-8000-000000000014',
          kind: 'output',
          source: 'sample-source',
          status: 'sealed',
          itemCount: 2,
          byteCount: 16,
          digest: 'synthetic-page-digest',
          incompleteReason: null,
          createdAt: '2030-01-02T03:04:05Z',
        },
      ],
      totalCount: 4,
      nextCursor: 'sample-next-cursor',
      selectedPayloadId: null,
      hasPreviousPage: true,
    },
  },
}
