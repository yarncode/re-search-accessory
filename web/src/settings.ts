import { apiGet, putSettings } from '@/api'
import type { AppSettings } from '@/api'

/**
 * UI settings live in the app's SQLite store (theme, last KiCad project, last
 * gerber folder). localStorage stays as the instant, pre-fetch copy so the
 * first paint already has the right theme.
 */

const STORAGE_KEYS: Record<string, string> = {
  theme: 'res-theme',
  kicad_project: 'res-kicad-project',
  gerber_dir: 'res-gerber-dir',
}

let loaded: AppSettings = {}
let timer: number | undefined
let pending: AppSettings = {}

export function cachedSetting(key: string): string {
  return loaded[key] != null ? String(loaded[key]) : ''
}

export async function loadSettings(): Promise<AppSettings> {
  try {
    loaded = await apiGet<AppSettings>('/settings', undefined, AbortSignal.timeout(2000))
  }
  catch {
    loaded = {}
  }

  for (const [key, storage] of Object.entries(STORAGE_KEYS)) {
    const value = loaded[key]

    if (typeof value === 'string' && value)
      localStorage.setItem(storage, value)
  }

  return loaded
}

/** Queue a setting write (debounced); localStorage is updated immediately. */
export function rememberSetting(key: string, value: string) {
  loaded[key] = value

  if (STORAGE_KEYS[key])
    localStorage.setItem(STORAGE_KEYS[key], value)

  pending[key] = value
  window.clearTimeout(timer)
  timer = window.setTimeout(() => void flushSettings(), 400)
}

export async function flushSettings() {
  const body = pending

  pending = {}

  if (!Object.keys(body).length)
    return

  try {
    await putSettings(body)
  }
  catch {
    // non-fatal: localStorage still holds the value
  }
}
