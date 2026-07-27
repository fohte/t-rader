import { Result } from 'neverthrow'

export const parseJson = Result.fromThrowable((raw: string): unknown =>
  JSON.parse(raw),
)
