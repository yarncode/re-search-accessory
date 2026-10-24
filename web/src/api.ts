export class ApiError extends Error {
  status: number

  constructor(message: string, status: number) {
    super(message)
    this.status = status
  }
}

type Params = Record<string, string | number | boolean | null | undefined>

function buildUrl(path: string, params?: Params): string {
  const search = new URLSearchParams()

  if (params) {
    for (const [key, value] of Object.entries(params)) {
      if (value === undefined || value === null || value === '' || value === false)
        continue

      search.set(key, String(value))
    }
  }

  const qs = search.toString()

  return `/api${path}${qs ? `?${qs}` : ''}`
}

export async function apiGet<T>(path: string, params?: Params, signal?: AbortSignal): Promise<T> {
  const response = await fetch(buildUrl(path, params), {
    signal,
    headers: { Accept: 'application/json' },
  })

  if (!response.ok) {
    let message = `Lỗi ${response.status}`

    try {
      const body = (await response.json()) as { error?: string }

      if (body?.error)
        message = body.error
    }
    catch {
      // keep the status text
    }

    throw new ApiError(message, response.status)
  }

  return (await response.json()) as T
}

export type ImageSize = 'small' | 'medium' | 'large'

export const imageUrl = (imageId: number | null | undefined, size: ImageSize = 'medium') =>
  imageId ? `/api/image/${imageId}?size=${size}` : ''

export const formatVnd = (value: number) =>
  `${new Intl.NumberFormat('vi-VN').format(Math.round(value || 0))} ₫`

export const siteUrl = (slug: string) => `https://www.thegioiic.com/${slug}`

export async function apiSend<T>(
  method: 'POST' | 'PUT' | 'DELETE',
  path: string,
  body?: unknown,
): Promise<T> {
  const response = await fetch(`/api${path}`, {
    method,
    headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  })

  if (!response.ok) {
    let message = `Lỗi ${response.status}`

    try {
      const payload = (await response.json()) as { error?: string }

      if (payload?.error)
        message = payload.error
    }
    catch {
      // keep the status text
    }

    throw new ApiError(message, response.status)
  }

  return (await response.json()) as T
}

export type LocalFileData = { name: string; bytes: Uint8Array }

/** Native file/folder dialog opened by the backend (loopback-only). */
export async function pickNativePath(
  kind: 'folder' | 'file',
  title?: string,
  filter?: { name: string; ext: string },
): Promise<string | null> {
  const response = await apiSend<{ path: string | null; cancelled: boolean }>('POST', '/pick', {
    kind,
    title,
    filter_name: filter?.name,
    filter: filter?.ext,
  })

  return response.path ?? null
}

export type AppSettings = Record<string, unknown>

export const getSettings = () => apiGet<AppSettings>('/settings')

export const putSettings = (values: AppSettings) => apiSend<AppSettings>('PUT', '/settings', values)

export async function readLocalFiles(
  path: string,
  kind: 'gerber' | 'step',
): Promise<LocalFileData[]> {
  const response = await apiGet<{ files: { name: string; size: number; data: string }[] }>(
    '/localfiles',
    { path, kind },
  )

  return (response.files ?? []).map(file => ({
    name: file.name,
    bytes: Uint8Array.from(atob(file.data), char => char.charCodeAt(0)),
  }))
}
