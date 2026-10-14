import type { ResolvedRef } from '#hooks/use-resolve-ref'

type NoteBodyNavigationTarget =
  | {
      to: '/stocks/$stockId'
      params: { stockId: string }
    }
  | {
      to: '/annotations/$annoId'
      params: { annoId: string }
    }

type Navigate = (target: NoteBodyNavigationTarget) => unknown

export function createNoteBodyNavigationHandlers(navigate: Navigate) {
  return {
    onRef: (_token: string, resolved: ResolvedRef) => {
      void navigate({
        to: '/stocks/$stockId',
        params: { stockId: resolved.id },
      })
    },
    onAnno: (annoId: string) => {
      void navigate({
        to: '/annotations/$annoId',
        params: { annoId },
      })
    },
  }
}
