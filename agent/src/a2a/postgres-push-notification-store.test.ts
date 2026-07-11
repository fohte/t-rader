import { expect, it } from 'vitest'

import { PostgresPushNotificationStore } from '@/a2a/postgres-push-notification-store'
import { describeIfDb, setupDrizzleTx } from '@/test/db'

describeIfDb('PostgresPushNotificationStore', () => {
  const getTx = setupDrizzleTx()

  // DefaultRequestHandler.sendMessage() saves the push config before the
  // task row exists (the task is only inserted once the executor publishes
  // its first event), so this must succeed with no matching a2a_tasks row.
  it('saves a config for a task id that has no a2a_tasks row yet', async () => {
    const store = new PostgresPushNotificationStore(getTx())

    await store.save('task-not-yet-created', {
      url: 'https://example.com/hook',
    })

    expect(await store.load('task-not-yet-created')).toEqual([
      { id: 'task-not-yet-created', url: 'https://example.com/hook' },
    ])
  })

  it('defaults the config id to the task id when none is given', async () => {
    const store = new PostgresPushNotificationStore(getTx())

    await store.save('task-1', { url: 'https://example.com/hook' })

    expect(await store.load('task-1')).toEqual([
      { id: 'task-1', url: 'https://example.com/hook' },
    ])
  })

  it('replaces an existing config with the same id', async () => {
    const store = new PostgresPushNotificationStore(getTx())
    await store.save('task-2', {
      id: 'cfg-1',
      url: 'https://example.com/old',
    })

    await store.save('task-2', {
      id: 'cfg-1',
      url: 'https://example.com/new',
    })

    expect(await store.load('task-2')).toEqual([
      { id: 'cfg-1', url: 'https://example.com/new' },
    ])
  })

  it('keeps multiple distinct configs for the same task', async () => {
    const store = new PostgresPushNotificationStore(getTx())
    await store.save('task-3', { id: 'cfg-a', url: 'https://example.com/a' })
    await store.save('task-3', { id: 'cfg-b', url: 'https://example.com/b' })

    const loaded = await store.load('task-3')

    expect(
      loaded.toSorted((a, b) => (a.id ?? '').localeCompare(b.id ?? '')),
    ).toEqual([
      { id: 'cfg-a', url: 'https://example.com/a' },
      { id: 'cfg-b', url: 'https://example.com/b' },
    ])
  })

  it('deletes a config by id, defaulting to the task id', async () => {
    const store = new PostgresPushNotificationStore(getTx())
    await store.save('task-4', { url: 'https://example.com/hook' })

    await store.delete('task-4')

    expect(await store.load('task-4')).toEqual([])
  })
})
