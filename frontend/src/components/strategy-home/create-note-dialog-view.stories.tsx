import type { Meta, StoryObj } from '@storybook/react-vite'

import {
  CreateNoteDialogView,
  type CreateNoteDialogViewProps,
} from '#components/strategy-home/create-note-dialog-view'
import type { components } from '#lib/api/schema.gen'

type NoteKind = components['schemas']['NoteKind']

const noteKinds: NoteKind[] = [
  {
    key: 'sample-kind',
    display_name: 'サンプル種別',
    requires_approval: false,
    description: null,
    sort_order: 0,
  },
  {
    key: 'review-kind',
    display_name: 'レビュー種別',
    requires_approval: true,
    description: null,
    sort_order: 1,
  },
]

const args: CreateNoteDialogViewProps = {
  open: true,
  onOpenChange: () => {},
  title: '',
  body: '',
  kind: '',
  noteKinds,
  noteKindsPending: false,
  noteKindsError: false,
  formError: null,
  isSubmitting: false,
  onTitleChange: () => {},
  onBodyChange: () => {},
  onKindChange: () => {},
  onSubmit: (event) => {
    event.preventDefault()
  },
}

const meta = {
  title: 'StrategyHome/CreateNoteDialogView',
  component: CreateNoteDialogView,
} satisfies Meta<typeof CreateNoteDialogView>

export default meta
type Story = StoryObj<typeof meta>

export const Open: Story = {
  name: 'shows note creation with available kinds.',
  args,
}

export const TradeReflectionDraft: Story = {
  name: 'shows a trade reflection draft with facts and a stock reference.',
  args: {
    ...args,
    showTrigger: false,
    title: '架空銘柄を売った振り返り',
    body: [
      '2099-02-10 売 10 株 @¥1,000',
      '実現損益 +¥2,200',
      '',
      '[[stock:FIC-01]]',
    ].join('\n'),
  },
}

export const LoadingKinds: Story = {
  name: 'shows the kind selector while kinds are loading.',
  args: { ...args, noteKinds: [], noteKindsPending: true },
}

export const WithoutKinds: Story = {
  name: 'shows note creation when no kinds are available.',
  args: { ...args, noteKinds: [] },
}

export const KindsLoadError: Story = {
  name: 'shows an error when kinds cannot be loaded.',
  args: { ...args, noteKinds: [], noteKindsError: true },
}

export const FormError: Story = {
  name: 'shows a note creation form error.',
  args: { ...args, formError: '入力に問題があります' },
}

export const Submitting: Story = {
  name: 'shows the note creation form while submitting.',
  args: {
    ...args,
    title: 'サンプルノート',
    body: '本文',
    isSubmitting: true,
  },
}
