import type { Meta, StoryObj } from '@storybook/react-vite'

import { TaskRunPayloadPanel } from '#components/strategy-shell/task-run-payload-panel'

const meta = {
  title: 'StrategyShell/TaskRunPayloadPanel',
  component: TaskRunPayloadPanel,
} satisfies Meta<typeof TaskRunPayloadPanel>

export default meta
type Story = StoryObj<typeof meta>

const REFERENCE = {
  payloadId: '00000000-0000-4000-8000-000000000001',
  label: '出力',
  pointer: '/value',
  cursor: 0,
  limit: 2,
}

export const BoundedPage: Story = {
  name: 'shows a bounded payload page with its total count.',
  args: {
    reference: REFERENCE,
    payloadRead: {
      status: 'ready',
      manifest: {
        status: 'sealed',
        itemCount: 3,
        incompleteReason: null,
      },
      page: {
        data: [{ value: 'synthetic-a' }, { value: 'synthetic-b' }],
        totalCount: 3,
        nextCursor: 2,
      },
    },
    onSelectPayload: () => {},
    onNextPayloadPage: () => {},
  },
}

export const EditedPointer: Story = {
  name: 'shows an edited pointer before its page is requested.',
  args: {
    reference: { ...REFERENCE, pointer: '/value/items' },
    initialPointerDraft: '/value',
    payloadRead: {
      status: 'ready',
      manifest: {
        status: 'sealed',
        itemCount: 0,
        incompleteReason: null,
      },
      page: null,
    },
    onSelectPayload: () => {},
  },
}

export const Loading: Story = {
  name: 'shows the loading state while a payload page is fetched.',
  args: {
    reference: REFERENCE,
    payloadRead: { status: 'loading' },
  },
}

export const Error: Story = {
  name: 'shows an error when a payload page cannot be fetched.',
  args: {
    reference: REFERENCE,
    payloadRead: { status: 'error' },
  },
}

export const Incomplete: Story = {
  name: 'shows why an incomplete payload cannot be displayed.',
  args: {
    reference: REFERENCE,
    payloadRead: {
      status: 'ready',
      manifest: {
        status: 'incomplete',
        itemCount: 0,
        incompleteReason: 'capture_interrupted',
      },
      page: null,
    },
  },
}

export const Staging: Story = {
  name: 'shows that a staging payload is not ready to display.',
  args: {
    reference: REFERENCE,
    payloadRead: {
      status: 'ready',
      manifest: {
        status: 'staging',
        itemCount: 0,
        incompleteReason: 'capture_in_progress',
      },
      page: null,
    },
  },
}

export const PageMissing: Story = {
  name: 'shows when a sealed payload has no readable page.',
  args: {
    reference: REFERENCE,
    payloadRead: {
      status: 'ready',
      manifest: {
        status: 'sealed',
        itemCount: 0,
        incompleteReason: null,
      },
      page: null,
    },
  },
}
