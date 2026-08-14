import type { PhrasingContent, Root } from 'mdast'
import { findAndReplace } from 'mdast-util-find-and-replace'

import { REF_PREFIX_RE } from '#lib/note-utils'

const TOKEN_RE = /\[\[([^\]]+)\]\]/g
const ANNO_RE = /^anno:([A-Za-z][\w-]*)$/

// mdast の PhrasingContent は既知のノード型の閉じた union なので、
// hName 経由で hast 要素化するだけの独自ノードはここで unknown 経由キャストする。
function noteToken(
  hName: string,
  hProperties: Record<string, string>,
): PhrasingContent {
  // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- mdast の PhrasingContent は既知ノードの閉じた union なので、hName で hast 要素化するだけの独自ノードは as で通す以外に方法がない
  return {
    type: 'noteToken',
    data: { hName, hProperties },
  } as unknown as PhrasingContent
}

// `[[stock:xxx]]` / `[[anno:xxx]]` を note-ref / note-anno 要素に差し替える remark plugin。
// 図トークン (`[[graph:xxx]]`) を足す場合もここに分岐を足すだけでよい。
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
