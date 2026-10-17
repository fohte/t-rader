import type { Meta, StoryObj } from '@storybook/react-vite'
import type { ExtraProps } from 'react-markdown'

import { MarkdownBodyRangeBlock } from '#components/note-detail/markdown-body-range-block'
import {
  MarkdownBodyRangeCommentContext,
  type MarkdownBodyRangeCommentProps,
} from '#components/note-detail/markdown-body-range-comment-context'
import { getDiffCommentAnchorKey } from '#components/note-detail/note-version-line-comment-group'
import type { NoteVersionComment } from '#lib/api/note-version-types'

const node: NonNullable<ExtraProps['node']> = {
  type: 'element',
  tagName: 'p',
  properties: {},
  children: [],
  position: {
    start: { line: 3, column: 1, offset: 20 },
    end: { line: 4, column: 10, offset: 43 },
  },
}

const comment: NoteVersionComment = {
  id: 'sample-range-comment',
  target_kind: 'note_version',
  target_id: 'sample-version',
  parent_id: null,
  body: 'この段落の根拠を補足してください。',
  author_kind: 'human',
  author_label: 'レビュー担当者',
  resolved: false,
  anchor_text: '対象の段落です。',
  anchor_side: 'new',
  start_line: 3,
  end_line: 4,
  created_at: '2026-01-12T03:00:00Z',
}

const rangeComments: MarkdownBodyRangeCommentProps = {
  comments: [comment],
  activeCommentAnchor: null,
  activeCommentKey: null,
  replyingCommentId: null,
  isCreating: false,
  isReplying: false,
  resolvingCommentId: null,
  hasCreateError: false,
  hasReplyError: false,
  hasResolveError: false,
  isCommentsPending: false,
  hasCommentsError: false,
  onStartComment: () => {},
  onCancelComment: () => {},
  onCreateComment: () => {},
  onStartReply: () => {},
  onCancelReply: () => {},
  onReply: () => {},
  onToggleResolved: () => {},
}

const contextValue = {
  props: rangeComments,
  selectedRange: null,
  refChipTexts: {},
  onGutterPointerDown: () => {},
  onGutterClick: () => {},
}

const meta = {
  title: 'NoteDetail/MarkdownBodyRangeBlock',
  component: MarkdownBodyRangeBlock,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <MarkdownBodyRangeCommentContext.Provider value={contextValue}>
        <div className="max-w-3xl border border-border bg-background p-4 text-foreground">
          <Story />
        </div>
      </MarkdownBodyRangeCommentContext.Provider>
    ),
  ],
} satisfies Meta<typeof MarkdownBodyRangeBlock>

export default meta
type Story = StoryObj<typeof meta>

export const BlockShowsItsCommentThread: Story = {
  name: 'shows the thread below the last line in the block range.',
  args: {
    node,
    children: <p className="my-2.5 text-sm">対象の段落です。次の行です。</p>,
  },
}

export const BlockShowsItsCommentThreadWithoutGutter: Story = {
  name: 'shows the thread beneath the block without a gutter.',
  args: {
    node,
    showGutter: false,
    children: <p className="my-2.5 text-sm">対象の段落です。次の行です。</p>,
  },
}

export const TableRowShowsItsCommentThread: Story = {
  name: 'shows the thread below a table row in the range.',
  args: {
    node,
    variant: 'table-row',
    children: [
      <td key="item" className="px-3 py-1.5">
        架空の項目
      </td>,
      <td key="value" className="px-3 py-1.5">
        架空の値
      </td>,
    ],
  },
  render: (args) => (
    <table className="w-full border-collapse text-sm">
      <tbody>
        <MarkdownBodyRangeBlock {...args}>
          {args.children}
        </MarkdownBodyRangeBlock>
      </tbody>
    </table>
  ),
}

export const BlockShowsItsCommentComposer: Story = {
  name: 'shows the composer below the selected source range.',
  args: {
    node,
    children: <p className="my-2.5 text-sm">対象の段落です。次の行です。</p>,
  },
  decorators: [
    (Story) => {
      const activeCommentAnchor = {
        side: 'new' as const,
        lineNumber: 3,
        endLineNumber: 4,
        text: '対象の段落です。',
      }
      return (
        <MarkdownBodyRangeCommentContext.Provider
          value={{
            ...contextValue,
            props: {
              ...rangeComments,
              comments: [],
              activeCommentAnchor,
              activeCommentKey: getDiffCommentAnchorKey(activeCommentAnchor),
            },
          }}
        >
          <div className="max-w-3xl border border-border bg-background p-4 text-foreground">
            <Story />
          </div>
        </MarkdownBodyRangeCommentContext.Provider>
      )
    },
  ],
}
