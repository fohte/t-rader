import type { Data, Node, Root, RootContent } from 'mdast'
import { findAndReplace } from 'mdast-util-find-and-replace'

import { REF_PREFIX_RE } from '#lib/note-utils'

const TOKEN_RE = /\[\[([^\]]+)\]\]/g
const ANNO_RE = /^anno:([A-Za-z][\w-]*)$/
const GRAPH_TOKEN_RE = /^\[\[graph:([A-Za-z][\w-]*)\]\]$/

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

// `[[stock:xxx]]` / `[[anno:xxx]]` を note-ref / note-anno 要素に差し替える remark plugin。
export function remarkNoteTokens() {
  return (tree: Root) => {
    replaceGraphParagraphs(tree)
    findAndReplace(tree, [
      TOKEN_RE,
      (_value: string, inner: string) => {
        const anno = ANNO_RE.exec(inner)
        if (anno) return noteToken('note-anno', { annoId: anno[1] ?? '' })
        if (REF_PREFIX_RE.test(inner))
          return noteToken('note-ref', { token: inner })
        return false
      },
    ])
  }
}
