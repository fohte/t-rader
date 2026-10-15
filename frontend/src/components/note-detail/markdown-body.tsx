import { Link } from '@tanstack/react-router'
import {
  type ComponentType,
  createContext,
  type ReactNode,
  useContext,
  useEffect,
  useMemo,
} from 'react'
import { ErrorBoundary, type FallbackProps } from 'react-error-boundary'
import Markdown, { type Components, type ExtraProps } from 'react-markdown'
import remarkGfm from 'remark-gfm'

import { GraphRenderer } from '#components/graph/graph-renderer'
import { ChangeReferenceFigure } from '#components/note-detail/change-reference-figure'
import { toGraphDef } from '#components/note-detail/markdown-body-graph-def'
import { MarkdownBodyRangeBlock } from '#components/note-detail/markdown-body-range-block'
import {
  MarkdownBodyRangeCommentContext,
  type MarkdownBodyRangeCommentProps,
  MarkdownBodyTableCellGutterContext,
  NOTE_BODY_INDENT_REM,
} from '#components/note-detail/markdown-body-range-comment-context'
import { MarkdownBodyTextSelectionAction } from '#components/note-detail/markdown-body-text-selection-action'
import {
  NoteChangeReference,
  NoteEvaluationEvidenceReference,
  NoteIndicatorObservationReference,
  NotePriceReference,
  NoteValuationReference,
} from '#components/note-detail/note-evidence-reference'
import { remarkNoteTokens } from '#components/note-detail/remark-note-tokens'
import { useMarkdownBodyCommentSelection } from '#components/note-detail/use-markdown-body-comment-selection'
import { RefChip } from '#components/strategy-shell/ref-chip'
import type { ResolvedRef } from '#hooks/use-resolve-ref'
import type { components } from '#lib/api/schema.gen'
import type { ParsedNoteChangeReference } from '#lib/note-change-reference'
import type { ParsedNotePriceReference } from '#lib/note-price-reference'
import type {
  ParsedNoteIndicatorObservationReference,
  ParsedNoteValuationReference,
} from '#lib/note-valuation-indicator-reference'

type ApiGraphDef = components['schemas']['GraphDef']
type NoteLinkItem = components['schemas']['NoteLinkItem']
type MarkdownNode = NonNullable<ExtraProps['node']>

const EMPTY_GRAPHS: ApiGraphDef[] = []
const EMPTY_NOTE_LINKS: NoteLinkItem[] = []

interface MarkdownBodyProps {
  source: string
  graphs?: ApiGraphDef[]
  noteLinks?: NoteLinkItem[]
  resolvedPriceReferences?: components['schemas']['Value']
  onAnno?: (id: string) => void
  onRef?: (token: string, resolved: ResolvedRef) => void
  rangeComments?: MarkdownBodyRangeCommentProps
}

// remarkNoteTokens が data.hName で生成するタグ名に対応する。
type NoteTokenComponents = {
  'note-ref': ComponentType<{ token: string }>
  'note-anno': ComponentType<{ annoId: string }>
  'note-graph': ComponentType<{ graphId: string; node?: MarkdownNode }>
  'note-link': ComponentType<{ noteId: string; token: string }>
  'note-price-reference': ComponentType<
    ParsedNotePriceReference & { value: string }
  >
  'note-eval-reference': ComponentType<{
    evaluationId: string
    outputName: string
    value: string
  }>
  'note-valuation-reference': ComponentType<
    ParsedNoteValuationReference & { value: string }
  >
  'note-indicator-reference': ComponentType<
    ParsedNoteIndicatorObservationReference & { value: string }
  >
  'note-change-reference': ComponentType<
    ParsedNoteChangeReference & { token: string; value?: string }
  >
  'note-change-figure': ComponentType<
    ParsedNoteChangeReference & { node?: MarkdownNode }
  >
}

const MarkdownBodyListDepthContext = createContext(0)

function GraphNotice({ children }: { children: ReactNode }) {
  return (
    <div
      role="alert"
      className="my-3 border border-dashed border-border p-3 font-mono text-xs text-muted-foreground"
    >
      {children}
    </div>
  )
}

