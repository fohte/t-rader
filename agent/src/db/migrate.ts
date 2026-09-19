import { createSql } from '#db'
import { runMigrations } from '#db/migrations'
import { EnvError } from '#env'
import { logger } from '#logger'

// infra runs this as `node dist/db/migrate.js` in an init container.
const main = async (): Promise<void> => {
  const databaseUrl = process.env['DATABASE_URL']
  if (databaseUrl === undefined || databaseUrl === '') {
    // eslint-disable-next-line no-restricted-syntax -- init container のエントリポイント、fail fast
    throw new EnvError(['missing required env: DATABASE_URL'])
  }

  const sql = createSql(databaseUrl)
  // eslint-disable-next-line no-restricted-syntax -- sql.end() を finally で必ず呼ぶため try/finally が必要
  try {
    await runMigrations(sql)
    logger.info({}, 'migrations applied')
  } finally {
    await sql.end({ timeout: 5 })
  }
}

main().catch((err: unknown) => {
  if (err instanceof EnvError) {
    logger.error({ issues: err.issues }, 'invalid environment')
  } else {
    logger.error({ err }, 'migration failed')
  }
  process.exit(1)
})
