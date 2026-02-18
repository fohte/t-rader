import createFetchClient from 'openapi-fetch'
import createClient from 'openapi-react-query'

import type { paths } from '@/lib/api/schema.gen'

// openapi-fetch クライアント (React 外でも利用可能)
export const fetchClient = createFetchClient<paths>({
  baseUrl: '/',
})

// openapi-react-query クライアント (React コンポーネント内で利用)
export const $api = createClient(fetchClient)
