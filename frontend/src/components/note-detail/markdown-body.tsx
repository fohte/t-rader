import type { ComponentType, ReactNode } from 'react'
import { ErrorBoundary, type FallbackProps } from 'react-error-boundary'
import Markdown, { type Components } from 'react-markdown'
import remarkGfm from 'remark-gfm'

import { GraphRenderer } from '#components/graph/graph-renderer'
import type { GraphDef, GraphEdge, GraphNode } from '#components/graph/types'
import { remarkNoteTokens } from '#components/note-detail/remark-note-tokens'
import { RefChip } from '#components/strategy-shell/ref-chip'
import type { components } from '#lib/api/schema.gen'

type ApiGraphDef = components['schemas']['GraphDef']

interface MarkdownBodyProps {
  source: string
  graphs?: ApiGraphDef[]
  onAnno?: (id: string) => void
  onRef?: (token: string) => void
}

// remarkNoteTokens が data.hName で生成するタグ名に対応する。
type NoteTokenComponents = {
  'note-ref': ComponentType<{ token: string }>
  'note-anno': ComponentType<{ annoId: string }>
  'note-graph': ComponentType<{ graphId: string }>
}

// backend は Option<T> を持つフィールドを `T | null` として返す。
// GraphRenderer が期待する型 (graph/types.ts) は `T | undefined` なので、ここで正規化する。
function toGraphNode(n: components['schemas']['GraphNode']): GraphNode {
  return {
    id: n.id,
    label: n.label,
    ref: n.ref ?? undefined,
    value: n.value ?? undefined,
    cite: n.cite ?? undefined,
    parent: n.parent ?? undefined,
    x: n.x ?? undefined,
    y: n.y ?? undefined,
  }
}

function toGraphEdge(e: components['schemas']['GraphEdge']): GraphEdge {
  return {
    source: e.source,
    target: e.target,
    label: e.label ?? undefined,
    value: e.value ?? undefined,
    cite: e.cite ?? undefined,
  }
}

function toGraphDef(g: ApiGraphDef): GraphDef {
  return {
    id: g.id,
    layout: g.layout,
    title: g.title ?? undefined,
    nodes: g.nodes.map(toGraphNode),
    edges: g.edges.map(toGraphEdge),
  }
}

function GraphNotice({ children }: { children: ReactNode }) {
  return (
    <div
      role="alert"
      className="my-3 border border-dashed border-[color:var(--color-hairline)] p-3 font-mono text-[12px] text-[color:var(--color-text-tertiary)]"
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
        className="ml-2 underline hover:text-[color:var(--color-text-secondary)]"
      >
        再試行
      </button>
    </GraphNotice>
  )
}

const HEADING3_CLASS =
  'mt-4 mb-2 text-[14px] font-bold uppercase tracking-wider text-[color:var(--color-text-secondary)]'

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
  graphs = [],
  onAnno,
  onRef,
}: MarkdownBodyProps) {
  const components: Components & NoteTokenComponents = {
    h1: ({ children }) => (
      <h1 className="mt-5 mb-2.5 text-[22px] font-bold leading-tight tracking-tight">
        {children}
      </h1>
    ),
    h2: ({ children }) => (
      <h2 className="mt-6 mb-2.5 text-[17px] font-bold leading-tight tracking-tight">
        {children}
      </h2>
    ),
    h3: ({ children }) => <h3 className={HEADING3_CLASS}>{children}</h3>,
    h4: ({ children }) => <h4 className={HEADING3_CLASS}>{children}</h4>,
    h5: ({ children }) => <h5 className={HEADING3_CLASS}>{children}</h5>,
    h6: ({ children }) => <h6 className={HEADING3_CLASS}>{children}</h6>,
    p: ({ children }) => (
      <p className="my-2.5 text-[14px] leading-[1.75] text-[color:var(--color-text-primary)]">
        {children}
      </p>
    ),
    ul: ({ children }) => (
      <ul className="my-2.5 ml-5 list-disc space-y-1 text-[14px] leading-[1.7]">
        {children}
      </ul>
    ),
    ol: ({ children }) => (
      <ol className="my-2.5 ml-5 list-decimal space-y-1 text-[14px] leading-[1.7]">
        {children}
      </ol>
    ),
    blockquote: ({ children }) => (
      <blockquote className="my-3 border-l-2 border-[color:var(--color-border-strategy)] bg-[color:var(--panel-inset)] py-1.5 pl-3 text-[13.5px] text-[color:var(--color-text-secondary)]">
        {children}
      </blockquote>
    ),
    pre: ({ children }) => (
      <pre className="my-3 overflow-x-auto border border-[color:var(--color-hairline)] bg-[color:var(--panel-inset)] p-3 font-mono text-[12.5px] leading-relaxed text-[color:var(--color-text-primary)] [&>code]:border-0 [&>code]:bg-transparent [&>code]:p-0 [&>code]:text-[1em]">
        {children}
      </pre>
    ),
    code: ({ children }) => (
      <code className="border border-[color:var(--color-hairline)] bg-[color:var(--panel-inset)] px-1 py-px font-mono text-[0.88em]">
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
        className="text-[color:var(--color-accent-strategy)] hover:underline"
      >
        {children}
      </a>
    ),
    table: ({ children }) => (
      <div className="my-3 overflow-x-auto border border-[color:var(--color-hairline)]">
        <table className="w-full border-collapse text-[13px]">{children}</table>
      </div>
    ),
    thead: ({ children }) => (
      <thead className="border-b border-[color:var(--color-border-strategy)]">
        {children}
      </thead>
    ),
    tr: ({ children }) => (
      <tr className="border-b border-[color:var(--color-hairline)] last:border-b-0">
        {children}
      </tr>
    ),
    th: ({ children, node }) => (
      <th
        align={cellAlign(node?.properties.align)}
        className="px-3 py-1.5 text-left text-[11px] font-medium uppercase tracking-wider text-[color:var(--color-text-tertiary)]"
      >
        {children}
      </th>
    ),
    td: ({ children, node }) => (
      <td
        align={cellAlign(node?.properties.align)}
        className="px-3 py-1.5 text-[color:var(--color-text-primary)]"
      >
        {children}
      </td>
    ),
    'note-ref': ({ token }) => <RefChip token={token} onOpen={onRef} />,
    'note-anno': ({ annoId }) => (
      <button
        type="button"
        onClick={() => onAnno?.(annoId)}
        title={`annotation ${annoId}`}
        className="inline-flex items-baseline gap-1 border border-[color:var(--color-accent-strategy)]/40 bg-[color:var(--panel-inset)] px-1.5 py-px font-mono text-[0.82em] text-[color:var(--color-accent-strategy)] hover:bg-[color:var(--color-accent-strategy)]/15"
      >
        <span className="font-bold">{annoId}</span>
        <span className="text-[0.85em] text-[color:var(--color-text-tertiary)]">
          annotation
        </span>
      </button>
    ),
    'note-graph': ({ graphId }) => {
      const apiDef = graphs.find((g) => g.id === graphId)
      if (!apiDef)
        return <GraphNotice>図 {graphId} が見つかりません</GraphNotice>
      const def = toGraphDef(apiDef)
      return (
        <div className="my-3">
          {def.title != null && (
            <div className="mb-1 font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
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
              className="h-[420px] border border-[color:var(--color-hairline)]"
            />
          </ErrorBoundary>
        </div>
      )
    },
  }

  return (
    <div className="text-[color:var(--color-text-primary)]">
      <Markdown
        remarkPlugins={[remarkGfm, remarkNoteTokens]}
        components={components}
      >
        {source}
      </Markdown>
    </div>
  )
}