function GraphErrorFallback({ resetErrorBoundary }: FallbackProps) {
  return (
    <GraphNotice>
      図を表示できませんでした
      <button
        type="button"
        onClick={resetErrorBoundary}
        className="ml-2 underline hover:text-muted-foreground-strong"
      >
        再試行
      </button>
    </GraphNotice>
  )
}

const HEADING3_CLASS =
  'mt-4 mb-2 text-sm font-bold uppercase tracking-wider text-muted-foreground-strong'

// hast-util-to-jsx-runtime の tableCellAlignToStyle (既定 true) が hast の
// align を style.textAlign に変換して prop から落とすため、生の hast node
// から直接読む。
function cellAlign(value: unknown): 'left' | 'right' | 'center' | undefined {
  return value === 'left' || value === 'right' || value === 'center'
    ? value
    : undefined
}

export function MarkdownBody({
  source,
  graphs = EMPTY_GRAPHS,
  noteLinks = EMPTY_NOTE_LINKS,
  resolvedPriceReferences,
  onAnno,
  onRef,
  rangeComments,
}: MarkdownBodyProps) {
  const selection = useMarkdownBodyCommentSelection({
    enabled: rangeComments != null,
    source,
    onStartComment: rangeComments?.onStartComment ?? (() => {}),
  })
  useEffect(() => {
    if (rangeComments?.activeCommentAnchor == null) {
      selection.clearBlockSelection()
    }
  }, [rangeComments?.activeCommentAnchor, selection.clearBlockSelection])

  const commentProps =
    rangeComments == null
      ? null
      : {
          ...rangeComments,
          onCancelComment: () => {
            selection.clearBlockSelection()
            rangeComments.onCancelComment()
          },
        }
  const commentContextValue =
    commentProps == null
      ? null
      : {
          props: commentProps,
          selectedRange: selection.selectedRange,
          onGutterPointerDown: selection.onGutterPointerDown,
          onGutterClick: selection.onGutterClick,
        }
  const hasRangeComments = rangeComments != null
  const components = useMemo<Components & NoteTokenComponents>(
    () => ({
      h1: ({ children, node }) => (
        <MarkdownBodyRangeBlock
          node={node}
          element={
            <h1 className="mt-5 mb-2.5 text-2xl font-bold leading-tight tracking-tight">
              {children}
            </h1>
          }
        />
      ),
      h2: ({ children, node }) => (
        <MarkdownBodyRangeBlock
          node={node}
          element={
            <h2 className="mt-6 mb-2.5 text-lg font-bold leading-tight tracking-tight">
              {children}
            </h2>
          }
        />
      ),
      h3: ({ children, node }) => (
        <MarkdownBodyRangeBlock
          node={node}
          element={<h3 className={HEADING3_CLASS}>{children}</h3>}
        />
      ),
      h4: ({ children, node }) => (
        <MarkdownBodyRangeBlock
          node={node}
          element={<h4 className={HEADING3_CLASS}>{children}</h4>}
        />
      ),
      h5: ({ children, node }) => (
        <MarkdownBodyRangeBlock
          node={node}
          element={<h5 className={HEADING3_CLASS}>{children}</h5>}
        />
      ),
      h6: ({ children, node }) => (
        <MarkdownBodyRangeBlock
          node={node}
          element={<h6 className={HEADING3_CLASS}>{children}</h6>}
        />
      ),
      p: ({ children, node }) => (
        <MarkdownBodyRangeBlock
          node={node}
          element={
            <p className="my-2.5 text-sm leading-relaxed text-foreground">
              {children}
            </p>
          }
        />
      ),
      ul: ({ children }) => {
        const listDepth = useContext(MarkdownBodyListDepthContext)
        return (
          <MarkdownBodyListDepthContext.Provider value={listDepth + 1}>
            <ul
              className="my-2.5 list-disc space-y-1 text-sm leading-relaxed"
              style={{ marginLeft: `${String(NOTE_BODY_INDENT_REM)}rem` }}
            >
              {children}
            </ul>
          </MarkdownBodyListDepthContext.Provider>
        )
      },
      ol: ({ children }) => {
        const listDepth = useContext(MarkdownBodyListDepthContext)
        return (
          <MarkdownBodyListDepthContext.Provider value={listDepth + 1}>
            <ol
              className="my-2.5 list-decimal space-y-1 text-sm leading-relaxed"
              style={{ marginLeft: `${String(NOTE_BODY_INDENT_REM)}rem` }}
            >
              {children}
            </ol>
          </MarkdownBodyListDepthContext.Provider>
        )
      },
      li: ({ children, node }) => {
        const listDepth = useContext(MarkdownBodyListDepthContext)
        return (
          <MarkdownBodyRangeBlock
            node={node}
            listDepth={listDepth}
            element={<li className="my-1">{children}</li>}
          />
        )
      },
      blockquote: ({ children, node }) => (
        <MarkdownBodyRangeBlock
          node={node}
          element={
            <blockquote className="my-3 border-l-2 border-border bg-surface-strong py-1.5 pl-3 text-sm text-muted-foreground-strong">
              {children}
            </blockquote>
          }
        />
      ),
      pre: ({ children, node }) => (
        <MarkdownBodyRangeBlock
          node={node}
          element={
            <pre className="my-3 overflow-x-auto border border-border bg-surface-strong p-3 font-mono text-xs leading-relaxed text-foreground [&>code]:border-0 [&>code]:bg-transparent [&>code]:p-0 [&>code]:text-xs">
              {children}
            </pre>
          }
        />
      ),
      code: ({ children }) => (
        // text-em-88 は markdown 中の任意の位置 (見出し内含む) に埋め込まれる
        // ため、周辺テキストに追従する相対値のまま残す。固定トークンに丸めると
        // 見出し内で不自然に縮小する。
        <code className="border border-border bg-surface-strong px-1 py-px font-mono text-em-88">
          {children}
        </code>
      ),
      strong: ({ children }) => (
        <strong className="font-semibold">{children}</strong>
      ),
      a: ({ children, href }) => (
        <a
          href={href}
          target="_blank"
          rel="noopener noreferrer"
          className="text-primary hover:underline"
        >
          {children}
        </a>
      ),
      table: ({ children }) => (
        <div
          className={`my-3 overflow-x-auto ${hasRangeComments ? 'relative -ml-11 pl-11' : 'border border-border'}`}
        >
          <table
            className={`w-full border-collapse text-sm ${hasRangeComments ? 'ring-1 ring-inset ring-border' : ''}`}
          >
            {children}
          </table>
        </div>
      ),
      thead: ({ children }) => (
        <thead className="border-b border-border">{children}</thead>
      ),
      tr: ({ children, node }) => (
        <MarkdownBodyRangeBlock variant="table-row" node={node}>
          {children}
        </MarkdownBodyRangeBlock>
      ),
      th: ({ children, node }) => {
        const tableGutter = useContext(MarkdownBodyTableCellGutterContext)
        return (
          <th
            align={cellAlign(node?.properties.align)}
            className={`px-3 py-1.5 text-left text-2xs font-medium uppercase tracking-wider text-muted-foreground ${tableGutter == null ? '' : 'relative'}`}
          >
            {children}
            {tableGutter}
          </th>
        )
      },
      td: ({ children, node }) => {
        const tableGutter = useContext(MarkdownBodyTableCellGutterContext)
        return (
          <td
            align={cellAlign(node?.properties.align)}
            className={`px-3 py-1.5 text-foreground ${tableGutter == null ? '' : 'relative'}`}
          >
            {children}
            {tableGutter}
          </td>
        )
      },
      'note-ref': ({ token }) => <RefChip token={token} onOpenRef={onRef} />,
      'note-link': ({ noteId, token }) => {
        const noteLink = noteLinks.find((link) => link.note_id === noteId)
        if (!noteLink) return <>{`[[${token}]]`}</>
        return (
          <Link
            to="/notes/$noteId"
            params={{ noteId }}
            search={{ version_id: noteLink.version_id ?? undefined }}
            className="text-primary hover:underline"
          >
            {noteLink.title ?? 'タイトルなし'}
            {noteLink.version_id == null
              ? ' (現行バージョンに追従)'
              : noteLink.version_no == null
                ? ''
                : ` (v${String(noteLink.version_no)})`}
          </Link>
        )
      },
      'note-anno': ({ annoId }) => (
        <button
          type="button"
          onClick={() => onAnno?.(annoId)}
          title={`annotation ${annoId}`}
          // text-em-82 も同様に markdown 中の任意の位置に埋め込まれるため、
          // 周辺テキストに追従する相対値のまま残す。
          className="inline-flex items-baseline gap-1 border border-primary/40 bg-surface-strong px-1.5 py-px font-mono text-em-82 text-primary hover:bg-primary/15"
        >
          <span className="font-bold">{annoId}</span>
          <span className="text-em-85 text-muted-foreground">annotation</span>
        </button>
      ),
      'note-price-reference': (reference) => (
        <NotePriceReference {...reference} />
      ),
      'note-eval-reference': (reference) => (
        <NoteEvaluationEvidenceReference {...reference} />
      ),
      'note-valuation-reference': (reference) => (
        <NoteValuationReference {...reference} />
      ),
      'note-indicator-reference': (reference) => (
        <NoteIndicatorObservationReference {...reference} />
      ),
      'note-change-reference': (reference) => (
        <NoteChangeReference {...reference} />
      ),
      'note-change-figure': ({ node, ...reference }) => (
        <MarkdownBodyRangeBlock
          node={node}
          showGutter={false}
          element={<ChangeReferenceFigure {...reference} />}
        />
      ),
      'note-graph': ({ graphId, node }) => {
        const apiDef = graphs.find((g) => g.id === graphId)
        if (!apiDef)
          return (
            <MarkdownBodyRangeBlock
              node={node}
              element={<GraphNotice>図 {graphId} が見つかりません</GraphNotice>}
            />
          )
        const def = toGraphDef(apiDef)
        return (
          <MarkdownBodyRangeBlock
            node={node}
            element={
              <div className="my-3">
                {def.title != null && (
                  <div className="mb-1 font-mono text-2xs uppercase tracking-wider text-muted-foreground">
                    {def.title}
                  </div>
                )}
                <ErrorBoundary
                  FallbackComponent={GraphErrorFallback}
                  resetKeys={[apiDef]}
                >
                  <GraphRenderer
                    def={def}
                    onOpenRef={onRef}
                    className="h-note-graph border border-border"
                  />
                </ErrorBoundary>
              </div>
            }
          />
        )
      },
    }),
    [
      graphs,
      hasRangeComments,
      noteLinks,
      onAnno,
      onRef,
      resolvedPriceReferences,
    ],
  )

  return (
    <div
      ref={selection.rootRef}
      className="text-foreground"
      style={
        hasRangeComments
          ? {
              marginLeft: `-${String(NOTE_BODY_INDENT_REM)}rem`,
              paddingLeft: `${String(NOTE_BODY_INDENT_REM)}rem`,
            }
          : undefined
      }
      onMouseUp={selection.onMouseUp}
      onTouchEnd={selection.onTouchEnd}
      onPointerMove={selection.onPointerMove}
      onPointerUp={selection.onPointerUp}
      onPointerCancel={selection.onPointerCancel}
    >
      {rangeComments?.isCommentsPending === true && (
        <p className="mb-2 font-mono text-2xs text-muted-foreground">
          コメントを読み込み中…
        </p>
      )}
      {rangeComments?.hasCommentsError === true && (
        <p className="mb-2 font-mono text-2xs text-primary">
          コメントを読み込めませんでした
        </p>
      )}
      {rangeComments?.hasResolveError === true && (
        <p role="alert" className="mb-2 font-mono text-2xs text-primary">
          コメントの解決状態を更新できませんでした
        </p>
      )}
      <MarkdownBodyRangeCommentContext.Provider value={commentContextValue}>
        <Markdown
          remarkPlugins={[
            remarkGfm,
            [remarkNoteTokens, resolvedPriceReferences],
          ]}
          components={components}
        >
          {source}
        </Markdown>
      </MarkdownBodyRangeCommentContext.Provider>
      <MarkdownBodyTextSelectionAction
        selection={selection.textSelectionAction}
        onStartComment={selection.onStartTextComment}
      />
    </div>
  )
}
