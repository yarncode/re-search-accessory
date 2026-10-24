<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { formatVnd, pickNativePath } from '@/api'
import { rememberSetting } from '@/settings'
import ProductDetail from '@/views/accessory/ProductDetail.vue'
import BomRow from '@/views/bom/BomRow.vue'
import { useBomStore } from '@/stores/bom'
import { detectColumns, parseBomFile } from '@/utils/bom'

const store = useBomStore()

const KICAD_CMD = 'kicad-cli sch export bom -o bom.csv --fields "Reference,Value,Footprint,${QUANTITY},DNP" board.kicad_sch'

const fileInput = ref<HTMLInputElement | null>(null)
const fileError = ref('')
const dragging = ref(false)
const filter = ref<'all' | 'ok' | 'review' | 'missing' | 'pending' | 'shortage'>('all')
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

const visibleItems = computed(() => {
  if (filter.value === 'all')
    return store.items

  if (filter.value === 'shortage')
    return store.items.filter(item => store.shortage(item))

  return store.items.filter(item => item.status === filter.value)
})

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
          Import từ KiCad / CSV / Excel, tự tìm linh kiện, tính theo số bo, đánh dấu mua hàng và lưu lại.
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
        @click="store.reset()"
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
              {{ store.counts.total }} dòng · tìm theo cột “{{ store.columns.search }}” ·
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
            Tạm tính ({{ store.boards }} bo)
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

      <!-- 👉 Lọc trạng thái -->
      <div class="d-flex flex-wrap ga-2 mb-3">
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

      <!-- 👉 Bảng BOM -->
      <div class="bom-table-scroll">
        <VCard
          class="bom-table"
          elevation="0"
        >
          <div class="bom-head">
            <span />
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
          />

          <div
            v-if="!visibleItems.length"
            class="pa-8 text-center text-medium-emphasis text-body-2"
          >
            Không có dòng nào ở trạng thái này.
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
  min-inline-size: 1180px;
  border: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
}

.bom-head {
  display: grid;
  grid-template-columns: 38px 58px 88px 86px minmax(0, 0.9fr) minmax(0, 1.5fr) 104px 124px;
  gap: 10px;
  padding: 10px 12px;
  font-size: 0.72rem;
  font-weight: 700;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: rgba(var(--v-theme-on-surface), 0.55);
}
</style>
