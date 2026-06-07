import { Fragment, type ReactNode } from 'react'

import {
  type Block,
  type InlineToken,
  parseMarkdown,
} from '@/components/note-detail/markdown'
import { RefChip } from '@/components/strategy-shell/ref-chip'

interface Ctx {
  onAnno?: (id: string) => void
  onRef?: (token: string) => void
}

function Inline({ tokens, ctx }: { tokens: InlineToken[]; ctx: Ctx }) {
  return (
    <>
      {tokens.map((t, i) => (
        <Fragment key={i}>{renderToken(t, ctx)}</Fragment>
      ))}
    </>
  )
}

function renderToken(t: InlineToken, ctx: Ctx): ReactNode {
  switch (t.kind) {
    case 'text':
      return t.value
    case 'bold':
      return <strong className="font-semibold">{t.value}</strong>
    case 'italic':
      return <em>{t.value}</em>
    case 'code':
      return (
        <code className="border border-[color:var(--color-hairline)] bg-[color:var(--panel-inset)] px-1 py-px font-mono text-[0.88em]">
          {t.value}
        </code>
      )
    case 'ref':
      return <RefChip token={t.token} onOpen={ctx.onRef} />
    case 'anno':
      return (
        <button
          type="button"
          onClick={() => ctx.onAnno?.(t.id)}
          title={`annotation ${t.id}`}
          className="inline-flex items-baseline gap-1 border border-[color:var(--color-accent-strategy)]/40 bg-[color:var(--panel-inset)] px-1.5 py-px font-mono text-[0.82em] text-[color:var(--color-accent-strategy)] hover:bg-[color:var(--color-accent-strategy)]/15"
        >
          <span className="font-bold">{t.id}</span>
          <span className="text-[0.85em] text-[color:var(--color-text-tertiary)]">
            annotation
          </span>
        </button>
      )
  }
}

function BlockNode({ block, ctx }: { block: Block; ctx: Ctx }) {
  switch (block.kind) {
    case 'h1':
      return (
        <h1 className="mt-5 mb-2.5 text-[22px] font-bold leading-tight tracking-tight">
          <Inline tokens={block.inline} ctx={ctx} />
        </h1>
      )
    case 'h2':
      return (
        <h2 className="mt-6 mb-2.5 text-[17px] font-bold leading-tight tracking-tight">
          <Inline tokens={block.inline} ctx={ctx} />
        </h2>
      )
    case 'h3':
      return (
        <h3 className="mt-4 mb-2 text-[14px] font-bold uppercase tracking-wider text-[color:var(--color-text-secondary)]">
          <Inline tokens={block.inline} ctx={ctx} />
        </h3>
      )
    case 'p':
      return (
        <p className="my-2.5 text-[14px] leading-[1.75] text-[color:var(--color-text-primary)]">
          <Inline tokens={block.inline} ctx={ctx} />
        </p>
      )
    case 'ul':
      return (
        <ul className="my-2.5 ml-5 list-disc space-y-1 text-[14px] leading-[1.7]">
          {block.items.map((it, i) => (
            <li key={i}>
              <Inline tokens={it} ctx={ctx} />
            </li>
          ))}
        </ul>
      )
    case 'ol':
      return (
        <ol className="my-2.5 ml-5 list-decimal space-y-1 text-[14px] leading-[1.7]">
          {block.items.map((it, i) => (
            <li key={i}>
              <Inline tokens={it} ctx={ctx} />
            </li>
          ))}
        </ol>
      )
    case 'quote':
      return (
        <blockquote className="my-3 border-l-2 border-[color:var(--color-border-strategy)] bg-[color:var(--panel-inset)] py-1.5 pl-3 text-[13.5px] text-[color:var(--color-text-secondary)]">
          <Inline tokens={block.inline} ctx={ctx} />
        </blockquote>
      )
    case 'code':
      return (
        <pre className="my-3 overflow-x-auto border border-[color:var(--color-hairline)] bg-[color:var(--panel-inset)] p-3 font-mono text-[12.5px] leading-relaxed text-[color:var(--color-text-primary)]">
          <code>{block.value}</code>
        </pre>
      )
  }
}

interface MarkdownBodyProps {
  source: string
  onAnno?: (id: string) => void
  onRef?: (token: string) => void
}

export function MarkdownBody({ source, onAnno, onRef }: MarkdownBodyProps) {
  const blocks = parseMarkdown(source)
  const ctx = { onAnno, onRef }
  return (
    <div className="text-[color:var(--color-text-primary)]">
      {blocks.map((b, i) => (
        <BlockNode key={i} block={b} ctx={ctx} />
      ))}
    </div>
  )
}
