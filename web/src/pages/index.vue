<script setup lang="ts">
import { useAccessoryStore } from '@/stores/accessory'
import CategoryPicker from '@/views/accessory/CategoryPicker.vue'
import FilterPanel from '@/views/accessory/FilterPanel.vue'
import HeroPanel from '@/views/accessory/HeroPanel.vue'
import PagerBar from '@/views/accessory/PagerBar.vue'
import ProductCard from '@/views/accessory/ProductCard.vue'
import ProductDetail from '@/views/accessory/ProductDetail.vue'

const store = useAccessoryStore()

onMounted(() => void store.ensureCatalog())

const sortOptions = [
  { title: 'Mặc định', value: 0 },
  { title: 'Tên A→Z', value: 1 },
  { title: 'Giá tăng dần', value: 2 },
  { title: 'Tồn kho nhiều', value: 3 },
  { title: 'Mới nhất', value: 4 },
]

const categoryDialog = ref(false)
const pickerKey = ref(0)

function openPicker() {
  pickerKey.value += 1
  categoryDialog.value = true
}

function pickCategory(id: number) {
  store.selectCategory(id)
  categoryDialog.value = false
}

const sortModel = computed({
  get: () => store.ui.sort,
  set: (value: number) => store.setSort(value),
})

const detailModel = computed({
  get: () => store.ui.item,
  set: (value: string | null) => {
    if (value === null)
      store.closeDetail()
  },
})

const snackbar = ref(false)
const snackText = ref('')

function handleCopy(label: string, text: string) {
  navigator.clipboard
    .writeText(text)
    .then(() => {
      snackText.value = label
      snackbar.value = true
    })
    .catch(() => {
      snackText.value = 'Không copy được vào clipboard'
      snackbar.value = true
    })
}
</script>

