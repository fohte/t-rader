import type { ResolvedRef } from '#hooks/use-resolve-ref'

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
          const separatorIndex = resolved.id.indexOf('/')
          if (
            separatorIndex <= 0 ||
            separatorIndex === resolved.id.length - 1
          ) {
            return
          }
          void navigate({
            to: '/groups/$axisKey/$groupKey',
            params: {
              axisKey: resolved.id.slice(0, separatorIndex),
              groupKey: resolved.id.slice(separatorIndex + 1),
            },
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
