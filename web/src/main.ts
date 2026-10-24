import { Buffer } from 'buffer'

import { createApp } from 'vue'

import App from '@/App.vue'
import { loadSettings } from '@/settings'
import { registerPlugins } from '@core/utils/plugins'

// gerber-to-svg is written for Node and uses Buffer at module load.
;(globalThis as unknown as { Buffer?: typeof Buffer }).Buffer ??= Buffer

// Styles
import '@core/scss/template/index.scss'
import '@layouts/styles/index.scss'
import '@/assets/styles/accessory.scss'

// Create vue app
const app = createApp(App)

async function bootstrap() {
  // Settings live in SQLite; load them before the plugins so the very first
  // paint already uses the saved theme and the remembered folder paths.
  await loadSettings()

  registerPlugins(app)
  app.mount('#app')
}

void bootstrap()
