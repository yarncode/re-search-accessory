<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { apiGet, apiSend, formatVnd, pickNativePath } from '@/api'
import { rememberSetting } from '@/settings'
import ProductDetail from '@/views/accessory/ProductDetail.vue'
import BomRow from '@/views/bom/BomRow.vue'
import { useBomStore } from '@/stores/bom'
import { type BomItem, detectColumns, parseBomFile } from '@/utils/bom'
import type { ThegioiicCartResult, ThegioiicStatus } from '@/types'

const store = useBomStore()

const KICAD_CMD = 'kicad-cli sch export bom -o bom.csv --fields "Reference,Value,Footprint,${QUANTITY},DNP" board.kicad_sch'

const fileInput = ref<HTMLInputElement | null>(null)
const fileError = ref('')
const dragging = ref(false)
const filter = ref<'all' | 'ok' | 'review' | 'missing' | 'pending' | 'shortage'>('all')
const tableSearch = ref('')
const groupFilter = ref('')
const selectedIds = ref<string[]>([])
const groupDialog = ref(false)
const groupNameDraft = ref('')
const tgConnected = ref(false)
const tgLoading = ref(true)
const pushOpen = ref(false)
const pushing = ref(false)
const pushError = ref('')
const pushResult = ref<ThegioiicCartResult | null>(null)
const pushScope = ref<{ type: 'bom' } | { type: 'group'; name: string } | null>(null)
const manualOpen = ref(false)
const tokenDraft = ref('')
const connecting = ref(false)
const connectError = ref('')
const detailSlug = ref<string | null>(null)

const savedOpen = ref(false)
const saveOpen = ref(false)
const saveName = ref('')
const jlcHelpOpen = ref(false)
const optimizing = ref(false)

const snackbar = ref(false)
const snackText = ref('')

function notify(message: string) {
  snackText.value = message
  snackbar.value = true
}

const searchItems = computed(() => store.headers.map(header => ({ title: header, value: header })))
const optionalItems = computed(() => [{ title: '(không dùng)', value: '' }, ...searchItems.value])

const searchColumn = computed({
  get: () => store.columns.search,
  set: (value: string) => store.setColumn('search', value),
})
const qtyColumn = computed({
  get: () => store.columns.qty,
  set: (value: string) => store.setColumn('qty', value),
})
const refColumn = computed({
  get: () => store.columns.ref,
  set: (value: string) => store.setColumn('ref', value),
})
const footprintColumn = computed({
  get: () => store.columns.footprint,
  set: (value: string) => store.setColumn('footprint', value),
})

const boardsDraft = ref(String(store.boards))
const scrapDraft = ref(String(store.scrap))
const fxDraft = ref(String(store.fxRate))
const projectDraft = ref(store.project)

watch(() => store.boards, value => (boardsDraft.value = String(value)))
watch(() => store.project, value => (projectDraft.value = value))
watch(() => store.scrap, value => (scrapDraft.value = String(value)))
watch(() => store.fxRate, value => (fxDraft.value = String(value)))

function commitBoards() {
  const value = Number(boardsDraft.value)

  if (Number.isFinite(value) && value > 0)
    store.setBoards(value)
  else
    boardsDraft.value = String(store.boards)
}

function commitScrap() {
  const value = Number(scrapDraft.value)

  if (Number.isFinite(value) && value >= 0)
    store.setScrap(value)
  else
    scrapDraft.value = String(store.scrap)
}

function commitProject() {
  store.setProject(projectDraft.value)
  projectDraft.value = store.project

  if (store.project)
    rememberSetting('kicad_project', store.project)
}

async function pickProject() {
  const path = await pickNativePath('folder', 'Chọn thư mục project KiCad')

  if (!path)
    return

  projectDraft.value = path
  commitProject()
}

function commitFx() {
  const value = Number(fxDraft.value)

  if (Number.isFinite(value) && value > 0)
    store.setFxRate(value)
  else
    fxDraft.value = String(store.fxRate)
}

const searchIsFootprint = computed(() =>
  Boolean(store.columns.search) && store.columns.search === store.columns.footprint)

const filters = computed(() => [
  { value: 'all' as const, label: `Tất cả (${store.counts.total})` },
  { value: 'ok' as const, label: `OK (${store.counts.ok})` },
  { value: 'review' as const, label: `Cần xem (${store.counts.review})` },
  { value: 'missing' as const, label: `Không thấy (${store.counts.missing})` },
  { value: 'pending' as const, label: `Chưa xử lý (${store.counts.pending})` },
  { value: 'shortage' as const, label: `Thiếu hàng (${store.counts.shortage})` },
])

function normalizeSearch(value: string): string {
  return value
    .normalize('NFD')
    .replace(/[\u0300-\u036F]/g, '')
    .toLowerCase()
    .replace(/đ/g, 'd')
}

const searchTokens = computed(() =>
  normalizeSearch(tableSearch.value).split(/\s+/).filter(Boolean))

function searchText(item: BomItem): string {
  return normalizeSearch([
    item.ref,
    item.input,
    item.footprint,
    item.note,
    item.product?.name ?? '',
    item.product?.manufacturer_code ?? '',
    item.product?.supplier?.name ?? '',
    item.group,
    ...Object.values(item.extra ?? {}),
  ].join(' '))
}

const visibleItems = computed(() => {
  let list = groupFilter.value === ''
    ? store.items.filter(item => !item.group)
    : store.items.filter(item => item.group === groupFilter.value)

  if (filter.value === 'shortage')
    list = list.filter(item => store.shortage(item))
  else if (filter.value !== 'all')
    list = list.filter(item => item.status === filter.value)

  const tokens = searchTokens.value

  if (!tokens.length)
    return list

  return list.filter(item => {
    const haystack = searchText(item)

    return tokens.every(token => haystack.includes(token))
  })
})

const selectedSet = computed(() => new Set(selectedIds.value))
const selectedItems = computed(() => store.items.filter(item => selectedSet.value.has(item.id)))
const visibleIds = computed(() => visibleItems.value.map(item => item.id))

const allVisibleSelected = computed(() =>
  visibleIds.value.length > 0 && visibleIds.value.every(id => selectedSet.value.has(id)))

const someVisibleSelected = computed(() =>
  visibleIds.value.some(id => selectedSet.value.has(id)))

const activeGroup = computed(() =>
  groupFilter.value ? store.groups.find(group => group.name === groupFilter.value) ?? null : null)

const groupedCount = computed(() => store.counts.total - store.ungroupedCount)

function itemsCost(list: BomItem[]): number {
  return list.reduce((sum, item) =>
    sum + (item.product ? item.product.price * store.orderQty(item) : 0), 0)
}

const ungroupedCost = computed(() => itemsCost(store.items.filter(item => !item.group)))
const selectedCost = computed(() => itemsCost(selectedItems.value))

const pushItems = computed(() => {
  const scope = pushScope.value

  if (!scope)
    return []

  return scope.type === 'bom'
    ? store.items.filter(item => !item.group)
    : store.items.filter(item => item.group === scope.name)
})

