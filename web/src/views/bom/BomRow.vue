<script setup lang="ts">
import { computed, defineAsyncComponent, ref, watch } from 'vue'
import { apiGet, formatVnd, imageUrl, siteUrl } from '@/api'
import { PURCHASE_LABEL, useBomStore } from '@/stores/bom'
import AsyncBlock from '@/components/AsyncBlock.vue'
import type { BomItem } from '@/utils/bom'
import type { Product, SearchResponse } from '@/types'

// three.js + OpenCascade (~800 KB) must not be part of the BOM page chunk: load
// them only when a row's 3D dialog is opened (or hovered).
const loadFootprintViewer = () => import('@/views/viewer/FootprintViewer.vue')

const FootprintViewer = defineAsyncComponent({
  loader: loadFootprintViewer,
  loadingComponent: AsyncBlock,
  delay: 80,
})

const props = defineProps<{ item: BomItem }>()

const PURCHASE_OPTIONS = [
  { value: 'need', title: 'Cần mua' },
  { value: 'ordered', title: 'Đã đặt' },
  { value: 'received', title: 'Đã nhận' },
] as const

const emit = defineEmits<{
  'toggle-ok': []
  'pick': [product: Product]
  'open': [slug: string]
  'update-qty': [qty: number]
  'update-input': [value: string]
  'analyze-row': []
  'purchase': [status: BomItem['purchase']]
  'ordered-at': [value: string]
  'supplier': [value: string]
  'optimize': []
}>()

const store = useBomStore()

const panel = ref(false)
const fp3dOpen = ref(false)
const query = ref(props.item.input)
const results = ref<Product[]>([])
const searching = ref(false)
const searched = ref(false)

const qtyDraft = ref(String(props.item.qty))
const inputDraft = ref(props.item.input)
const orderedDraft = ref(props.item.orderedAt)
const supplierDraft = ref(props.item.supplier)

watch(() => props.item.qty, value => (qtyDraft.value = String(value)))
watch(() => props.item.input, value => (inputDraft.value = value))
watch(() => props.item.orderedAt, value => (orderedDraft.value = value))
watch(() => props.item.supplier, value => (supplierDraft.value = value))

const required = computed(() => store.requiredQty(props.item))
const order = computed(() => store.orderQty(props.item))
const short = computed(() => store.shortage(props.item))
const jlc = computed(() => store.jlcFor(props.item))
const jlcPriceVnd = computed(() => (jlc.value ? jlc.value.price * store.fxRate : 0))
const jlcDeltaPct = computed(() => {
  const price = props.item.product?.price ?? 0

  if (!jlc.value || !price)
    return 0

  return Math.round(((jlcPriceVnd.value - price) / price) * 100)
})
const alternates = computed(() => {
  const id = props.item.product?.id

  return id ? store.alternates[id] ?? [] : []
})
const alternatesLoading = computed(() => {
  const id = props.item.product?.id

  return id ? Boolean(store.alternatesLoading[id]) : false
})

const statusIcon = computed(() => {
  switch (props.item.status) {
    case 'ok': return 'ri-checkbox-circle-fill'
    case 'review': return 'ri-error-warning-line'
    case 'missing': return 'ri-close-circle-line'
    default: return 'ri-checkbox-blank-circle-line'
  }
})

const statusColor = computed(() => {
  switch (props.item.status) {
    case 'ok': return 'success'
    case 'review': return 'warning'
    case 'missing': return 'error'
    default: return 'medium-emphasis'
  }
})

const purchaseColor = computed(() => {
  switch (props.item.purchase) {
    case 'ordered': return 'info'
    case 'received': return 'success'
    default: return undefined
  }
})

const footprintShort = computed(() => {
  const value = props.item.footprint ?? ''

  if (!value)
    return ''

  const tail = value.includes(':') ? value.split(':').pop() ?? value : value

  return tail.length > 34 ? `${tail.slice(0, 32)}…` : tail
})

const subtitle = computed(() => {
  const product = props.item.product

  if (!product)
    return ''

  return [product.supplier?.name, product.manufacturer_code].filter(Boolean).join(' · ')
})

watch(panel, open => {
  if (open && props.item.product)
    void store.loadAlternates(props.item.product.id)
})

