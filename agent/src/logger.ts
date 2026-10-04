import type { Logger } from '#logger-factory'
import { createAppLogger, withLogBindings } from '#logger-factory'

export type { Logger }
export { withLogBindings }

export const logger: Logger = createAppLogger()