<template>
  <div class="accessory-page">
    <HeroPanel
      v-if="store.view === 'home'"
      :roots="store.tree"
      :category-count="store.categories.length"
      :supplier-count="store.supplierCount"
      @select="store.selectCategory"
      @search="store.setSearch"
    />

    <VRow v-else>
      <!-- 👉 Bộ lọc: chỉ hiện khi duyệt theo danh mục -->
      <VCol
        v-if="store.view === 'category'"
        cols="12"
        md="4"
        lg="3"
      >
        <VCard
          class="pa-4"
          elevation="0"
        >
          <FilterPanel
            :meta="store.filterMeta"
            :loading="store.filtersLoading"
            :available-prids="store.availablePrids"
            :available-suppliers="store.availableSuppliers"
            :supplier="store.ui.supplier"
            :prids="store.ui.prid"
            :stock="store.ui.stock"
            @supplier="store.setSupplier"
            @prid="store.togglePrid"
            @stock="store.setStock"
            @clear="store.clearFilters"
          />
        </VCard>
      </VCol>

      <!-- 👉 Kết quả (full width khi đang tìm kiếm) -->
      <VCol
        cols="12"
        :md="store.view === 'category' ? 8 : 12"
        :lg="store.view === 'category' ? 9 : 12"
      >
        <div class="d-flex flex-wrap align-center ga-3 mb-4 page-sticky page-sticky--bordered">
          <div style="min-inline-size: 0">
            <h2 class="text-h6 font-weight-bold text-truncate">
              {{ store.headerTitle }}
            </h2>
            <div class="text-body-2 text-medium-emphasis">
              <template v-if="store.loading && !store.result">
                Đang tải…
              </template>
              <template v-else>
                {{ store.total.toLocaleString('vi-VN') }} sản phẩm
              </template>
              <template v-if="store.view === 'category' && store.activeFilters > 0">
                · {{ store.activeFilters }} bộ lọc
              </template>
            </div>
          </div>

          <VSpacer />

          <!-- 👉 Chỉ một nút để mở dialog chọn danh mục -->
          <VBtn
            variant="tonal"
            class="text-none"
            prepend-icon="ri-folder-3-line"
            @click="openPicker"
          >
            <span
              class="text-truncate"
              style="max-inline-size: 200px"
            >{{ store.activeCategory?.name ?? 'Chọn danh mục' }}</span>
            <VIcon
              end
              icon="ri-arrow-down-s-line"
              size="18"
              class="ms-1"
            />
          </VBtn>

          <VSelect
            v-model="sortModel"
            :items="sortOptions"
            density="compact"
            variant="outlined"
            hide-details
            prepend-inner-icon="ri-arrow-up-down-line"
            style="inline-size: 200px; max-inline-size: 200px"
          />
        </div>

        <VProgressLinear
          v-if="store.loading && store.result"
          indeterminate
          color="primary"
          rounded
          class="mb-4"
        />

        <VAlert
          v-if="store.error"
          type="error"
          variant="tonal"
          class="mb-4"
        >
          {{ store.error }}
          <template #append>
            <VBtn
              size="small"
              variant="text"
              color="inherit"
              @click="store.reload()"
            >
              Thử lại
            </VBtn>
          </template>
        </VAlert>

        <div
          v-else-if="store.loading && !store.result"
          class="product-grid"
        >
          <VSkeletonLoader
            v-for="index in 8"
            :key="index"
            type="card"
          />
        </div>

        <template v-else-if="store.total === 0">
          <div v-if="store.activeNode && store.activeNode.children.length > 0">
            <div class="text-subtitle-2 font-weight-bold mb-3">
              Danh mục này gồm {{ store.activeNode.children.length }} danh mục con
            </div>
            <div class="subcategory-grid">
              <VCard
                v-for="child in store.activeNode.children"
                :key="child.id"
                class="subcategory-tile pa-4"
                elevation="0"
                @click="store.selectCategory(child.id)"
              >
                <div class="text-body-2 font-weight-medium">
                  {{ child.name }}
                </div>
                <div class="text-caption text-medium-emphasis">
                  {{ child.count > 0 ? `${child.count} sản phẩm` : `${child.children.length} danh mục con` }}
                </div>
              </VCard>
            </div>
          </div>

          <VCard
            v-else
            class="pa-10 text-center"
            elevation="0"
          >
            <VIcon
              icon="ri-search-eye-line"
              size="52"
              class="text-medium-emphasis"
            />
            <div class="text-h6 font-weight-bold mt-3">
              Không tìm thấy sản phẩm
            </div>
            <div class="text-body-2 text-medium-emphasis mt-1">
              Thử từ khoá ngắn hơn, bỏ bớt bộ lọc, hoặc chọn danh mục khác.
            </div>
            <VBtn
              v-if="store.activeFilters > 0"
              variant="tonal"
              class="mt-4"
              @click="store.clearFilters"
            >
              Xoá bộ lọc
            </VBtn>
          </VCard>
        </template>

        <template v-else>
          <div class="product-grid">
            <ProductCard
              v-for="product in store.products"
              :key="product.id"
              :product="product"
              @open="store.openDetail"
              @copy="handleCopy"
            />
          </div>

          <PagerBar
            :page="store.currentPage"
            :total-page="store.totalPage"
            :total="store.total"
            :limit="store.ui.limit"
            @update:page="store.setPage"
            @update:limit="store.setLimit"
          />
        </template>

        <div
          v-if="store.view === 'search' && store.result?.categories?.length"
          class="d-flex flex-wrap align-center ga-2 mt-5"
        >
          <span class="text-caption text-medium-emphasis">Danh mục liên quan:</span>
          <VChip
            v-for="item in store.result.categories.slice(0, 6)"
            :key="item.id"
            size="small"
            variant="tonal"
            @click="store.selectCategory(item.id)"
          >
            {{ item.name }}
          </VChip>
        </div>

        <div class="text-caption text-medium-emphasis mt-8">
          re-search-accessory · dữ liệu công khai của thegioiic.com (cache 2 phút) · không phải ứng
          dụng chính thức
        </div>
      </VCol>
    </VRow>

    <VDialog
      v-model="categoryDialog"
      max-width="620"
      scrollable
    >
      <VCard class="pa-2">
        <div class="d-flex align-center pa-2">
          <span class="text-h6">Chọn danh mục</span>
          <VSpacer />
          <IconBtn @click="categoryDialog = false">
            <VIcon icon="ri-close-line" />
          </IconBtn>
        </div>

        <CategoryPicker
          :key="pickerKey"
          :tree="store.tree"
          :selected="store.ui.cat"
          :loading="store.catalogLoading"
          @select="pickCategory"
          @close="categoryDialog = false"
        />
      </VCard>
    </VDialog>

    <ProductDetail
      v-model="detailModel"
      @copy="handleCopy"
    />

    <VSnackbar
      v-model="snackbar"
      :timeout="2200"
    >
      {{ snackText }}
    </VSnackbar>
  </div>
</template>

<style scoped>
.subcategory-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 14px;
}

.subcategory-tile {
  cursor: pointer;
  border: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
  transition:
    border-color 0.15s ease,
    box-shadow 0.15s ease;
}

.subcategory-tile:hover {
  border-color: rgb(var(--v-theme-primary));
  box-shadow: 0 14px 30px -22px rgba(46, 38, 61, 0.4);
}
</style>
