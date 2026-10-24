<script setup lang="ts">
import type { FilterMeta, Supplier } from '@/types'

const props = defineProps<{
  meta: FilterMeta | null
  loading: boolean
  availablePrids: Set<number> | null
  availableSuppliers: Set<number> | null
  supplier: number | null
  prids: number[]
  stock: boolean
}>()

const emit = defineEmits<{
  supplier: [id: number | null]
  prid: [id: number]
  stock: [value: boolean]
  clear: []
}>()

const brandQuery = ref('')

const brands = computed<Supplier[]>(() => {
  const list = props.meta?.suppliers ?? []
  const query = brandQuery.value.trim().toLowerCase()

  return query ? list.filter(brand => brand.name.toLowerCase().includes(query)) : list
})

const activeCount = computed(() => (props.supplier ? 1 : 0) + props.prids.length + (props.stock ? 1 : 0))

function toggleSupplier(id: number) {
  emit('supplier', props.supplier === id ? null : id)
}

function dimmedSupplier(id: number) {
  return props.supplier !== id && props.availableSuppliers !== null && !props.availableSuppliers.has(id)
}

function dimmedPrid(id: number) {
  return !props.prids.includes(id) && props.availablePrids !== null && !props.availablePrids.has(id)
}

function selectedInGroup(ids: number[]): number {
  return ids.filter(id => props.prids.includes(id)).length
}
</script>

<template>
  <div>
    <div class="d-flex align-center mb-2">
      <span class="text-overline text-medium-emphasis flex-grow-1">Bộ lọc</span>
      <VBtn
        v-if="activeCount"
        size="x-small"
        variant="text"
        color="primary"
        prepend-icon="ri-filter-off-line"
        @click="emit('clear')"
      >
        Xoá ({{ activeCount }})
      </VBtn>
    </div>

    <VProgressLinear
      v-if="loading"
      indeterminate
      color="primary"
      rounded
      class="mb-2"
    />

    <template v-if="meta">
      <div class="text-caption text-medium-emphasis mb-2">
        Danh mục: {{ meta.total.toLocaleString('vi-VN') }} sản phẩm · {{ meta.totalInStock }} còn hàng
      </div>

      <div class="d-flex align-center justify-space-between mb-1">
        <span class="text-body-2">Chỉ hàng còn</span>
        <VSwitch
          :model-value="stock"
          density="compact"
          hide-details
          @update:model-value="emit('stock', Boolean($event))"
        />
      </div>

      <VExpansionPanels
        variant="accordion"
        multiple
        flat
        class="accessory-filters"
      >
        <VExpansionPanel value="brands">
          <VExpansionPanelTitle class="text-body-2 font-weight-medium">
            <span class="flex-grow-1">Thương hiệu</span>
            <VChip
              v-if="supplier"
              size="x-small"
              color="primary"
              class="me-2"
            >
              1
            </VChip>
          </VExpansionPanelTitle>
          <VExpansionPanelText>
            <VTextField
              v-model="brandQuery"
              placeholder="Tìm thương hiệu"
              density="compact"
              variant="outlined"
              prepend-inner-icon="ri-search-line"
              hide-details
              class="mb-2"
            />
            <div class="filter-scroll">
              <div
                v-for="brand in brands"
                :key="brand.id"
                class="filter-row d-flex align-center"
                :class="{ dimmed: dimmedSupplier(brand.id) }"
              >
                <VCheckbox
                  :model-value="supplier === brand.id"
                  density="compact"
                  hide-details
                  class="flex-grow-0"
                  @update:model-value="toggleSupplier(brand.id)"
                />
                <span
                  class="text-body-2 flex-grow-1 text-truncate filter-label"
                  @click="toggleSupplier(brand.id)"
                >{{ brand.name }}</span>
                <span class="text-caption text-medium-emphasis">{{ brand.products_count }}</span>
              </div>
              <div
                v-if="brands.length === 0"
                class="text-caption text-medium-emphasis pa-2"
              >
                Không có thương hiệu khớp.
              </div>
            </div>
          </VExpansionPanelText>
        </VExpansionPanel>

        <VExpansionPanel
          v-for="group in meta.category_properties"
          :key="group.id"
          :value="group.id"
        >
          <VExpansionPanelTitle class="text-body-2 font-weight-medium">
            <span class="flex-grow-1">{{ group.name }}</span>
            <VChip
              v-if="selectedInGroup(group.childrens.map(value => value.id))"
              size="x-small"
              color="primary"
              class="me-2"
            >
              {{ selectedInGroup(group.childrens.map(value => value.id)) }}
            </VChip>
          </VExpansionPanelTitle>
          <VExpansionPanelText>
            <div class="filter-scroll">
              <div
                v-for="value in group.childrens"
                :key="value.id"
                class="filter-row d-flex align-center"
                :class="{ dimmed: dimmedPrid(value.id) }"
              >
                <VCheckbox
                  :model-value="prids.includes(value.id)"
                  density="compact"
                  hide-details
                  class="flex-grow-0"
                  @update:model-value="emit('prid', value.id)"
                />
                <span
                  class="text-body-2 flex-grow-1 text-truncate filter-label"
                  @click="emit('prid', value.id)"
                >{{ value.name }}</span>
                <span class="text-caption text-medium-emphasis">{{ value.products_count }}</span>
              </div>
            </div>
          </VExpansionPanelText>
        </VExpansionPanel>
      </VExpansionPanels>
    </template>

    <div
      v-else-if="!loading"
      class="text-caption text-medium-emphasis"
    >
      Chọn một danh mục con để lọc theo hãng và thông số.
    </div>
  </div>
</template>

<style scoped>
.filter-scroll {
  max-block-size: 240px;
  overflow-y: auto;
}

.filter-row {
  border-radius: 6px;
  padding-inline-end: 4px;
}

.filter-row:hover {
  background: rgba(var(--v-theme-on-surface), 0.04);
}

.filter-row.dimmed {
  opacity: 0.42;
}

.filter-label {
  cursor: pointer;
  user-select: none;
}

.accessory-filters :deep(.v-expansion-panel-title) {
  min-block-size: 42px;
  padding-inline: 4px;
}

.accessory-filters :deep(.v-expansion-panel-text__wrapper) {
  padding: 8px 4px 12px;
}
</style>
