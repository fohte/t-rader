import type { Data, Node, Root } from 'mdast'
import { findAndReplace } from 'mdast-util-find-and-replace'

import { REF_PREFIX_RE } from '#lib/note-utils'

const TOKEN_RE = /\[\[([^\]]+)\]\]/g
const ANNO_RE = /^anno:([A-Za-z][\w-]*)$/

interface NoteToken extends Node {
  type: 'noteToken'
  data: Data & { hName: string; hProperties: Record<string, string> }
}

declare module 'mdast' {
  interface PhrasingContentMap {
    noteToken: NoteToken
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

// `[[stock:xxx]]` / `[[anno:xxx]]` を note-ref / note-anno 要素に差し替える remark plugin。
export function remarkNoteTokens() {
  return (tree: Root) => {
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
