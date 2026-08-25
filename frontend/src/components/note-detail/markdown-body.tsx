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
  'mt-4 mb-2 text-sm font-bold uppercase tracking-wider text-text-secondary'

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
      <h1 className="mt-5 mb-2.5 text-2xl font-bold leading-tight tracking-tight">
        {children}
      </h1>
    ),
    h2: ({ children }) => (
      <h2 className="mt-6 mb-2.5 text-lg font-bold leading-tight tracking-tight">
        {children}
      </h2>
    ),
    h3: ({ children }) => <h3 className={HEADING3_CLASS}>{children}</h3>,
    h4: ({ children }) => <h4 className={HEADING3_CLASS}>{children}</h4>,
    h5: ({ children }) => <h5 className={HEADING3_CLASS}>{children}</h5>,
    h6: ({ children }) => <h6 className={HEADING3_CLASS}>{children}</h6>,
    p: ({ children }) => (
      <p className="my-2.5 text-sm leading-relaxed text-foreground">
        {children}
      </p>
    ),
    ul: ({ children }) => (
      <ul className="my-2.5 ml-5 list-disc space-y-1 text-sm leading-relaxed">
        {children}
      </ul>
    ),
    ol: ({ children }) => (
      <ol className="my-2.5 ml-5 list-decimal space-y-1 text-sm leading-relaxed">
        {children}
      </ol>
    ),
    blockquote: ({ children }) => (
      <blockquote className="my-3 border-l-2 border-border bg-surface-strong py-1.5 pl-3 text-sm text-muted-foreground-strong">
        {children}
      </blockquote>
    ),
    pre: ({ children }) => (
      <pre className="my-3 overflow-x-auto border border-border bg-surface-strong p-3 font-mono text-xs leading-relaxed text-foreground [&>code]:border-0 [&>code]:bg-transparent [&>code]:p-0 [&>code]:text-xs">
        {children}
      </pre>
    ),
    code: ({ children }) => (
      // text-[0.88em] は markdown 中の任意の位置 (見出し内含む) に埋め込まれる
      // ため、周辺テキストに追従する相対値のまま残す。固定トークンに丸めると
      // 見出し内で不自然に縮小する。
      <code className="border border-border bg-surface-strong px-1 py-px font-mono text-[0.88em]">
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
      <div className="my-3 overflow-x-auto border border-border">
        <table className="w-full border-collapse text-sm">{children}</table>
      </div>
    ),
    thead: ({ children }) => (
      <thead className="border-b border-border">{children}</thead>
    ),
    tr: ({ children }) => (
      <tr className="border-b border-border last:border-b-0">{children}</tr>
    ),
    th: ({ children, node }) => (
      <th
        align={cellAlign(node?.properties.align)}
        className="px-3 py-1.5 text-left text-2xs font-medium uppercase tracking-wider text-muted-foreground"
      >
        {children}
      </th>
    ),
    td: ({ children, node }) => (
      <td
        align={cellAlign(node?.properties.align)}
        className="px-3 py-1.5 text-foreground"
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
        // text-[0.82em] も同様に markdown 中の任意の位置に埋め込まれるため、
        // 周辺テキストに追従する相対値のまま残す。
        className="inline-flex items-baseline gap-1 border border-primary/40 bg-surface-strong px-1.5 py-px font-mono text-[0.82em] text-primary hover:bg-primary/15"
      >
        <span className="font-bold">{annoId}</span>
        <span className="text-[0.85em] text-muted-foreground">annotation</span>
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
      )
    },
  }

  return (
    <div className="text-foreground">
      <Markdown
        remarkPlugins={[remarkGfm, remarkNoteTokens]}
        components={components}
      >
        {source}
      </Markdown>
    </div>
  )
}
