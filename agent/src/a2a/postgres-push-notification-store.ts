import type { PushNotificationConfig } from '@a2a-js/sdk'
import type { PushNotificationStore } from '@a2a-js/sdk/server'
import { and, eq } from 'drizzle-orm'
import { drizzle } from 'drizzle-orm/postgres-js'

import type { Sql } from '@/db'
import { a2aPushConfigs } from '@/db/schema'

export class PostgresPushNotificationStore implements PushNotificationStore {
  private readonly db: ReturnType<typeof drizzle>

  constructor(sql: Sql) {
    this.db = drizzle(sql)
  }

  async save(
    taskId: string,
    pushNotificationConfig: PushNotificationConfig,
  ): Promise<void> {
    // Mirrors InMemoryPushNotificationStore: a config without an explicit id
    // is keyed by the task id itself.
    const configId = pushNotificationConfig.id ?? taskId
    const config = { ...pushNotificationConfig, id: configId }
    await this.db
      .insert(a2aPushConfigs)
      .values({ taskId, configId, config })
      .onConflictDoUpdate({
        target: [a2aPushConfigs.taskId, a2aPushConfigs.configId],
        set: { config },
      })
  }

  async load(taskId: string): Promise<PushNotificationConfig[]> {
    const rows = await this.db
      .select({ config: a2aPushConfigs.config })
      .from(a2aPushConfigs)
      .where(eq(a2aPushConfigs.taskId, taskId))
    return rows.map((r) => r.config)
  }

  async delete(taskId: string, configId?: string): Promise<void> {
    const resolvedConfigId = configId ?? taskId
    await this.db
      .delete(a2aPushConfigs)
      .where(
        and(
          eq(a2aPushConfigs.taskId, taskId),
          eq(a2aPushConfigs.configId, resolvedConfigId),
        ),
      )
  }
}