function commitQty() {
  const value = Number(qtyDraft.value)

  if (Number.isFinite(value) && value > 0 && value !== props.item.qty)
    emit('update-qty', value)
  else
    qtyDraft.value = String(props.item.qty)
}

function commitInput() {
  if (inputDraft.value !== props.item.input)
    emit('update-input', inputDraft.value)
}

function onInputEnter() {
  commitInput()
  emit('analyze-row')
}

function onPurchase(value: unknown) {
  if (value === 'need' || value === 'ordered' || value === 'received')
    emit('purchase', value)
}

function cyclePurchase() {
  emit('purchase', props.item.purchase === 'need' ? 'ordered' : props.item.purchase === 'ordered' ? 'received' : 'need')
}

function togglePanel() {
  panel.value = !panel.value

  if (panel.value) {
    query.value = props.item.input
    results.value = []
    searched.value = false
  }
}

async function runSearch() {
  const text = query.value.trim()

  if (!text)
    return

  searching.value = true
  searched.value = false

  try {
    const body = await apiGet<SearchResponse>('/search', { q: text, limit: 6 })

    results.value = body.data?.products?.data ?? []
  }
  catch {
    results.value = []
  }
  finally {
    searching.value = false
    searched.value = true
  }
}

function choose(product: Product) {
  emit('pick', product)
  panel.value = false
}
</script>