const pushMatchedItems = computed(() => pushItems.value.filter(item => item.product))
const pushSkippedCount = computed(() => pushItems.value.length - pushMatchedItems.value.length)
const pushTotal = computed(() => itemsCost(pushMatchedItems.value))

const pushScopeLabel = computed(() => {
  const scope = pushScope.value

  if (!scope)
    return ''

  return scope.type === 'bom' ? 'BOM tổng' : scope.name
})

const hasActiveFilters = computed(() =>
  groupFilter.value !== '' || filter.value !== 'all' || Boolean(tableSearch.value))

function clearFilters() {
  groupFilter.value = ''
  filter.value = 'all'
  tableSearch.value = ''
}

watch(() => store.items.map(item => item.id).join('|'), () => {
  const existing = new Set(store.items.map(item => item.id))
  const next = selectedIds.value.filter(id => existing.has(id))

  if (next.length !== selectedIds.value.length)
    selectedIds.value = next
})

watch(() => store.groups.map(group => group.name).join('|'), () => {
  if (groupFilter.value && !store.groups.some(group => group.name === groupFilter.value))
    groupFilter.value = ''
})

function toggleSelect(id: string) {
  selectedIds.value = selectedSet.value.has(id)
    ? selectedIds.value.filter(value => value !== id)
    : [...selectedIds.value, id]
}

function toggleSelectVisible() {
  const visible = new Set(visibleIds.value)

  if (allVisibleSelected.value) {
    selectedIds.value = selectedIds.value.filter(id => !visible.has(id))

    return
  }

  selectedIds.value = [...new Set([...selectedIds.value, ...visibleIds.value])]
}

function clearSelection() {
  selectedIds.value = []
}

function openGroupDialog() {
  if (!selectedItems.value.length)
    return

  groupNameDraft.value = ''
  groupDialog.value = true
}

function assignSelectedToGroup(name: string) {
  const count = store.setGroup(selectedItems.value.map(item => item.id), name)

  groupDialog.value = false

  if (!count) {
    notify(name ? 'Các dòng đã nằm trong nhóm này' : 'Các dòng đang chưa có nhóm')

    return
  }

  selectedIds.value = []

  if (name) {
    tableSearch.value = ''
    filter.value = 'all'
    groupFilter.value = ''

    notify(store.ungroupedCount > 0
      ? `Đã gán ${count} dòng vào nhóm “${name}”. Đã ẩn khỏi BOM tổng, còn ${store.ungroupedCount} dòng chưa nhóm.`
      : `Đã gán ${count} dòng vào nhóm “${name}”. Tất cả linh kiện đã được gán nhóm.`)
  }
  else {
    groupFilter.value = ''

    notify(`Đã bỏ nhóm ${count} dòng, đã trả về BOM tổng`)
  }
}

function createGroupAndAssign() {
  const name = groupNameDraft.value.trim()

  if (name)
    assignSelectedToGroup(name)
}

function markActiveGroupOrdered() {
  const group = activeGroup.value

  if (!group)
    return

  const count = store.markGroupOrdered(group.name)

  notify(count
    ? `Đã đánh dấu đặt ${count} dòng trong nhóm “${group.name}”`
    : 'Không còn dòng nào ở trạng thái “Cần mua”')
}

function exportGroupCsv() {
  const group = activeGroup.value

  if (!group)
    return

  store.exportCsv(group.name)
  notify(`Đã xuất CSV nhóm “${group.name}”`)
}

function clearActiveGroup() {
  const group = activeGroup.value

  if (!group)
    return

  const count = store.clearGroup(group.name)

  groupFilter.value = ''
  notify(`Đã bỏ nhóm ${count} dòng khỏi “${group.name}”, đã trả về BOM tổng`)
}

/* ---------------- thegioiic cart bridge ---------------- */
async function loadThegioiicStatus() {
  tgLoading.value = true

  try {
    const status = await apiGet<ThegioiicStatus>('/thegioiic/status')

    tgConnected.value = status.connected
  }
  catch {
    tgConnected.value = false
  }
  finally {
    tgLoading.value = false
  }
}

function connectThegioiic() {
  const callback = encodeURIComponent('https://www.thegioiic.com/auth/google/callback')

  try {
    if (pushOpen.value && pushScope.value)
      sessionStorage.setItem('res-push-scope', JSON.stringify(pushScope.value))
  }
  catch {
    // sessionStorage can be unavailable in hardened environments
  }

  window.location.href = `https://api.thegioiic.com/v1/auth/google/login?redirect_uri=${callback}`
}

function openManualConnect() {
  tokenDraft.value = ''
  connectError.value = ''
  manualOpen.value = true
}

async function connectWithToken() {
  const token = tokenDraft.value.trim()

  if (!token || connecting.value)
    return

  connecting.value = true
  connectError.value = ''

  try {
    await apiSend('POST', '/thegioiic/connect', { token })
    tgConnected.value = true
    manualOpen.value = false
    notify('Đã kết nối tài khoản thegioiic')
  }
  catch (err) {
    connectError.value = (err as Error).message
  }
  finally {
    connecting.value = false
  }
}

async function disconnectThegioiic() {
  try {
    await apiSend('DELETE', '/thegioiic/connect')
    tgConnected.value = false
    notify('Đã ngắt kết nối thegioiic')
  }
  catch (err) {
    notify((err as Error).message)
  }
}

function handleThegioiicReturn() {
  const url = new URL(window.location.href)
  const state = url.searchParams.get('thegioiic')

  if (!state)
    return

  url.searchParams.delete('thegioiic')
  window.history.replaceState({}, '', `${url.pathname}${url.search}`)

  if (state === 'connected') {
    tgConnected.value = true
    notify('Đã kết nối tài khoản thegioiic')

    try {
      const saved = sessionStorage.getItem('res-push-scope')

      if (saved) {
        sessionStorage.removeItem('res-push-scope')
        pushScope.value = JSON.parse(saved) as { type: 'bom' } | { type: 'group'; name: string }
        pushResult.value = null
        pushError.value = ''
        pushOpen.value = true
      }
    }
    catch {
      // ignore malformed saved state
    }
  }
  else if (state === 'error') {
    tgConnected.value = false
    sessionStorage.removeItem('res-push-scope')
    notify('Kết nối thegioiic thất bại. Hãy thử lại.')
  }
  else {
    sessionStorage.removeItem('res-push-scope')
    notify('Đã quay lại app')
  }
}

function openPush(scope: { type: 'bom' } | { type: 'group'; name: string }) {
  pushScope.value = scope
  pushResult.value = null
  pushError.value = ''
  pushOpen.value = true
}

