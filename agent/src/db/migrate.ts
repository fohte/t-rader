import { createSql } from '@/db'
import { runMigrations } from '@/db/migrations'
import { EnvError } from '@/env'

// infra runs this as `node dist/db/migrate.js` in an init container.
const main = async (): Promise<void> => {
  const databaseUrl = process.env['DATABASE_URL']
  if (databaseUrl === undefined || databaseUrl === '') {
    throw new EnvError(['missing required env: DATABASE_URL'])
  }

  const sql = createSql(databaseUrl)
  try {
    await runMigrations(sql)
    console.log('migrations applied')
  } finally {
    await sql.end({ timeout: 5 })
  }
}

main().catch((err: unknown) => {
  if (err instanceof EnvError) {
    for (const issue of err.issues) console.error(issue)
  } else {
    console.error(err)
  }
  process.exit(1)
})