<template>
  <div>
    <div
      class="bom-row"
      :class="`is-${item.status}`"
    >
      <div class="bom-cell bom-status">
        <VTooltip :text="item.status === 'ok' ? 'Bỏ đánh dấu OK' : 'Đánh dấu đã OK'">
          <template #activator="{ props: tip }">
            <VBtn
              v-bind="tip"
              icon
              size="small"
              variant="text"
              @click="emit('toggle-ok')"
            >
              <VIcon
                :icon="statusIcon"
                :color="statusColor"
                size="22"
              />
            </VBtn>
          </template>
        </VTooltip>
      </div>

      <div class="bom-cell bom-ref mono">
        {{ item.ref }}
      </div>

      <div class="bom-cell">
        <VTextField
          v-model="qtyDraft"
          type="number"
          density="compact"
          variant="outlined"
          hide-details
          style="inline-size: 76px"
          @blur="commitQty"
          @keydown.enter="commitQty"
        />
      </div>

      <div class="bom-cell bom-need">
        <div :class="short ? 'text-error' : 'text-success'">
          {{ required }}
        </div>
        <div class="text-caption text-medium-emphasis">
          đặt {{ order }}
        </div>
      </div>

      <div class="bom-cell bom-input">
        <VTextField
          v-model="inputDraft"
          density="compact"
          variant="outlined"
          hide-details
          class="mono"
          @blur="commitInput"
          @keydown.enter="onInputEnter"
        />
        <div
          class="text-caption text-medium-emphasis text-truncate"
          :title="item.footprint"
        >
          {{ item.note || ' ' }}<template v-if="footprintShort"> · FP: {{ footprintShort }}</template>
        </div>
      </div>

      <div class="bom-cell bom-product">
        <template v-if="item.product">
          <img
            class="bom-thumb"
            :src="imageUrl(item.product.image_id, 'small')"
            alt=""
            loading="lazy"
          >
          <div class="bom-product-text">
            <a
              class="bom-name"
              :title="item.product.name"
              @click="emit('open', item.product.slug)"
            >{{ item.product.name }}</a>
            <div class="text-caption text-medium-emphasis text-truncate">
              {{ subtitle }} · {{ formatVnd(item.product.price) }} ·
              <span :class="item.product.inventory > 0 ? 'text-success' : 'text-error'">
                {{ item.product.inventory > 0 ? `còn ${item.product.inventory}` : 'hết hàng' }}
              </span>
            </div>
            <div
              v-if="jlc"
              class="jlc-line"
            >
              <span class="mono">JLC {{ jlc.lcsc }}</span> · ${{ jlc.price.toFixed(4) }} ·
              còn {{ jlc.stock.toLocaleString('vi-VN') }}
              <span :class="jlcDeltaPct < 0 ? 'text-success' : 'text-error'">
                ({{ jlcDeltaPct < 0 ? 'rẻ hơn' : 'đắt hơn' }} {{ Math.abs(jlcDeltaPct) }}%)
              </span>
            </div>
          </div>
        </template>
        <span
          v-else
          class="text-caption text-medium-emphasis"
        >Chưa có sản phẩm</span>
      </div>

      <div class="bom-cell bom-purchase">
        <VChip
          size="small"
          :color="purchaseColor"
          variant="tonal"
          @click="cyclePurchase"
        >
          {{ PURCHASE_LABEL[item.purchase] }}
        </VChip>
      </div>

      <div class="bom-cell bom-actions">
        <VBtn
          v-if="item.footprint"
          size="small"
          icon
          variant="text"
          title="Xem 3D footprint"
          @mouseenter="loadFootprintViewer()"
          @focus="loadFootprintViewer()"
          @click="fp3dOpen = true"
        >
          <VIcon
            icon="ri-box-3-line"
            size="18"
          />
        </VBtn>

        <VBtn
          size="small"
          variant="text"
          @click="togglePanel"
        >
          {{ item.product ? 'Đổi' : 'Tìm' }}
        </VBtn>

        <VBtn
          v-if="item.product"
          size="small"
          icon
          variant="text"
          :href="siteUrl(item.product.slug)"
          target="_blank"
          rel="noreferrer"
        >
          <VIcon
            icon="ri-external-link-line"
            size="18"
          />
        </VBtn>
      </div>
    </div>

    <div
      v-if="panel"
      class="bom-panel"
    >
      <div class="d-flex flex-wrap align-center ga-3 mb-3">
        <span class="text-caption text-medium-emphasis">Mua hàng</span>
        <VBtnGroup
          divided
          variant="outlined"
          size="small"
        >
          <VBtn
            v-for="option in PURCHASE_OPTIONS"
            :key="option.value"
            :variant="item.purchase === option.value ? 'flat' : 'outlined'"
            :color="item.purchase === option.value ? 'primary' : undefined"
            @click="onPurchase(option.value)"
          >
            {{ option.title }}
          </VBtn>
        </VBtnGroup>

        <VTextField
          v-model="orderedDraft"
          type="date"
          label="Ngày đặt"
          density="compact"
          variant="outlined"
          hide-details
          style="inline-size: 160px"
          @blur="emit('ordered-at', orderedDraft)"
        />

        <VTextField
          v-model="supplierDraft"
          label="Nơi mua"
          density="compact"
          variant="outlined"
          hide-details
          style="inline-size: 180px"
          @blur="emit('supplier', supplierDraft)"
        />

        <VBtn
          variant="tonal"
          prepend-icon="ri-magic-line"
          :disabled="!item.product"
          @click="emit('optimize')"
        >
          Tối ưu giá
        </VBtn>
      </div>

      <div class="d-flex flex-wrap align-center ga-2 mb-3">
        <span class="text-caption text-medium-emphasis">Thay thế:</span>
        <VProgressCircular
          v-if="alternatesLoading"
          indeterminate
          size="16"
        />
        <VChip
          v-for="alt in alternates"
          :key="alt.id"
          size="small"
          variant="tonal"
          @click="choose(alt)"
        >
          {{ alt.manufacturer_code || alt.name }} · {{ formatVnd(alt.price) }} ·
          {{ alt.inventory > 0 ? `còn ${alt.inventory}` : 'hết hàng' }}
        </VChip>
        <span
          v-if="!alternatesLoading && !alternates.length"
          class="text-caption text-medium-emphasis"
        >Không có gợi ý.</span>
      </div>

      <div class="d-flex flex-wrap ga-2">
        <VTextField
          v-model="query"
          density="compact"
          variant="outlined"
          hide-details
          placeholder="Tìm sản phẩm cho dòng này…"
          prepend-inner-icon="ri-search-line"
          style="min-inline-size: 240px; flex: 1"
          @keydown.enter="runSearch"
        />
        <VBtn
          color="primary"
          :loading="searching"
          @click="runSearch"
        >
          Tìm
        </VBtn>
        <VBtn
          variant="text"
          @click="panel = false"
        >
          Đóng
        </VBtn>
      </div>

      <div
        v-if="results.length"
        class="bom-results"
      >
        <div
          v-for="product in results"
          :key="product.id"
          class="bom-result"
          @click="choose(product)"
        >
          <img
            :src="imageUrl(product.image_id, 'small')"
            alt=""
            loading="lazy"
          >
          <div class="bom-product-text">
            <div class="text-body-2 text-truncate">
              {{ product.name }}
            </div>
            <div class="text-caption text-medium-emphasis text-truncate">
              {{ [product.supplier?.name, product.manufacturer_code].filter(Boolean).join(' · ') }}
              · {{ formatVnd(product.price) }}
              · {{ product.inventory > 0 ? `còn ${product.inventory}` : 'hết hàng' }}
            </div>
          </div>
          <VBtn
            size="small"
            variant="text"
            color="primary"
          >
            Chọn
          </VBtn>
        </div>
      </div>

      <div
        v-else-if="searched && !searching"
        class="text-caption text-medium-emphasis pa-2"
      >
        Không có kết quả.
      </div>

      <div
        v-if="item.candidates.length"
        class="mt-2"
      >
        <div class="text-caption text-medium-emphasis mb-1">
          Gợi ý từ phân tích:
        </div>
        <div class="d-flex flex-wrap ga-1">
          <VChip
            v-for="candidate in item.candidates"
            :key="candidate.id"
            size="small"
            variant="tonal"
            @click="choose(candidate)"
          >
            {{ candidate.manufacturer_code || candidate.name }}
          </VChip>
        </div>
      </div>
    </div>

    <VDialog
      v-model="fp3dOpen"
      max-width="1040"
      scrollable
    >
      <VCard class="pa-3">
        <div class="d-flex align-center mb-2">
          <span class="text-subtitle-1 font-weight-bold">3D footprint</span>
          <VSpacer />
          <IconBtn @click="fp3dOpen = false">
            <VIcon icon="ri-close-line" />
          </IconBtn>
        </div>
        <FootprintViewer
          v-if="fp3dOpen"
          :footprint="item.footprint"
          :project="store.project"
        />
      </VCard>
    </VDialog>
  </div>
