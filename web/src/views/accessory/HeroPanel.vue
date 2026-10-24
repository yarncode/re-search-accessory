<script setup lang="ts">
import { imageUrl } from '@/api'
import type { CategoryNode } from '@/types'

const props = defineProps<{
  roots: CategoryNode[]
  categoryCount: number
  supplierCount: number | null
}>()

const emit = defineEmits<{
  select: [id: number]
  search: [term: string]
}>()

const examples = ['ESP32-S3-WROOM-1', 'STM32F103C8T6', 'LM358', 'RS485', 'relay 5V', 'cảm biến nhiệt độ']

const tips = [
  {
    icon: 'ri-barcode-line',
    title: 'Tìm theo mã part',
    body: 'Nhập đúng mã như ESP32-S3-WROOM-1-N4 hay PIC16F877A-I/P — kết quả khớp chính xác đứng đầu.',
  },
  {
    icon: 'ri-equalizer-line',
    title: 'Lọc theo thông số',
    body: 'Trong mỗi danh mục có bộ lọc thương hiệu, điện áp, kiểu chân, số chân, tồn kho…',
  },
  {
    icon: 'ri-file-copy-2-line',
    title: 'Copy nhanh cho BOM',
    body: 'Mỗi thẻ có nút Copy tên + mã hãng, tiện dán vào BOM hoặc đơn mua.',
  },
]

const stats = computed(() => [
  { value: props.categoryCount.toLocaleString('vi-VN'), label: 'danh mục' },
  { value: props.supplierCount ? props.supplierCount.toLocaleString('vi-VN') : '—', label: 'thương hiệu' },
  { value: 'VND', label: 'giá & tồn kho trực tiếp' },
])
</script>

<template>
  <div class="d-flex flex-column ga-6">
    <VCard
      class="hero-panel pa-6 pa-md-10"
      elevation="0"
    >
      <div class="text-overline" style="opacity: 0.72">
        thegioiic.com · tra cứu không chính thức
      </div>
      <h1 class="text-h3 text-md-h2 font-weight-bold mt-3">
        Tìm đúng linh kiện,<br>trong vài giây.
      </h1>
      <p class="mt-4 text-body-1" style="max-inline-size: 660px; opacity: 0.86">
        Gõ tên hoặc mã part ở thanh tìm kiếm, hoặc mở một danh mục để lọc theo thương hiệu, điện áp,
        kiểu chân, tồn kho… Dữ liệu lấy trực tiếp từ thegioiic.com và được cache cục bộ.
      </p>

      <div class="d-flex flex-wrap align-center ga-2 mt-6">
        <VChip
          v-for="example in examples"
          :key="example"
          variant="outlined"
          class="hero-chip"
          @click="emit('search', example)"
        >
          {{ example }}
        </VChip>
      </div>

      <div class="d-flex flex-wrap ga-2 mt-4">
        <VBtn
          to="/bom"
          color="white"
          class="text-primary text-none"
          prepend-icon="ri-file-list-3-line"
        >
          Phân tích BOM từ CSV / Excel
        </VBtn>
      </div>

      <div class="d-flex flex-wrap ga-8 mt-8">
        <div
          v-for="stat in stats"
          :key="stat.label"
        >
          <div class="text-h5 font-weight-bold">
            {{ stat.value }}
          </div>
          <div class="text-caption" style="opacity: 0.72">
            {{ stat.label }}
          </div>
        </div>
      </div>
    </VCard>

    <section>
      <div class="d-flex align-baseline ga-3 mb-3">
        <h2 class="text-h6 font-weight-bold">
          Danh mục
        </h2>
        <span class="text-body-2 text-medium-emphasis">chọn để xem và lọc sản phẩm</span>
      </div>

      <div class="category-grid">
        <VCard
          v-for="node in roots"
          :key="node.id"
          class="category-tile pa-3"
          elevation="0"
          @click="emit('select', node.id)"
        >
          <div class="d-flex align-center ga-3">
            <VAvatar
              size="52"
              rounded="lg"
              color="rgba(var(--v-theme-on-surface), 0.05)"
              class="flex-shrink-0"
            >
              <img
                v-if="node.image_id"
                :src="imageUrl(node.image_id, 'small')"
                alt=""
                style="inline-size: 40px; block-size: 40px; object-fit: contain"
              >
              <VIcon
                v-else
                icon="ri-cpu-line"
              />
            </VAvatar>
            <div style="min-inline-size: 0">
              <div class="text-body-2 font-weight-medium text-truncate">
                {{ node.name }}
              </div>
              <div class="text-caption text-medium-emphasis">
                {{ node.count > 0 ? `${node.count} sản phẩm` : `${node.children.length} danh mục con` }}
              </div>
            </div>
          </div>
        </VCard>
      </div>
    </section>

    <section class="tip-grid">
      <VCard
        v-for="tip in tips"
        :key="tip.title"
        class="tip-card pa-4"
        elevation="0"
      >
        <div class="d-flex ga-3">
          <VIcon
            :icon="tip.icon"
            color="primary"
          />
          <div>
            <div class="text-subtitle-2 font-weight-bold">
              {{ tip.title }}
            </div>
            <p class="text-body-2 text-medium-emphasis mt-1 mb-0">
              {{ tip.body }}
            </p>
          </div>
        </div>
      </VCard>
    </section>
  </div>
</template>

<style scoped>
.hero-panel {
  position: relative;
  overflow: hidden;
  border-radius: 14px;
  color: #fff;
  background:
    radial-gradient(900px 340px at 6% -20%, rgba(140, 87, 255, 0.9) 0%, rgba(140, 87, 255, 0) 62%),
    radial-gradient(680px 320px at 94% 120%, rgba(22, 177, 255, 0.55) 0%, rgba(22, 177, 255, 0) 58%),
    linear-gradient(135deg, #1c1329 0%, #241a3c 46%, #2b1450 100%);
}

.hero-chip {
  color: #fff;
  border-color: rgba(255, 255, 255, 0.38);
  background: rgba(255, 255, 255, 0.06);
}

.hero-chip:hover {
  background: rgba(255, 255, 255, 0.16);
}

.category-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 14px;
}

.category-tile {
  cursor: pointer;
  border: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
  transition:
    transform 0.15s ease,
    box-shadow 0.15s ease,
    border-color 0.15s ease;
}

.category-tile:hover {
  transform: translateY(-2px);
  border-color: rgb(var(--v-theme-primary));
  box-shadow: 0 16px 32px -24px rgba(46, 38, 61, 0.45);
}

.tip-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 14px;
}

.tip-card {
  border: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
}
</style>
