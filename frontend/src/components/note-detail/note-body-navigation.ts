import type { ResolvedRef } from '#hooks/use-resolve-ref'
import { parseGroupRefId } from '#lib/ref-id'

type NoteBodyNavigationTarget =
  | {
      to: '/stocks/$stockId'
      params: { stockId: string }
    }
  | {
      to: '/indicators/$indicatorId'
      params: { indicatorId: string }
    }
  | {
      to: '/groups/$axisKey/$groupKey'
      params: { axisKey: string; groupKey: string }
    }
  | {
      to: '/annotations/$annoId'
      params: { annoId: string }
    }

type Navigate = (target: NoteBodyNavigationTarget) => unknown

export function createNoteBodyNavigationHandlers(navigate: Navigate) {
  return {
    onRef: (_token: string, resolved: ResolvedRef) => {
      switch (resolved.kind) {
        case 'stock':
          void navigate({
            to: '/stocks/$stockId',
            params: { stockId: resolved.id },
          })
          return
        case 'indicator':
          void navigate({
            to: '/indicators/$indicatorId',
            params: { indicatorId: resolved.id },
          })
          return
        case 'group': {
          const groupRefId = parseGroupRefId(resolved.id)
          if (groupRefId == null) return
          void navigate({
            to: '/groups/$axisKey/$groupKey',
            params: groupRefId,
          })
        }
      }
    },
    onAnno: (annoId: string) => {
      void navigate({
        to: '/annotations/$annoId',
        params: { annoId },
      })
    },
  }
}
