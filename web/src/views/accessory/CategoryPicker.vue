<script setup lang="ts">
import type { CategoryNode } from '@/types'

const props = defineProps<{
  tree: CategoryNode[]
  selected: number | null
  loading: boolean
}>()

const emit = defineEmits<{
  select: [id: number]
  close: []
}>()

/** Root -> current node. Drilling only renders one level at a time, so the
 *  1162-node catalogue never blocks the UI. */
const path = ref<CategoryNode[]>([])

function locate(id: number | null): CategoryNode[] {
  if (id === null)
    return []

  const trail: CategoryNode[] = []

  const walk = (nodes: CategoryNode[], stack: CategoryNode[]): boolean => {
    for (const node of nodes) {
      const next = [...stack, node]

      if (node.id === id) {
        trail.push(...next)

        return true
      }

      if (walk(node.children, next))
        return true
    }

    return false
  }

  walk(props.tree, [])

  return trail
}

onMounted(() => {
  path.value = locate(props.selected)
})

const current = computed(() => path.value.at(-1) ?? null)
const nodes = computed(() => (current.value ? current.value.children : props.tree))
const canApply = computed(() =>
  Boolean(current.value && (current.value.count > 0 || current.value.children.length === 0)),
)

function drill(node: CategoryNode) {
  if (node.children.length) {
    path.value = [...path.value, node]

    return
  }

  if (node.count > 0) {
    emit('select', node.id)
    emit('close')
  }
}

function applyCurrent() {
  if (!current.value)
    return

  emit('select', current.value.id)
  emit('close')
}

function breadcrumb(index: number) {
  path.value = path.value.slice(0, index + 1)
}
</script>

<template>
  <div>
    <VProgressLinear
      v-if="loading"
      indeterminate
      color="primary"
      rounded
      class="mb-3"
    />

    <template v-else>
      <div class="d-flex flex-wrap align-center ga-1 mb-3">
        <VChip
          size="small"
          variant="text"
          @click="path = []"
        >
          Tất cả
        </VChip>
        <template
          v-for="(node, index) in path"
          :key="node.id"
        >
          <VIcon
            icon="ri-arrow-right-s-line"
            size="14"
            class="text-medium-emphasis"
          />
          <VChip
            size="small"
            :variant="index === path.length - 1 ? 'tonal' : 'text'"
            @click="breadcrumb(index)"
          >
            {{ node.name }}
          </VChip>
        </template>
      </div>

      <VBtn
        v-if="current && canApply"
        block
        color="primary"
        class="mb-3 text-none"
        prepend-icon="ri-check-line"
        @click="applyCurrent"
      >
        <span class="text-truncate">
          Xem “{{ current.name }}”<template v-if="current.count > 0"> · {{ current.count }} sản phẩm</template>
        </span>
      </VBtn>

      <VList
        density="compact"
        class="picker-list"
      >
        <VListItem
          v-for="node in nodes"
          :key="node.id"
          @click="drill(node)"
        >
          <template #prepend>
            <VIcon
              :icon="node.children.length ? 'ri-folder-3-line' : 'ri-price-tag-3-line'"
              size="18"
              class="text-medium-emphasis me-2"
            />
          </template>

          <template #title>
            <span class="text-body-2">{{ node.name }}</span>
          </template>

          <template #subtitle>
            <span class="text-caption text-medium-emphasis">
              {{ node.count > 0 ? `${node.count} sản phẩm` : '' }}<template v-if="node.count > 0 && node.children.length"> · </template><template v-if="node.children.length">{{ node.children.length }} danh mục con</template>
            </span>
          </template>

          <template #append>
            <VIcon
              :icon="node.children.length ? 'ri-arrow-right-s-line' : 'ri-check-line'"
              size="18"
              class="text-medium-emphasis"
            />
          </template>
        </VListItem>

        <div
          v-if="nodes.length === 0"
          class="text-caption text-medium-emphasis pa-4 text-center"
        >
          Không có danh mục con.
        </div>
      </VList>
    </template>
  </div>
</template>

<style scoped>
.picker-list {
  max-block-size: 56vh;
  overflow-y: auto;
}
</style>
