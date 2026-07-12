export interface Logger {
  log(event: string, payload?: Readonly<Record<string, unknown>>): void
}

export const createJsonStdoutLogger = (): Logger => ({
  log(event, payload) {
    const line = JSON.stringify({ event, ...payload })
    process.stdout.write(`${line}\n`)
  },
})