</template>

<style scoped>
.bom-row {
  display: grid;
  grid-template-columns: 38px 58px 88px 86px minmax(0, 0.9fr) minmax(0, 1.5fr) 104px 124px;
  gap: 10px;
  align-items: center;
  padding: 8px 12px;
  border-block-start: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
}

.bom-row.is-ok {
  background: rgba(var(--v-theme-success), 0.045);
}

.bom-row.is-review {
  background: rgba(var(--v-theme-warning), 0.07);
}

.bom-row.is-missing {
  background: rgba(var(--v-theme-error), 0.055);
}

.bom-cell {
  min-inline-size: 0;
}

.bom-status {
  display: flex;
  justify-content: center;
}

.bom-ref {
  font-size: 0.78rem;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.bom-need {
  font-weight: 700;
  font-size: 0.86rem;
  line-height: 1.15;
}

.bom-input {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.bom-product {
  display: flex;
  align-items: center;
  gap: 10px;
  min-inline-size: 0;
}

.bom-thumb {
  inline-size: 42px;
  block-size: 42px;
  object-fit: contain;
  border-radius: 8px;
  background: rgba(var(--v-theme-on-surface), 0.05);
  flex: 0 0 auto;
}

.bom-product-text {
  min-inline-size: 0;
  flex: 1;
}

.bom-name {
  display: block;
  font-size: 0.82rem;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: pointer;
  color: rgb(var(--v-theme-primary));
}

.jlc-line {
  font-size: 0.72rem;
  opacity: 0.8;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.bom-purchase {
  display: flex;
  justify-content: flex-start;
}

.bom-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 4px;
}

.bom-panel {
  padding: 12px 16px 16px 104px;
  background: rgba(var(--v-theme-on-surface), 0.03);
  border-block-start: 1px dashed rgba(var(--v-border-color), var(--v-border-opacity));
}

.bom-results {
  margin-block-start: 10px;
  border: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
  border-radius: 8px;
  overflow: hidden;
}

.bom-result {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 12px;
  cursor: pointer;
}

.bom-result + .bom-result {
  border-block-start: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
}

.bom-result:hover {
  background: rgba(var(--v-theme-primary), 0.06);
}

.bom-result img {
  inline-size: 40px;
  block-size: 40px;
  object-fit: contain;
  border-radius: 6px;
  background: rgba(var(--v-theme-on-surface), 0.05);
  flex: 0 0 auto;
}

@media (max-width: 760px) {
  .bom-panel {
    padding-inline-start: 16px;
  }
}
</style>
