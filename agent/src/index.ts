import '#bootstrap'

import { EnvError } from '#env'
import { logger } from '#logger'
import { main } from '#main'

main().catch((err: unknown) => {
  if (err instanceof EnvError) {
    logger.error({ issues: err.issues }, 'invalid environment')
  } else {
    logger.error({ err }, 'failed to start')
  }
  process.exit(1)
})
