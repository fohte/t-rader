import type {
  BlockContent,
  Data,
  DefinitionContent,
  Node,
  Root,
  RootContent,
} from 'mdast'
import { findAndReplace } from 'mdast-util-find-and-replace'

import {
  type ParsedNoteChangeReference,
  parseNoteChangeReference,
} from '#lib/note-change-reference'
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

interface NoteChangeFigure extends Node {
  type: 'noteChangeFigure'
  data: Data & { hName: string; hProperties: ParsedNoteChangeReference }
}

declare module 'mdast' {
  interface PhrasingContentMap {
    noteToken: NoteToken
  }
  interface BlockContentMap {
    noteChangeFigure: NoteChangeFigure
  }
  interface RootContentMap {
    noteGraphBlock: NoteGraphBlock
    noteChangeFigure: NoteChangeFigure
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

function noteChangeFigure(
  reference: ParsedNoteChangeReference,
): NoteChangeFigure {
  return {
    type: 'noteChangeFigure',
    data: { hName: 'note-change-figure', hProperties: reference },
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value != null && typeof value === 'object' && !Array.isArray(value)
}

function isNode(value: unknown): value is Node {
  return isRecord(value) && typeof value.type === 'string'
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

function collectChangeReferences(
  node: Node,
  references: Map<string, ParsedNoteChangeReference>,
): void {
  if (
    node.type === 'noteToken' &&
    node.data != null &&
    isRecord(node.data) &&
    node.data.hName === 'note-change-reference' &&
    isRecord(node.data.hProperties) &&
    typeof node.data.hProperties.token === 'string' &&
    typeof node.data.hProperties.instrumentId === 'string' &&
    typeof node.data.hProperties.start === 'string' &&
    typeof node.data.hProperties.end === 'string'
  ) {
    const token = node.data.hProperties.token
    references.set(token, {
      instrumentId: node.data.hProperties.instrumentId,
      start: node.data.hProperties.start,
      end: node.data.hProperties.end,
    })
  }

  if ('children' in node && Array.isArray(node.children)) {
    node.children.forEach((child: unknown) => {
      if (isNode(child)) collectChangeReferences(child, references)
    })
  }
}

function appendChangeFigures(
  children: RootContent[] | Array<BlockContent | DefinitionContent>,
): void {
  for (let index = 0; index < children.length; index += 1) {
    const child = children[index]
    if (child == null) continue

    if (child.type === 'paragraph') {
      const references = new Map<string, ParsedNoteChangeReference>()
      collectChangeReferences(child, references)
      const figures = [...references.values()].map(noteChangeFigure)
      children.splice(index + 1, 0, ...figures)
      index += figures.length
      continue
    }

    if (child.type === 'blockquote') {
      appendChangeFigures(child.children)
    } else if (child.type === 'list') {
      child.children.forEach((item) => {
        appendChangeFigures(item.children)
      })
    }
  }
}

// 参照トークンは本文に残し、解決値は表示時だけ展開する。
export function remarkNoteTokens(resolvedPriceReferences?: unknown) {
  return (tree: Root) => {
    replaceGraphParagraphs(tree)
    findAndReplace(tree, [
      TOKEN_RE,
      (token: string, inner: string) => {
        if (inner.startsWith('change:')) {
          const reference = parseNoteChangeReference(token)
          if (reference == null) return false
          const value = resolvedPriceReferenceValue(
            resolvedPriceReferences,
            token,
          )
          return noteToken('note-change-reference', {
            token,
            ...reference,
            ...(value == null ? {} : { value: String(value) }),
          })
        }

        const priceReferenceValue = inner.startsWith('price:')
          ? resolvedPriceReferenceValue(resolvedPriceReferences, token)
          : undefined
        if (priceReferenceValue != null) {
          return noteToken('note-price-reference', {
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

    // ブロック図は段落内に置けないため、変化率を含む段落の直後に図を挿入する。
    appendChangeFigures(tree.children)
  }
}
