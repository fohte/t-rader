import type { Data, Node, Root, RootContent } from 'mdast'
import { findAndReplace } from 'mdast-util-find-and-replace'

import { REF_PREFIX_RE } from '#lib/note-utils'

const TOKEN_RE = /\[\[([^\]]+)\]\]/g
const ANNO_RE = /^anno:([A-Za-z0-9][\w-]*)$/
const GRAPH_TOKEN_RE = /^\[\[graph:([A-Za-z][\w-]*)\]\]$/
const NOTE_TOKEN_RE =
  /^note:([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})(@current)?$/i

interface NoteToken extends Node {
  type: 'noteToken'
  data: Data & { hName: string; hProperties: Record<string, string> }
}

interface NoteGraphBlock extends Node {
  type: 'noteGraphBlock'
  data: Data & { hName: string; hProperties: Record<string, string> }
}

declare module 'mdast' {
  interface PhrasingContentMap {
    noteToken: NoteToken
  }
  interface RootContentMap {
    noteGraphBlock: NoteGraphBlock
  }
}

function noteToken(
  hName: string,
  hProperties: Record<string, string>,
): NoteToken {
  return {
    type: 'noteToken',
    data: { hName, hProperties },
  }
}

function noteGraphBlock(graphId: string): NoteGraphBlock {
  return {
    type: 'noteGraphBlock',
    data: { hName: 'note-graph', hProperties: { graphId } },
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value != null && typeof value === 'object' && !Array.isArray(value)
}

function resolvedPriceReferenceValue(
  references: unknown,
  token: string,
): number | undefined {
  if (!isRecord(references)) return undefined

  const reference = references[token]
  if (!isRecord(reference)) return undefined

  const value = reference.value
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined
}

// `[[graph:g1]]` は図 (ブロック要素) を指す。<p> の子には div を置けないため、
// 段落全体がこのトークン 1 つだけで構成されている場合に限り、段落ごとブロック要素に差し替える。
// 他のテキストと同じ段落に混在する場合は下の findAndReplace の対象外のまま素通りする。
function replaceGraphParagraphs(root: Root): void {
  root.children = root.children.map((child): RootContent => {
    if (child.type !== 'paragraph' || child.children.length !== 1) {
      return child
    }
    const [only] = child.children
    if (only?.type !== 'text') return child
    const match = GRAPH_TOKEN_RE.exec(only.value.trim())
    const graphId = match?.[1]
    if (graphId == null) return child
    return noteGraphBlock(graphId)
  })
}

// 本文中の価格参照はトークンのまま保ち、解決値は表示時だけ展開する。
export function remarkNoteTokens(resolvedPriceReferences?: unknown) {
  return (tree: Root) => {
    replaceGraphParagraphs(tree)
    findAndReplace(tree, [
      TOKEN_RE,
      (token: string, inner: string) => {
        const priceReferenceKind = inner.startsWith('price:')
          ? 'price'
          : inner.startsWith('change:')
            ? 'change'
            : undefined
        const priceReferenceValue =
          priceReferenceKind == null
            ? undefined
            : resolvedPriceReferenceValue(resolvedPriceReferences, token)
        if (priceReferenceKind != null && priceReferenceValue != null) {
          return noteToken('note-price-reference', {
            kind: priceReferenceKind,
            value: String(priceReferenceValue),
          })
        }

        const note = NOTE_TOKEN_RE.exec(inner)
        if (note)
          return noteToken('note-link', {
            noteId: note[1]?.toLowerCase() ?? '',
            token: inner,
          })
        const anno = ANNO_RE.exec(inner)
        if (anno) return noteToken('note-anno', { annoId: anno[1] ?? '' })
        if (REF_PREFIX_RE.test(inner))
          return noteToken('note-ref', { token: inner })
        return false
      },
    ])
  }
}
