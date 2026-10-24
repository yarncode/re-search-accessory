<script setup lang="ts">
const props = defineProps<{
  page: number
  totalPage: number
  total: number
  limit: number
}>()

const emit = defineEmits<{
  'update:page': [page: number]
  'update:limit': [limit: number]
}>()

const pageSizes = [
  { title: '20 / trang', value: 20 },
  { title: '40 / trang', value: 40 },
  { title: '80 / trang', value: 80 },
  { title: '120 / trang', value: 120 },
]

const pageModel = computed({
  get: () => props.page,
  set: (value: number) => emit('update:page', value),
})

const limitModel = computed({
  get: () => props.limit,
  set: (value: number) => emit('update:limit', value),
})

const range = computed(() => {
  const from = (props.page - 1) * props.limit + 1
  const to = Math.min(props.page * props.limit, props.total)

  return `${from.toLocaleString('vi-VN')}–${to.toLocaleString('vi-VN')}`
})
</script>

<template>
  <div
    v-if="total > 0"
    class="d-flex flex-wrap align-center justify-space-between ga-3 mt-6"
  >
    <div class="d-flex align-center ga-4">
      <span class="text-body-2 text-medium-emphasis">
        {{ range }} / {{ total.toLocaleString('vi-VN') }} sản phẩm
      </span>
      <VSelect
        v-model="limitModel"
        :items="pageSizes"
        density="compact"
        variant="outlined"
        hide-details
        style="max-inline-size: 136px"
      />
    </div>

    <VPagination
      v-model="pageModel"
      :length="Math.min(totalPage, 500)"
      :total-visible="6"
    />
  </div>
</template>
