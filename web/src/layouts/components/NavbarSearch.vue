<script setup lang="ts">
import { apiGet, formatVnd } from '@/api'
import { useAccessoryStore } from '@/stores/accessory'
import type { Product } from '@/types'

type SuggestResponse = { data: { products: Product[] } }

const store = useAccessoryStore()

const text = ref(store.ui.q)
const suggestions = ref<Product[]>([])
const loading = ref(false)
const menuOpen = ref(false)
const wrap = ref<HTMLElement | null>(null)

let timer: number | undefined
let abort: AbortController | null = null

watch(
  () => store.ui.q,
  value => {
    if (value !== text.value)
      text.value = value
  },
)

watch(text, value => {
  window.clearTimeout(timer)
  const query = value.trim()

  if (query.length < 2) {
    suggestions.value = []
    loading.value = false
    menuOpen.value = false

    return
  }

  timer = window.setTimeout(async () => {
    abort?.abort()
    abort = new AbortController()
    loading.value = true

    try {
      const body = await apiGet<SuggestResponse>('/suggest', { q: query, limit: 7 }, abort.signal)

      suggestions.value = body.data?.products ?? []
      menuOpen.value = suggestions.value.length > 0
    }
    catch {
      suggestions.value = []
    }
    finally {
      loading.value = false
    }
  }, 250)
})

onBeforeUnmount(() => {
  window.clearTimeout(timer)
  abort?.abort()
})

const subtitleOf = (product: Product) =>
  [product.supplier?.name, product.manufacturer_code].filter(Boolean).join(' · ') || '—'

function submit() {
  menuOpen.value = false
  store.setSearch(text.value)
}

function pick(product: Product) {
  menuOpen.value = false
  text.value = product.name
  store.setSearch(product.name)
  store.openDetail(product.slug)
}

function clear() {
  suggestions.value = []
  menuOpen.value = false
  store.clearSearch()
}
</script>

<template>
  <div
    ref="wrap"
    class="navbar-search"
  >
    <VTextField
      v-model="text"
      density="compact"
      variant="outlined"
      hide-details
      placeholder="Tìm linh kiện, mã part…"
      prepend-inner-icon="ri-search-line"
      clearable
      style="max-inline-size: 380px"
      @keydown.enter="submit"
      @keydown.esc="menuOpen = false"
      @click:clear="clear"
    >
      <template #append-inner>
        <VProgressCircular
          v-if="loading"
          indeterminate
          size="16"
          color="primary"
        />
      </template>
    </VTextField>

    <VMenu
      v-model="menuOpen"
      activator="parent"
      :close-on-content-click="false"
      :disabled="suggestions.length === 0"
      location="bottom start"
      offset="6"
      :width="380"
    >
      <VList density="compact">
        <VListItem
          v-for="suggestion in suggestions"
          :key="suggestion.id"
          :title="suggestion.name"
          :subtitle="subtitleOf(suggestion)"
          @click="pick(suggestion)"
        >
          <template #append>
            <span class="text-caption font-weight-bold">{{ formatVnd(suggestion.price) }}</span>
          </template>
        </VListItem>
      </VList>
    </VMenu>
  </div>
</template>

<style scoped>
.navbar-search {
  position: relative;
  min-inline-size: 0;
}
</style>