async function pushToThegioiic() {
  if (!pushMatchedItems.value.length || pushing.value)
    return

  pushing.value = true
  pushError.value = ''
  pushResult.value = null

  try {
    const result = await apiSend<ThegioiicCartResult>('POST', '/thegioiic/cart', {
      items: pushMatchedItems.value.map(item => ({
        product_id: item.product?.id ?? 0,
        quantity: store.orderQty(item),
      })),
    })

    pushResult.value = result

    if (result.failed.length) {
      if (result.failed.some(item => /hết hạn|đăng nhập|401/i.test(item.error)))
        tgConnected.value = false

      notify(`Đã thêm ${result.added}/${pushMatchedItems.value.length} dòng vào giỏ thegioiic`)
    }
    else {
      notify(`Đã thêm ${result.added} dòng vào giỏ thegioiic`)
    }
  }
  catch (err) {
    if ((err as { status?: number }).status === 401)
      tgConnected.value = false

    pushError.value = (err as Error).message
  }
  finally {
    pushing.value = false
  }
}

function openThegioiicCart() {
  window.location.href = 'https://www.thegioiic.com/gio-hang?res_cart=1'
}

function removeRow(id: string) {
  selectedIds.value = selectedIds.value.filter(value => value !== id)
  store.removeItem(id)
}

function resetBom() {
  selectedIds.value = []
  groupFilter.value = ''
  store.reset()
}

const progress = computed(() =>
  (store.counts.total ? Math.round((store.done / store.counts.total) * 100) : 0))

const draftLabel = computed(() =>
  (store.draftAt ? `Tự lưu: ${new Date(store.draftAt).toLocaleTimeString('vi-VN')}` : 'Chưa tự lưu'))

const jlcLabel = computed(() => {
  if (!store.jlcStatus)
    return 'JLC: đang kiểm tra…'

  if (!store.jlcStatus.available)
    return 'JLC: chưa có database'

  return `JLC: ${store.jlcStatus.parts.toLocaleString('vi-VN')} part`
})

async function loadFile(file: File) {
  fileError.value = ''

  try {
    const parsed = await parseBomFile(file)
    const map = detectColumns(parsed.headers)

    // KiCad exports a DNP column; those components must not be purchased.
    const dnpColumn = parsed.headers.find(header => /^(dnp|do not populate|không lắp|khong lap|exclude)$/i.test(header.trim()))
    let rows = parsed.rows
    let skipped = 0

    if (dnpColumn) {
      rows = rows.filter(row => {
        const value = String(row[dnpColumn] ?? '').trim().toLowerCase()
        const isDnp = ['1', 'yes', 'true', 'dnp', 'x', 'y'].includes(value)

        if (isDnp)
          skipped += 1

        return !isDnp
      })
    }

    store.setParsed(file.name, parsed.headers, rows, map)
    notify(`Đã đọc ${rows.length} dòng${skipped ? `, bỏ qua ${skipped} DNP` : ''}`)
  }
  catch (err) {
    fileError.value = (err as Error).message
  }
}

async function onFile(event: Event) {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]

  input.value = ''

  if (file)
    await loadFile(file)
}

async function onDrop(event: DragEvent) {
  dragging.value = false
  const file = event.dataTransfer?.files?.[0]

  if (file)
    await loadFile(file)
}

function openSave() {
  if (!store.items.length)
    return

  saveName.value = store.currentName || store.fileName.replace(/\.(csv|xlsx|xls)$/i, '') || 'BOM'
  saveOpen.value = true
}

async function doSave() {
  try {
    await store.saveAs(saveName.value)
    saveOpen.value = false
    notify('Đã lưu BOM vào máy')
  }
  catch (err) {
    notify((err as Error).message)
  }
}

async function doLoad(id: string) {
  try {
    await store.loadDoc(id)
    savedOpen.value = false
    notify('Đã mở BOM đã lưu')
  }
  catch (err) {
    notify((err as Error).message)
  }
}

async function doDelete(id: string) {
  try {
    await store.deleteDoc(id)
  }
  catch (err) {
    notify((err as Error).message)
  }
}

async function optimizeAll() {
  if (optimizing.value)
    return

  optimizing.value = true

  try {
    const saved = await store.optimizeAll()

    notify(saved > 0
      ? `Đã chọn phương án rẻ hơn, tiết kiệm ${formatVnd(saved)}`
      : 'Không tìm được phương án rẻ hơn cho các dòng hiện tại')
  }
  finally {
    optimizing.value = false
  }
}

async function optimizeOne(id: string) {
  const saved = await store.optimizeRow(id)

  notify(saved > 0
    ? `Đã tối ưu dòng, tiết kiệm ${formatVnd(saved)}`
    : 'Không có phương án rẻ hơn cho dòng này')
}

async function compareJlc() {
  if (!store.jlcStatus?.available) {
    jlcHelpOpen.value = true

    return
  }

  await store.compareJlc()
  notify(`Đã so sánh ${store.jlcCount} dòng với JLCPCB`)
}

async function copyKicadCommand() {
  try {
    await navigator.clipboard.writeText(KICAD_CMD)
    notify('Đã copy lệnh KiCad')
  }
  catch {
    notify('Không copy được')
  }
}

function handleCopy(label: string, text: string) {
  navigator.clipboard
    .writeText(text)
    .then(() => notify(label))
    .catch(() => notify('Không copy được vào clipboard'))
}

function onKey(event: KeyboardEvent) {
  if (!(event.ctrlKey || event.metaKey))
    return

  const target = event.target as HTMLElement | null

  if (target && ['INPUT', 'TEXTAREA'].includes(target.tagName))
    return

  const key = event.key.toLowerCase()

  if (key === 'z') {
    event.preventDefault()
    if (event.shiftKey)
      store.redo()
    else
      store.undo()
  }
  else if (key === 'y') {
    event.preventDefault()
    store.redo()
  }
}

onMounted(() => {
  void store.init()
  loadThegioiicStatus()
  handleThegioiicReturn()
  window.addEventListener('keydown', onKey)
})

onBeforeUnmount(() => window.removeEventListener('keydown', onKey))
</script>

