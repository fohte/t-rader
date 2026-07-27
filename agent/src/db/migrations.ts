import path from 'node:path'

import { drizzle } from 'drizzle-orm/postgres-js'
import { migrate } from 'drizzle-orm/postgres-js/migrator'

import type { Sql } from '#db'

// import.meta.url-relative resolution breaks once this module is bundled: a
// bundler may place the compiled output at a different directory depth than
// the source file. The migrations folder is deployed alongside the
// process's working directory instead, so anchor to that.
export const MIGRATIONS_FOLDER = path.join(process.cwd(), 'drizzle')

export const runMigrations = async (sql: Sql): Promise<void> => {
  const db = drizzle(sql)
  await migrate(db, { migrationsFolder: MIGRATIONS_FOLDER })
}
