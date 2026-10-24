<script setup lang="ts">
import { apiGet, formatVnd, imageUrl, siteUrl } from '@/api'
import type { ProductDetail, ProductDetailResponse } from '@/types'

const props = defineProps<{ modelValue: string | null }>()

const emit = defineEmits<{
  'update:modelValue': [value: string | null]
  copy: [label: string, text: string]
}>()

const detail = ref<ProductDetail | null>(null)
const loading = ref(false)
const error = ref<string | null>(null)
const retry = ref(0)
const activeImage = ref<number | null>(null)
let abort: AbortController | null = null

const open = computed({
  get: () => Boolean(props.modelValue),
  set: (value: boolean) => {
    if (!value)
      emit('update:modelValue', null)
  },
})

const gallery = computed(() => {
  if (!detail.value)
    return [] as number[]

  const ids = [detail.value.image_id, ...(detail.value.images ?? []).map(image => image.image_id)]

  return Array.from(new Set(ids.filter((id): id is number => typeof id === 'number')))
})

watch(
  () => [props.modelValue, retry.value] as const,
  async ([slug]) => {
    if (!slug)
      return

    abort?.abort()
    abort = new AbortController()
    loading.value = true
    error.value = null
    detail.value = null

    try {
      const body = await apiGet<ProductDetailResponse>(
        `/product/${encodeURIComponent(slug)}`,
        undefined,
        abort.signal,
      )
      const product = body.data?.product ?? null

      if (!product) {
        error.value = 'Không tìm thấy sản phẩm này.'

        return
      }

      detail.value = product
      activeImage.value = product.image_id ?? product.images?.[0]?.image_id ?? null
    }
    catch (err) {
      if ((err as Error).name !== 'AbortError')
        error.value = (err as Error).message
    }
    finally {
      if (!abort.signal.aborted)
        loading.value = false
    }
  },
  { immediate: true },
)

onBeforeUnmount(() => abort?.abort())

function copyCode() {
  if (!detail.value)
    return

  const text = detail.value.manufacturer_code
    ? `${detail.value.name} (${detail.value.manufacturer_code})`
    : detail.value.name

  emit('copy', 'Đã copy tên + mã', text)
}
</script>

