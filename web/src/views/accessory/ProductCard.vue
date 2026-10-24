<script setup lang="ts">
import { formatVnd, imageUrl, siteUrl } from '@/api'
import type { Product } from '@/types'

const props = defineProps<{ product: Product }>()

const emit = defineEmits<{
  open: [slug: string]
  copy: [label: string, text: string]
}>()

const broken = ref(false)
const inStock = computed(() => props.product.inventory > 0)
const copyText = computed(() =>
  props.product.manufacturer_code
    ? `${props.product.name} (${props.product.manufacturer_code})`
    : props.product.name,
)

function copy() {
  emit('copy', 'Đã copy tên + mã', copyText.value)
}
</script>

<template>
  <VCard
    class="accessory-card h-100 d-flex flex-column"
    elevation="0"
    @click="emit('open', product.slug)"
  >
    <div
      class="product-media"
      :class="{ 'is-out': !inStock }"
    >
      <img
        v-if="product.image_id && !broken"
        :src="imageUrl(product.image_id)"
        :alt="product.name"
        loading="lazy"
        decoding="async"
        @error="broken = true"
      >
      <VIcon
        v-else
        icon="ri-archive-line"
        size="44"
        class="text-medium-emphasis"
      />
      <VChip
        v-if="inStock"
        class="stock-chip"
        size="small"
        color="success"
        label
      >
        Còn {{ product.inventory }}
      </VChip>
      <div
        v-else
        class="out-of-stock"
      >
        Hết hàng
      </div>
    </div>

    <div class="pa-4 d-flex flex-column flex-grow-1 ga-2">
      <div
        class="product-name"
        :title="product.name"
      >
        {{ product.name }}
      </div>

      <div class="d-flex flex-wrap ga-1">
        <VChip
          v-if="product.supplier?.name"
          size="x-small"
          variant="tonal"
        >
          {{ product.supplier.name }}
        </VChip>
        <VChip
          v-if="product.manufacturer_code"
          size="x-small"
          variant="outlined"
          class="mono text-truncate"
          style="max-inline-size: 170px"
        >
          {{ product.manufacturer_code }}
        </VChip>
      </div>

      <VSpacer />

      <div>
        <div class="text-subtitle-1 font-weight-bold text-primary">
          {{ formatVnd(product.price) }}
        </div>
        <div class="text-caption text-medium-emphasis">
          / {{ product.unit?.trim() || 'sản phẩm' }}<template v-if="(product.minimum_order_quantity ?? 1) > 1">
            · MOQ {{ product.minimum_order_quantity }}
          </template>
        </div>
      </div>
    </div>

    <VDivider />
    <div class="d-flex align-center px-2 py-1">
      <VBtn
        size="small"
        variant="text"
        prepend-icon="ri-file-copy-line"
        @click.stop="copy"
      >
        Copy
      </VBtn>
      <VSpacer />
      <VBtn
        size="small"
        variant="text"
        append-icon="ri-external-link-line"
        :href="siteUrl(product.slug)"
        target="_blank"
        rel="noreferrer"
        @click.stop
      >
        Shop
      </VBtn>
    </div>
  </VCard>
</template>

<style scoped>
.accessory-card {
  cursor: pointer;
  overflow: hidden;
}

.product-media {
  position: relative;
  aspect-ratio: 4 / 3;
  background: rgba(var(--v-theme-on-surface), 0.04);
  display: grid;
  place-items: center;
  overflow: hidden;
}

.product-media img {
  inline-size: 100%;
  block-size: 100%;
  object-fit: contain;
  padding: 8px;
  transition: transform 0.3s ease;
}

.accessory-card:hover .product-media img {
  transform: scale(1.04);
}

.product-media.is-out img {
  opacity: 0.4;
  filter: grayscale(0.5);
}

.out-of-stock {
  position: absolute;
  inset-inline: 0;
  inset-block-start: 50%;
  transform: translateY(-50%);
  padding-block: 10px;
  text-align: center;
  font-weight: 800;
  font-size: 1rem;
  letter-spacing: 0.14em;
  text-transform: uppercase;
  background: rgb(var(--v-theme-error));
  color: rgb(var(--v-theme-on-error));
  box-shadow: 0 10px 24px -10px rgba(0, 0, 0, 0.55);
}

.stock-chip {
  position: absolute;
  inset-block-start: 8px;
  inset-inline-end: 8px;
}

.product-name {
  font-size: 0.875rem;
  font-weight: 600;
  line-height: 1.35;
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
  min-block-size: 3.4em;
}
</style>
