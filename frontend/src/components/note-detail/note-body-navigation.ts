type NoteBodyNavigationTarget =
  | {
      to: '/charts/$instrumentId'
      params: { instrumentId: string }
    }
  | {
      to: '/annotations/$annoId'
      params: { annoId: string }
    }

type Navigate = (target: NoteBodyNavigationTarget) => unknown

const STOCK_REF_PREFIX = 'stock:'

export function createNoteBodyNavigationHandlers(navigate: Navigate) {
  return {
    onRef: (token: string) => {
      if (!token.startsWith(STOCK_REF_PREFIX)) return

      const instrumentId = token.slice(STOCK_REF_PREFIX.length)
      if (instrumentId === '') return

      void navigate({
        to: '/charts/$instrumentId',
        params: { instrumentId },
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