<template>
  <VDialog
    v-model="open"
    max-width="1000"
  >
    <VCard class="detail-card">
      <!-- 👉 Header -->
      <div class="detail-head d-flex align-center px-5 py-3">
        <div class="text-subtitle-1 font-weight-bold">
          Chi tiết sản phẩm
        </div>
        <VSpacer />
        <IconBtn
          size="small"
          @click="open = false"
        >
          <VIcon icon="ri-close-line" />
        </IconBtn>
      </div>

      <div
        v-if="loading"
        class="pa-6"
      >
        <VSkeletonLoader
          type="image"
          height="240"
        />
        <VSkeletonLoader
          type="article"
          class="mt-4"
        />
      </div>

      <VAlert
        v-else-if="error"
        type="error"
        variant="tonal"
        class="ma-6"
      >
        {{ error }}
        <template #append>
          <VBtn
            size="small"
            variant="text"
            color="inherit"
            @click="retry++"
          >
            Thử lại
          </VBtn>
        </template>
      </VAlert>

      <!-- 👉 Hai cột: trái = ảnh + giá, phải = thông tin + thông số -->
      <div
        v-else-if="detail"
        class="detail-body"
      >
        <div class="detail-col">
          <div class="detail-image-block rounded pa-3">
            <img
              v-if="activeImage"
              :src="imageUrl(activeImage, 'large')"
              :alt="detail.name"
              class="detail-image"
            >
            <VIcon
              v-else
              icon="ri-archive-line"
              size="64"
              class="text-medium-emphasis"
            />
            <div
              v-if="gallery.length > 1"
              class="d-flex flex-wrap ga-2 mt-3"
            >
              <VSheet
                v-for="image in gallery"
                :key="image"
                rounded="lg"
                :border="image === activeImage ? 'primary thin' : 'thin'"
                class="pa-1"
                style="cursor: pointer"
                @click="activeImage = image"
              >
                <img
                  :src="imageUrl(image, 'small')"
                  alt=""
                  style="inline-size: 52px; block-size: 52px; object-fit: contain"
                >
              </VSheet>
            </div>
          </div>

          <div class="mt-4">
            <h2 class="text-h6 font-weight-bold">
              {{ detail.name }}
            </h2>
            <p
              v-if="detail.desc_small"
              class="text-body-2 text-medium-emphasis mt-1 mb-0"
            >
              {{ detail.desc_small }}
            </p>
            <div class="d-flex flex-wrap ga-2 mt-3">
              <VChip
                v-if="detail.supplier?.name"
                size="small"
                variant="tonal"
              >
                {{ detail.supplier.name }}
              </VChip>
              <VChip
                v-if="detail.manufacturer_code"
                size="small"
                variant="outlined"
                class="mono"
                @click="copyCode"
              >
                {{ detail.manufacturer_code }}
                <VIcon
                  end
                  icon="ri-file-copy-line"
                  size="14"
                  class="ms-2"
                />
              </VChip>
              <VChip
                v-if="detail.rohs"
                size="small"
                color="success"
                variant="tonal"
              >
                RoHS
              </VChip>
            </div>
          </div>

          <VSheet
            rounded="lg"
            class="pa-4 detail-block mt-4"
          >
            <div class="d-flex align-baseline ga-2">
              <span class="text-h5 font-weight-bold text-primary">{{ formatVnd(detail.price) }}</span>
              <span class="text-body-2 text-medium-emphasis">/ {{ detail.unit?.trim() || 'sản phẩm' }}</span>
            </div>
            <div
              v-if="detail.prices && detail.prices.length > 1"
              class="mt-3"
            >
              <div
                v-for="tier in detail.prices"
                :key="tier.quantity"
                class="d-flex justify-space-between text-body-2 py-1"
              >
                <span class="text-medium-emphasis">Từ {{ tier.quantity }}</span>
                <span class="font-weight-medium">{{ formatVnd(tier.price) }}</span>
              </div>
            </div>
          </VSheet>
        </div>

        <div class="detail-col">
          <VSheet
            rounded="lg"
            class="pa-4 detail-block"
          >
            <div class="text-overline text-medium-emphasis mb-2">
              Thông tin
            </div>
            <div class="d-flex justify-space-between text-body-2 py-1">
              <span class="text-medium-emphasis">Tồn kho</span>
              <span
                class="font-weight-bold"
                :class="detail.inventory > 0 ? 'text-success' : 'text-medium-emphasis'"
              >
                {{ detail.inventory > 0 ? `${detail.inventory} ${detail.unit?.trim() ?? ''}` : 'Hết hàng' }}
              </span>
            </div>
            <div class="d-flex justify-space-between text-body-2 py-1">
              <span class="text-medium-emphasis">Số lượng tối thiểu</span>
              <span>{{ detail.minimum_order_quantity ?? 1 }}</span>
            </div>
            <div class="d-flex justify-space-between text-body-2 py-1">
              <span class="text-medium-emphasis">Bội số</span>
              <span>{{ detail.multiples ?? 1 }}</span>
            </div>
            <div class="d-flex justify-space-between text-body-2 py-1">
              <span class="text-medium-emphasis">Mã thegioiic</span>
              <span>#{{ detail.id }}</span>
            </div>
          </VSheet>

          <VSheet
            v-if="detail.specs && detail.specs.length"
            rounded="lg"
            class="pa-4 detail-block mt-4"
          >
            <div class="text-overline text-medium-emphasis mb-2">
              Thông số
            </div>
            <div
              v-for="(spec, index) in detail.specs"
              :key="`${spec.name}-${index}`"
              class="d-flex py-2 spec-row"
            >
              <div
                class="text-body-2 text-medium-emphasis"
                style="inline-size: 42%; padding-inline-end: 12px"
              >
                {{ spec.name }}
              </div>
              <div class="text-body-2 font-weight-medium">
                {{ spec.value }}
              </div>
            </div>
          </VSheet>
          <div
            v-else
            class="text-caption text-medium-emphasis mt-3"
          >
            Chưa có bảng thông số cho sản phẩm này — xem thêm trên trang shop.
          </div>
        </div>
      </div>

      <!-- 👉 Hành động -->
      <div class="detail-actions px-5 py-3 d-flex ga-2">
        <VBtn
          v-if="detail"
          color="primary"
          class="flex-grow-1"
          append-icon="ri-external-link-line"
          :href="siteUrl(detail.slug)"
          target="_blank"
          rel="noreferrer"
        >
          Mở trên thegioiic
        </VBtn>
        <VBtn
          v-if="detail"
          variant="tonal"
          prepend-icon="ri-file-copy-line"
          @click="copyCode"
        >
          Copy
        </VBtn>
      </div>
    </VCard>
  </VDialog>
</template>

<style scoped>
.detail-card {
  display: flex;
  flex-direction: column;
  max-block-size: 90vh;
  overflow: hidden;
}

.detail-head {
  flex: 0 0 auto;
  border-block-end: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
}

.detail-body {
  display: grid;
  grid-template-columns: minmax(0, 0.95fr) minmax(0, 1.05fr);
  gap: 24px;
  padding: 20px;
  overflow: hidden;
  flex: 1 1 auto;
  min-block-size: 0;
}

.detail-col {
  min-inline-size: 0;
  overflow-y: auto;
  padding-inline-end: 4px;
}

.detail-actions {
  flex: 0 0 auto;
  border-block-start: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
}

.detail-block {
  border: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
}

.detail-image-block {
  background: rgba(var(--v-theme-on-surface), 0.04);
  display: grid;
  place-items: center;
}

.detail-image {
  inline-size: 100%;
  block-size: 260px;
  object-fit: contain;
}

.spec-row + .spec-row {
  border-block-start: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
}

@media (max-width: 760px) {
  .detail-body {
    grid-template-columns: 1fr;
    overflow-y: auto;
  }

  .detail-col {
    overflow: visible;
  }
}
</style>