<template>
  <div>
    <div class="d-flex flex-wrap align-center ga-2 mb-2">
      <div style="min-inline-size: 0">
        <h1 class="text-h5 font-weight-bold">
          Phân tích BOM
        </h1>
        <div class="text-body-2 text-medium-emphasis">
          Import từ KiCad / CSV / Excel, tự tìm linh kiện, tính theo số bo, chia nhóm mua trước và lưu lại.
        </div>
      </div>
    </div>

    <div class="d-flex flex-wrap align-center ga-2 mb-4 page-sticky page-sticky--bordered">
      <span
        v-if="store.items.length"
        class="text-caption text-medium-emphasis text-truncate"
        style="max-inline-size: 320px"
      >
        {{ store.currentName || store.fileName }} · {{ store.counts.total }} dòng
      </span>

      <VSpacer />

      <VTooltip text="Tự nhận cột Reference/Value/Footprint + bỏ qua DNP">
        <template #activator="{ props: tip }">
          <VBtn
            v-bind="tip"
            variant="tonal"
            prepend-icon="ri-cpu-line"
            @click="fileInput?.click()"
          >
            Import KiCad
          </VBtn>
        </template>
      </VTooltip>
      <VBtn
        variant="tonal"
        prepend-icon="ri-upload-2-line"
        @click="fileInput?.click()"
      >
        Tải file
      </VBtn>
      <VBtn
        variant="tonal"
        prepend-icon="ri-folder-open-line"
        @click="savedOpen = true"
      >
        BOM đã lưu
      </VBtn>
      <VBtn
        v-if="store.items.length"
        variant="tonal"
        prepend-icon="ri-save-3-line"
        @click="openSave"
      >
        Lưu
      </VBtn>
      <VBtn
        v-if="store.items.length"
        color="primary"
        prepend-icon="ri-download-2-line"
        @click="store.exportCsv()"
      >
        Xuất CSV
      </VBtn>
      <VBtn
        v-if="store.items.length"
        variant="text"
        color="error"
        @click="resetBom"
      >
        Xoá
      </VBtn>

      <input
        ref="fileInput"
        type="file"
        accept=".csv,.xlsx,.xls"
        hidden
        @change="onFile"
      >
    </div>

    <VAlert
      v-if="fileError"
      type="error"
      variant="tonal"
      class="mb-4"
      closable
      @click:close="fileError = ''"
    >
      {{ fileError }}
    </VAlert>

    <!-- 👉 Chưa có dữ liệu -->
    <VCard
      v-if="!store.items.length"
      class="drop-zone pa-10 text-center"
      :class="{ dragging }"
      elevation="0"
      @click="fileInput?.click()"
      @dragover.prevent="dragging = true"
      @dragleave.prevent="dragging = false"
      @drop.prevent="onDrop"
    >
      <VIcon
        icon="ri-file-upload-line"
        size="56"
        color="primary"
      />
      <div class="text-h6 font-weight-bold mt-3">
        Kéo file CSV / Excel vào đây
      </div>
      <div class="text-body-2 text-medium-emphasis mt-1">
        Hỗ trợ .csv, .xlsx, .xls và file BOM xuất từ KiCad (tự bỏ qua DNP).
      </div>
      <VBtn
        class="mt-5"
        color="primary"
        prepend-icon="ri-upload-2-line"
      >
        Chọn file
      </VBtn>

      <VSheet
        class="kicad-hint mt-6 mx-auto pa-3 text-start"
        rounded="lg"
        max-width="760"
      >
        <div class="d-flex align-center ga-2 mb-1">
          <VIcon
            icon="ri-terminal-box-line"
            size="18"
            color="primary"
          />
          <span class="text-caption font-weight-medium">Xuất BOM từ KiCad</span>
          <VSpacer />
          <VBtn
            size="x-small"
            variant="text"
            prepend-icon="ri-file-copy-line"
            @click.stop="copyKicadCommand"
          >
            Copy
          </VBtn>
        </div>
        <code class="kicad-cmd">{{ KICAD_CMD }}</code>
      </VSheet>

      <div class="text-caption text-medium-emphasis mt-4">
        Cột được nhận diện: “Manufacturer Part Number”, “MPN”, “Part”, “Value”, “Description”,
        “Qty”, “Quantity”, “Ref”, “Designator”, “Footprint”.
      </div>
    </VCard>

    <template v-else>
      <!-- 👉 Cấu hình -->
      <VCard
        class="pa-4 mb-4"
        elevation="0"
      >
        <div class="d-flex flex-wrap align-center ga-3">
          <VIcon
            icon="ri-file-list-3-line"
            color="primary"
          />
          <div>
            <div class="text-body-1 font-weight-medium">
              {{ store.currentName || store.fileName }}
            </div>
            <div class="text-caption text-medium-emphasis">
              {{ store.counts.total }} dòng · còn {{ store.ungroupedCount }} chưa nhóm · tìm theo cột “{{ store.columns.search }}” ·
              {{ draftLabel }} · {{ jlcLabel }}
            </div>
          </div>
          <VSpacer />

          <VTextField
            v-model="boardsDraft"
            type="number"
            label="Số bo"
            density="compact"
            variant="outlined"
            hide-details
            style="inline-size: 104px"
            @blur="commitBoards"
            @keydown.enter="commitBoards"
          />
          <VTextField
            v-model="scrapDraft"
            type="number"
            label="Hao hụt %"
            density="compact"
            variant="outlined"
            hide-details
            style="inline-size: 116px"
            @blur="commitScrap"
            @keydown.enter="commitScrap"
          />
          <VTextField
            v-model="fxDraft"
            type="number"
            label="Tỷ giá USD"
            density="compact"
            variant="outlined"
            hide-details
            style="inline-size: 130px"
            @blur="commitFx"
            @keydown.enter="commitFx"
          />
          <VTextField
            v-model="projectDraft"
            label="Project KiCad"
            placeholder="Bấm nút thư mục để chọn"
            density="compact"
            variant="outlined"
            hide-details
            readonly
            style="inline-size: 220px"
            @blur="commitProject"
            @keydown.enter="commitProject"
          />
          <VBtn
            variant="tonal"
            icon="ri-folder-open-line"
            title="Chọn thư mục project KiCad"
            @click="pickProject"
          />

          <VBtn
            color="primary"
            prepend-icon="ri-play-line"
            :loading="store.analyzing"
            @click="store.analyze()"
          >
            {{ store.counts.pending === store.counts.total ? 'Bắt đầu phân tích' : 'Phân tích lại' }}
          </VBtn>
          <VBtn
            v-if="store.analyzing"
            variant="text"
            @click="store.cancel()"
          >
            Dừng
          </VBtn>
          <VBtn
            variant="tonal"
            prepend-icon="ri-magic-line"
            :loading="optimizing"
            @click="optimizeAll"
          >
            Tối ưu giá
          </VBtn>
          <VBtn
            variant="tonal"
            prepend-icon="ri-bar-chart-grouped-line"
            :loading="store.jlcLoading"
            @click="compareJlc"
          >
            So sánh JLCPCB
          </VBtn>
          <VTooltip text="Hoàn tác (Ctrl+Z)">
            <template #activator="{ props: tip }">
              <IconBtn
                v-bind="tip"
                :disabled="!store.undoStack.length"
                @click="store.undo()"
              >
                <VIcon icon="ri-arrow-go-back-line" />
              </IconBtn>
            </template>
          </VTooltip>
          <VTooltip text="Làm lại (Ctrl+Shift+Z)">
            <template #activator="{ props: tip }">
              <IconBtn
                v-bind="tip"
                :disabled="!store.redoStack.length"
                @click="store.redo()"
              >
                <VIcon icon="ri-arrow-go-forward-line" />
              </IconBtn>
            </template>
          </VTooltip>
        </div>

        <div class="d-flex flex-wrap align-center ga-2 mt-4">
          <span class="text-caption text-medium-emphasis">Tìm theo cột</span>
          <VSelect
            v-model="searchColumn"
            :items="searchItems"
            density="compact"
            variant="outlined"
            hide-details
            style="inline-size: 220px"
          />
          <span class="text-caption text-medium-emphasis">Footprint</span>
          <VSelect
            v-model="footprintColumn"
            :items="optionalItems"
            density="compact"
            variant="outlined"
            hide-details
            style="inline-size: 190px"
          />
          <span class="text-caption text-medium-emphasis">Số lượng</span>
          <VSelect
            v-model="qtyColumn"
            :items="optionalItems"
            density="compact"
            variant="outlined"
            hide-details
            style="inline-size: 160px"
          />
          <span class="text-caption text-medium-emphasis">Ref</span>
          <VSelect
            v-model="refColumn"
            :items="optionalItems"
            density="compact"
            variant="outlined"
            hide-details
            style="inline-size: 160px"
          />
        </div>

        <div class="text-caption text-medium-emphasis mt-2">
          Footprint dùng để xác nhận và tìm bổ sung (0805, SOT-23, SOIC-8…). Nên chọn
          Value/MPN làm cột chính. “Project KiCad” dùng để xem 3D footprint (đọc
          fp-lib-table của project + ${KIPRJMOD}).
        </div>

        <VAlert
          v-if="searchIsFootprint"
          type="warning"
          variant="tonal"
          density="compact"
          class="mt-3 text-caption"
        >
          Đang tìm theo cột Footprint: kết quả chỉ là gợi ý theo package. Hãy chọn Value/MPN
          làm cột chính và giữ Footprint ở cột phụ.
        </VAlert>

        <div
          v-if="store.analyzing || store.done"
          class="mt-4"
        >
          <VProgressLinear
            :model-value="progress"
            color="primary"
            rounded
            height="6"
          />
          <div class="text-caption text-medium-emphasis mt-1">
            {{ store.done }}/{{ store.counts.total }} dòng đã xử lý
          </div>
        </div>
      </VCard>

      <!-- 👉 Tổng quan -->
      <div class="bom-summary mb-4">
        <VCard
          class="pa-4"
          elevation="0"
        >
          <div class="text-caption text-medium-emphasis">
            Tổng dòng
          </div>
          <div class="text-h6 font-weight-bold">
            {{ store.counts.total }}
          </div>
        </VCard>
        <VCard
          v-if="groupedCount"
          class="pa-4"
          elevation="0"
        >
          <div class="text-caption text-medium-emphasis">
            Đã gán nhóm
          </div>
          <div class="text-h6 font-weight-bold text-primary">
            {{ groupedCount }}
          </div>
        </VCard>
        <VCard
          class="pa-4"
          elevation="0"
        >
          <div class="text-caption text-medium-emphasis">
            Đã OK
          </div>
          <div class="text-h6 font-weight-bold text-success">
            {{ store.counts.ok }}
          </div>
        </VCard>
        <VCard
          class="pa-4"
          elevation="0"
        >
          <div class="text-caption text-medium-emphasis">
            Cần xem
          </div>
          <div class="text-h6 font-weight-bold text-warning">
            {{ store.counts.review }}
          </div>
        </VCard>
        <VCard
          class="pa-4"
          elevation="0"
        >
          <div class="text-caption text-medium-emphasis">
            Không thấy
          </div>
          <div class="text-h6 font-weight-bold text-error">
            {{ store.counts.missing }}
          </div>
        </VCard>
        <VCard
          class="pa-4"
          elevation="0"
        >
          <div class="text-caption text-medium-emphasis">
            Thiếu hàng
          </div>
          <div class="text-h6 font-weight-bold text-error">
            {{ store.counts.shortage }}
          </div>
        </VCard>
        <VCard
          class="pa-4"
          elevation="0"
        >
          <div class="text-caption text-medium-emphasis">
            Tạm tính toàn BOM ({{ store.boards }} bo)
          </div>
          <div class="text-h6 font-weight-bold text-primary">
            {{ formatVnd(store.totalCost) }}
          </div>
        </VCard>
        <VCard
          v-if="store.jlcCount"
          class="pa-4"
          elevation="0"
        >
          <div class="text-caption text-medium-emphasis">
            JLC chênh lệch
          </div>
          <div
            class="text-h6 font-weight-bold"
            :class="store.jlcDelta > 0 ? 'text-success' : 'text-warning'"
          >
            {{ store.jlcDelta > 0 ? '+' : '' }}{{ formatVnd(store.jlcDelta) }}
          </div>
        </VCard>
      </div>

      <!-- 👉 Nhóm mua -->
      <div class="d-flex flex-wrap align-center ga-2 mb-3">
        <VIcon
          icon="ri-group-line"
          size="18"
          class="text-medium-emphasis"
        />
        <span class="text-caption text-medium-emphasis">Nhóm mua</span>
        <VChip
          :color="groupFilter === '' ? 'primary' : undefined"
          :variant="groupFilter === '' ? 'flat' : 'tonal'"
          title="Chỉ hiện các linh kiện chưa gán nhóm"
          @click="groupFilter = ''"
        >
          BOM tổng ({{ store.ungroupedCount }})
        </VChip>
        <VChip
          v-for="group in store.groups"
          :key="group.name"
          :color="groupFilter === group.name ? 'primary' : undefined"
          :variant="groupFilter === group.name ? 'flat' : 'tonal'"
          @click="groupFilter = group.name"
        >
          {{ group.name }} ({{ group.count }})
        </VChip>
        <span
          v-if="!store.groups.length"
          class="text-caption text-medium-emphasis"
        >
          · Chọn các dòng cần mua trước rồi bấm “Gán vào nhóm”.
        </span>
        <VSpacer />
        <VChip
          v-if="tgConnected"
          size="small"
          color="success"
          variant="tonal"
          closable
          :disabled="tgLoading"
          @click:close="disconnectThegioiic"
        >
          <VIcon
            start
            icon="ri-links-line"
            size="14"
          />
          thegioiic đã kết nối
        </VChip>
        <VBtn
          v-else
          size="small"
          variant="text"
          prepend-icon="ri-login-circle-line"
          :loading="tgLoading"
          @click="connectThegioiic"
        >
          Kết nối thegioiic
        </VBtn>
      </div>

      <!-- 👉 Thông tin nhóm đang lọc -->
      <VCard
        v-if="activeGroup"
        class="pa-3 mb-3 group-panel"
        elevation="0"
      >
        <div class="d-flex flex-wrap align-center ga-3">
          <VIcon
            icon="ri-flag-2-line"
            color="primary"
            size="24"
          />
          <div>
            <div class="text-body-1 font-weight-medium">
              {{ activeGroup.name }}
            </div>
            <div class="text-body-2 font-weight-bold text-primary">
              Tổng tiền nhóm: {{ formatVnd(activeGroup.cost) }}
            </div>
            <div class="text-caption text-medium-emphasis">
              {{ activeGroup.count }} dòng ·
              Cần mua {{ activeGroup.need }} · Đã đặt {{ activeGroup.ordered }} · Đã nhận {{ activeGroup.received }}
            </div>
          </div>
          <VSpacer />
          <VBtn
            size="small"
            variant="tonal"
            prepend-icon="ri-shopping-cart-2-line"
            :disabled="!activeGroup.need"
            @click="markActiveGroupOrdered"
          >
            Đánh dấu đã đặt
          </VBtn>
          <VBtn
            size="small"
            color="primary"
            prepend-icon="ri-shopping-cart-2-line"
            @click="openPush({ type: 'group', name: activeGroup.name })"
          >
            Đẩy lên thegioiic
          </VBtn>
          <VBtn
            size="small"
            variant="text"
            prepend-icon="ri-shopping-bag-3-line"
            :disabled="!tgConnected"
            @click="openThegioiicCart"
          >
            Mở giỏ hàng
          </VBtn>
          <VBtn
            size="small"
            variant="tonal"
            prepend-icon="ri-download-2-line"
            @click="exportGroupCsv"
          >
            Xuất CSV nhóm
          </VBtn>
          <VBtn
            size="small"
            variant="text"
            color="error"
            @click="clearActiveGroup"
          >
            Bỏ nhóm
          </VBtn>
        </div>
      </VCard>

      <!-- 👉 Thông tin BOM tổng -->
      <VCard
        v-if="!activeGroup"
        class="pa-3 mb-3 group-panel"
        elevation="0"
      >
        <div class="d-flex flex-wrap align-center ga-3">
          <VIcon
            icon="ri-file-list-3-line"
            color="primary"
            size="24"
          />
          <div>
            <div class="text-body-1 font-weight-medium">
              BOM tổng
            </div>
            <div class="text-caption text-medium-emphasis">
              {{ store.ungroupedCount }} dòng chưa nhóm · {{ groupedCount }} dòng đã gán nhóm
            </div>
          </div>
          <VSpacer />
          <VBtn
            size="small"
            color="primary"
            prepend-icon="ri-shopping-cart-2-line"
            :disabled="!store.ungroupedCount"
            @click="openPush({ type: 'bom' })"
          >
            Đẩy lên thegioiic
          </VBtn>
          <VBtn
            size="small"
            variant="text"
            prepend-icon="ri-shopping-bag-3-line"
            :disabled="!tgConnected"
            @click="openThegioiicCart"
          >
            Mở giỏ hàng
          </VBtn>
          <div class="text-end">
            <div class="text-caption text-medium-emphasis">
              Tổng tiền BOM tổng
            </div>
            <div class="text-h6 font-weight-bold text-primary">
              {{ formatVnd(ungroupedCost) }}
            </div>
          </div>
        </div>
      </VCard>

      <!-- 👉 Lọc trạng thái + tìm nhanh -->
      <div class="d-flex flex-wrap align-center ga-2 mb-3">
        <VTextField
          v-model="tableSearch"
          density="compact"
          variant="outlined"
          hide-details
          clearable
          prepend-inner-icon="ri-search-line"
          placeholder="Tìm nhanh theo Ref, từ khoá, tên sản phẩm, MPN…"
          style="inline-size: 360px"
          @keydown.esc="tableSearch = ''"
        />
        <span
          v-if="tableSearch"
          class="text-caption text-medium-emphasis"
        >
          {{ visibleItems.length }}/{{ store.counts.total }} dòng
        </span>
        <VChip
          v-for="entry in filters"
          :key="entry.value"
          :color="filter === entry.value ? 'primary' : undefined"
          :variant="filter === entry.value ? 'flat' : 'tonal'"
          @click="filter = entry.value"
        >
          {{ entry.label }}
        </VChip>
      </div>

      <!-- 👉 Dòng đang chọn -->
      <VCard
        v-if="selectedItems.length"
        class="pa-2 mb-3 group-select-bar"
        elevation="0"
      >
        <div class="d-flex flex-wrap align-center ga-2 px-2">
          <VIcon
            icon="ri-checkbox-multiple-line"
            color="primary"
          />
          <span class="text-body-2">
            Đã chọn <b>{{ selectedItems.length }}</b> dòng ·
            Tạm tính <b class="text-primary">{{ formatVnd(selectedCost) }}</b>
          </span>
          <VSpacer />
          <VBtn
            size="small"
            color="primary"
            prepend-icon="ri-play-list-add-line"
            @click="openGroupDialog"
          >
            Gán vào nhóm
          </VBtn>
          <VBtn
            size="small"
            variant="text"
            @click="clearSelection"
          >
            Bỏ chọn
          </VBtn>
        </div>
      </VCard>

      <!-- 👉 Bảng BOM -->
      <div class="bom-table-scroll">
        <VCard
          class="bom-table"
          elevation="0"
        >
          <div class="bom-head">
            <div class="bom-select-all">
              <VCheckbox
                :model-value="allVisibleSelected"
                :indeterminate="!allVisibleSelected && someVisibleSelected"
                density="compact"
                hide-details
                aria-label="Chọn tất cả dòng đang hiển thị"
                @update:model-value="toggleSelectVisible"
              />
            </div>
            <span>Ref</span>
            <span>SL</span>
            <span>Cần / đặt</span>
            <span>Từ khoá</span>
            <span>Sản phẩm khớp</span>
            <span>Mua</span>
            <span class="text-end">Thao tác</span>
          </div>

          <BomRow
            v-for="item in visibleItems"
            :key="item.id"
            :item="item"
            :selected="selectedSet.has(item.id)"
            @select="toggleSelect(item.id)"
            @toggle-ok="store.toggleOk(item.id)"
            @pick="(product) => store.pick(item.id, product)"
            @open="(slug) => (detailSlug = slug)"
            @update-qty="(qty) => store.setQty(item.id, qty)"
            @update-input="(value) => store.setInput(item.id, value)"
            @analyze-row="store.analyzeItem(item.id)"
            @purchase="(status) => store.setPurchase(item.id, status)"
            @ordered-at="(value) => store.setOrderedAt(item.id, value)"
            @supplier="(value) => store.setSupplier(item.id, value)"
            @optimize="optimizeOne(item.id)"
            @remove="removeRow(item.id)"
          />

          <div
            v-if="!visibleItems.length"
            class="pa-8 text-center text-medium-emphasis text-body-2"
          >
            <div>
              {{ tableSearch
                ? 'Không có dòng nào khớp từ khoá tìm kiếm.'
                : groupFilter === ''
                  ? filter !== 'all'
                    ? 'BOM tổng không có dòng nào ở trạng thái này.'
                    : 'Tất cả linh kiện đã được gán nhóm.'
                  : 'Nhóm này không có dòng nào khớp bộ lọc trạng thái hiện tại.' }}
            </div>
            <VBtn
              v-if="hasActiveFilters"
              class="mt-3"
              size="small"
              variant="tonal"
              prepend-icon="ri-filter-off-line"
              @click="clearFilters"
            >
              Xoá bộ lọc
            </VBtn>
          </div>
        </VCard>
      </div>
    </template>

    <ProductDetail
      v-model="detailSlug"
      @copy="handleCopy"
    />

    <!-- 👉 BOM đã lưu -->
    <VDialog
      v-model="savedOpen"
      max-width="720"
      scrollable
    >
      <VCard class="pa-2">
        <div class="d-flex align-center pa-2">
          <span class="text-h6">BOM đã lưu</span>
          <VSpacer />
          <IconBtn @click="savedOpen = false">
            <VIcon icon="ri-close-line" />
          </IconBtn>
        </div>

        <VList>
          <VListItem
            v-for="doc in store.savedDocs"
            :key="doc.id"
            :title="doc.name"
            :subtitle="new Date(doc.saved_at).toLocaleString('vi-VN')"
          >
            <template #append>
              <VBtn
                size="small"
                variant="tonal"
                @click="doLoad(doc.id)"
              >
                Mở
              </VBtn>
              <VBtn
                size="small"
                variant="text"
                color="error"
                @click="doDelete(doc.id)"
              >
                Xoá
              </VBtn>
            </template>
          </VListItem>
          <div
            v-if="store.docsReady && !store.savedDocs.length"
            class="text-caption text-medium-emphasis pa-4 text-center"
          >
            Chưa có BOM nào được lưu.
          </div>
        </VList>
      </VCard>
    </VDialog>

    <!-- 👉 Lưu BOM -->
    <VDialog
      v-model="saveOpen"
      max-width="440"
    >
      <VCard class="pa-2">
        <div class="d-flex align-center pa-2">
          <span class="text-h6">Lưu BOM</span>
          <VSpacer />
          <IconBtn @click="saveOpen = false">
            <VIcon icon="ri-close-line" />
          </IconBtn>
        </div>

        <div class="pa-3">
          <VTextField
            v-model="saveName"
            label="Tên BOM"
            variant="outlined"
            density="comfortable"
            hide-details
            @keydown.enter="doSave"
          />
          <div class="text-caption text-medium-emphasis mt-2">
            Lưu trong SQLite trên máy. Sau khi đặt tên, mọi thay đổi được <b>tự lưu</b> vào BOM này —
            không cần bấm Lưu lần nữa.
          </div>
          <div class="d-flex justify-end ga-2 mt-4">
            <VBtn
              variant="text"
              @click="saveOpen = false"
            >
              Huỷ
            </VBtn>
            <VBtn
              color="primary"
              @click="doSave"
            >
              Lưu
            </VBtn>
          </div>
        </div>
      </VCard>
    </VDialog>

    <!-- 👉 Gán nhóm mua -->
    <VDialog
      v-model="groupDialog"
      max-width="540"
    >
      <VCard class="pa-2">
        <div class="d-flex align-center pa-2">
          <span class="text-h6">Gán vào nhóm mua</span>
          <VSpacer />
          <IconBtn @click="groupDialog = false">
            <VIcon icon="ri-close-line" />
          </IconBtn>
        </div>

        <div class="pa-3">
          <div class="text-caption text-medium-emphasis mb-2">
            Đang chọn <b>{{ selectedItems.length }}</b> dòng. Tạo nhóm để ưu tiên mua trước
            hoặc thêm vào nhóm đã có.
          </div>

          <VTextField
            v-model="groupNameDraft"
            label="Tên nhóm mới"
            placeholder="VD: Đợt 1 – mua trước"
            variant="outlined"
            density="comfortable"
            hide-details
            autofocus
            @keydown.enter="createGroupAndAssign"
          />

          <div
            v-if="store.groups.length"
            class="mt-4"
          >
            <div class="text-caption text-medium-emphasis mb-1">
              Hoặc thêm vào nhóm đã có
            </div>
            <div class="d-flex flex-wrap ga-2">
              <VChip
                v-for="group in store.groups"
                :key="group.name"
                variant="tonal"
                @click="assignSelectedToGroup(group.name)"
              >
                {{ group.name }} ({{ group.count }})
              </VChip>
            </div>
          </div>

          <div class="d-flex flex-wrap align-center ga-2 mt-5">
            <VBtn
              variant="text"
              color="error"
              :disabled="!selectedItems.length"
              @click="assignSelectedToGroup('')"
            >
              Bỏ nhóm
            </VBtn>
            <VSpacer />
            <VBtn
              variant="text"
              @click="groupDialog = false"
            >
              Huỷ
            </VBtn>
            <VBtn
              color="primary"
              :disabled="!groupNameDraft.trim()"
              @click="createGroupAndAssign"
            >
              Tạo nhóm & gán
            </VBtn>
          </div>
        </div>
      </VCard>
    </VDialog>

    <!-- 👉 Đẩy lên thegioiic -->
    <VDialog
      v-model="pushOpen"
      max-width="720"
      scrollable
    >
      <VCard class="pa-2">
        <div class="d-flex align-center pa-2">
          <span class="text-h6">Đẩy lên thegioiic</span>
          <VSpacer />
          <IconBtn @click="pushOpen = false">
            <VIcon icon="ri-close-line" />
          </IconBtn>
        </div>

        <div class="pa-3">
          <div class="d-flex flex-wrap align-center ga-3">
            <div>
              <div class="text-body-1 font-weight-medium">
                {{ pushScopeLabel }}
              </div>
              <div class="text-caption text-medium-emphasis">
                {{ pushMatchedItems.length }}/{{ pushItems.length }} dòng khớp sản phẩm ·
                Tạm tính {{ formatVnd(pushTotal) }}
              </div>
            </div>
            <VSpacer />
            <VChip
              v-if="tgConnected"
              size="small"
              color="success"
              variant="tonal"
            >
              thegioiic đã kết nối
            </VChip>
            <VBtn
              v-else
              size="small"
              color="primary"
              prepend-icon="ri-login-circle-line"
              @click="connectThegioiic"
            >
              Đăng nhập thegioiic
            </VBtn>
          </div>

          <VAlert
            v-if="!tgConnected"
            type="info"
            variant="tonal"
            density="compact"
            class="mt-3 text-caption"
          >
            App mở trang đăng nhập chính thức của thegioiic để bạn đăng nhập Google.
            App không lưu mật khẩu Google, chỉ giữ phiên đăng nhập trên máy này để thêm
            linh kiện vào giỏ hàng của bạn.
            <div class="mt-2">
              <VBtn
                size="x-small"
                variant="text"
                @click="openManualConnect"
              >
                Kết nối thủ công
              </VBtn>
            </div>
          </VAlert>

          <VAlert
            v-if="pushError"
            type="error"
            variant="tonal"
            density="compact"
            class="mt-3 text-caption"
          >
            {{ pushError }}
            <VBtn
              v-if="!tgConnected"
              size="x-small"
              variant="text"
              class="ms-2"
              @click="connectThegioiic"
            >
              Kết nối lại
            </VBtn>
          </VAlert>

          <VAlert
            v-if="pushResult"
            :type="pushResult.failed.length ? 'warning' : 'success'"
            variant="tonal"
            density="compact"
            class="mt-3 text-caption"
          >
            Đã thêm {{ pushResult.added }} dòng vào giỏ thegioiic.
            <template v-if="pushSkippedCount">
              Bỏ qua {{ pushSkippedCount }} dòng chưa khớp sản phẩm.
            </template>
            <template v-if="pushResult.failed.length">
              {{ pushResult.failed.length }} dòng lỗi:
              <span class="d-block mt-1">
                {{ pushResult.failed.slice(0, 4).map(item => item.error).join(' · ') }}
              </span>
            </template>
          </VAlert>

          <div
            v-if="pushMatchedItems.length"
            class="push-list mt-3"
          >
            <div
              v-for="item in pushMatchedItems"
              :key="item.id"
              class="push-row"
            >
              <div class="push-row-text">
                <div class="text-body-2 text-truncate">
                  {{ item.product?.name }}
                </div>
                <div class="text-caption text-medium-emphasis text-truncate">
                  {{ item.ref }} · SL {{ store.orderQty(item) }} ×
                  {{ formatVnd(item.product?.price ?? 0) }}
                </div>
              </div>
              <div class="text-body-2 font-weight-medium">
                {{ formatVnd((item.product?.price ?? 0) * store.orderQty(item)) }}
              </div>
            </div>
          </div>

          <div
            v-else
            class="text-caption text-medium-emphasis pa-3 text-center mt-3"
          >
            Không có dòng nào đã khớp sản phẩm thegioiic để đẩy lên.
          </div>

          <div class="text-caption text-medium-emphasis mt-2">
            Lưu ý: nếu sản phẩm đã có trong giỏ thegioiic, số lượng sẽ được cộng dồn.
          </div>

          <div class="d-flex flex-wrap align-center ga-2 mt-4">
            <VBtn
              v-if="pushResult && pushResult.added"
              variant="tonal"
              prepend-icon="ri-external-link-line"
              @click="openThegioiicCart"
            >
              Mở giỏ hàng trong app
            </VBtn>
            <VSpacer />
            <VBtn
              variant="text"
              @click="pushOpen = false"
            >
              Đóng
            </VBtn>
            <VBtn
              v-if="!pushResult || !pushResult.added"
              color="primary"
              prepend-icon="ri-shopping-cart-2-line"
              :loading="pushing"
              :disabled="!pushMatchedItems.length || !tgConnected"
              @click="pushToThegioiic"
            >
              Thêm vào giỏ
            </VBtn>
          </div>
        </div>
      </VCard>
    </VDialog>

    <!-- 👉 Kết nối thegioiic thủ công -->
    <VDialog
      v-model="manualOpen"
      max-width="560"
    >
      <VCard class="pa-2">
        <div class="d-flex align-center pa-2">
          <span class="text-h6">Kết nối thegioiic thủ công</span>
          <VSpacer />
          <IconBtn @click="manualOpen = false">
            <VIcon icon="ri-close-line" />
          </IconBtn>
        </div>

        <div class="pa-3">
          <VAlert
            type="info"
            variant="tonal"
            density="compact"
            class="text-caption"
          >
            Nếu đăng nhập Google trong app bị chặn, bạn có thể:
            <ol class="mt-2 ps-4">
              <li>Mở thegioiic.com trên trình duyệt và đăng nhập.</li>
              <li>Nhấn F12 → tab Console → dán lệnh sau rồi Enter:</li>
            </ol>
            <code class="d-block my-2">copy(localStorage.getItem('token'))</code>
            <div>Mã phiên đã được copy. Quay lại app và dán vào ô bên dưới.</div>
          </VAlert>

          <VTextField
            v-model="tokenDraft"
            label="Mã kết nối thegioiic"
            placeholder="Dán token vừa copy"
            variant="outlined"
            density="comfortable"
            hide-details
            type="password"
            class="mt-4"
            @keydown.enter="connectWithToken"
          />

          <VAlert
            v-if="connectError"
            type="error"
            variant="tonal"
            density="compact"
            class="mt-3 text-caption"
          >
            {{ connectError }}
          </VAlert>

          <div class="d-flex justify-end ga-2 mt-4">
            <VBtn
              variant="text"
              @click="manualOpen = false"
            >
              Huỷ
            </VBtn>
            <VBtn
              color="primary"
              :loading="connecting"
              :disabled="!tokenDraft.trim()"
              @click="connectWithToken"
            >
              Kết nối
            </VBtn>
          </div>
        </div>
      </VCard>
    </VDialog>

    <!-- 👉 JLC help -->
    <VDialog
      v-model="jlcHelpOpen"
      max-width="560"
    >
      <VCard class="pa-2">
        <div class="d-flex align-center pa-2">
          <span class="text-h6">Chưa có database JLCPCB</span>
          <VSpacer />
          <IconBtn @click="jlcHelpOpen = false">
            <VIcon icon="ri-close-line" />
          </IconBtn>
        </div>

        <div class="pa-3 text-body-2">
          <p>
            App đọc trực tiếp file cache của Konnect (read-only), không tải qua mạng:
          </p>
          <code class="d-block mb-3">{{ store.jlcStatus?.path || '%APPDATA%\\konnect\\jlcpcb.db' }}</code>
          <p>
            Nếu file chưa có, hãy dùng công cụ <b>Konnect</b> (MCP) chạy
            <code>download_jlcpcb_database</code> một lần — sau đó bấm
            <b>So sánh JLCPCB</b> lại.
          </p>
          <VAlert
            v-if="store.jlcStatus?.error"
            type="warning"
            variant="tonal"
            density="compact"
            class="mt-2 text-caption"
          >
            {{ store.jlcStatus.error }}
          </VAlert>
        </div>
      </VCard>
    </VDialog>

    <VSnackbar
      v-model="snackbar"
      :timeout="2600"
    >
      {{ snackText }}
    </VSnackbar>
  </div>
</template>

<style scoped>
.drop-zone {
  cursor: pointer;
  border: 2px dashed rgba(var(--v-theme-primary), 0.35);
  background: rgba(var(--v-theme-on-surface), 0.025);
  transition:
    border-color 0.15s ease,
    background 0.15s ease;
}

.drop-zone:hover,
.drop-zone.dragging {
  border-color: rgb(var(--v-theme-primary));
  background: rgba(var(--v-theme-primary), 0.06);
}

.kicad-hint {
  border: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
  background: rgba(var(--v-theme-on-surface), 0.03);
}

.kicad-cmd {
  display: block;
  font-size: 0.78rem;
  word-break: break-all;
  opacity: 0.85;
}

.bom-summary {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
  gap: 12px;
}

.bom-table-scroll {
  overflow-x: auto;
}

.bom-table {
  min-inline-size: 1260px;
  border: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
}

.bom-head {
  display: grid;
  grid-template-columns: 76px 58px 88px 86px minmax(0, 0.9fr) minmax(0, 1.5fr) 104px 156px;
  gap: 10px;
  padding: 10px 12px;
  font-size: 0.72rem;
  font-weight: 700;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: rgba(var(--v-theme-on-surface), 0.55);
}

.bom-select-all {
  display: flex;
  align-items: center;
  justify-content: center;
  margin-inline-start: -6px;
}

.group-panel {
  border: 1px solid rgba(var(--v-theme-primary), 0.28);
  background: rgba(var(--v-theme-primary), 0.05);
}

.group-select-bar {
  border: 1px solid rgba(var(--v-theme-primary), 0.35);
  background: rgba(var(--v-theme-primary), 0.07);
}

.push-list {
  max-block-size: 300px;
  overflow-y: auto;
  border: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
  border-radius: 8px;
}

.push-row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 12px;
}

.push-row + .push-row {
  border-block-start: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
}

.push-row-text {
  min-inline-size: 0;
  flex: 1;
}
</style>
