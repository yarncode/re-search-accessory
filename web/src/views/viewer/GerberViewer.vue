<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { unzipSync } from 'fflate'
import { pickNativePath, readLocalFiles } from '@/api'
import { rememberSetting } from '@/settings'
import { loadScript } from '@/utils/loadScript'
import { GerberScene } from './gerberScene'

type LayerInfo = {
  id: string
  file: string
  kind: string
  color: string
  order: number
  alpha: number
  svg: string
  viewBox: number[]
  bytes: number
  visible: boolean
}

type RawFile = { name: string; bytes: Uint8Array }

const LAYER_STYLE: Record<string, { label: string; color: string; order: number; alpha: number; on: boolean }> = {
  F_Cu: { label: 'F.Cu', color: '#ff5c62', order: 10, alpha: 0.9, on: true },
  In1_Cu: { label: 'In1.Cu', color: '#f7b955', order: 11, alpha: 0.8, on: false },
  In2_Cu: { label: 'In2.Cu', color: '#7ccf6a', order: 12, alpha: 0.8, on: false },
  B_Cu: { label: 'B.Cu', color: '#4d8dff', order: 13, alpha: 0.9, on: true },
  F_Mask: { label: 'F.Mask', color: '#c084fc', order: 20, alpha: 0.35, on: false },
  B_Mask: { label: 'B.Mask', color: '#a78bfa', order: 21, alpha: 0.35, on: false },
  F_Paste: { label: 'F.Paste', color: '#cbd5e1', order: 22, alpha: 0.5, on: false },
  B_Paste: { label: 'B.Paste', color: '#94a3b8', order: 23, alpha: 0.5, on: false },
  F_Silkscreen: { label: 'F.Silk', color: '#f8fafc', order: 30, alpha: 0.95, on: true },
  B_Silkscreen: { label: 'B.Silk', color: '#f9a8d4', order: 31, alpha: 0.85, on: false },
  Edge_Cuts: { label: 'Edge.Cuts', color: '#facc15', order: 40, alpha: 1, on: true },
  PTH: { label: 'PTH drill', color: '#7dd3fc', order: 50, alpha: 1, on: true },
  NPTH: { label: 'NPTH drill', color: '#67e8f9', order: 51, alpha: 1, on: true },
  Other: { label: 'Khác', color: '#e2e8f0', order: 60, alpha: 0.8, on: false },
}

const layers = ref<LayerInfo[]>([])
const loading = ref(false)
const rendering = ref(false)
const error = ref('')
const dragging = ref(false)
const fileInput = ref<HTMLInputElement | null>(null)
const viewport = ref<HTMLDivElement | null>(null)
const revision = ref(0)
const dirOpen = ref(false)
const dirLoading = ref(false)
const dirDraft = ref(localStorage.getItem('res-gerber-dir') || '')

let scene: GerberScene | null = null
let dragStart: { x: number; y: number } | null = null

const GERBER_EXT = /\.(gbr|gtl|gbl|gto|gbo|gts|gbs|gko|gm1|gtp|gbp|drl|xln)$/i

function isGerberName(name: string): boolean {
  const base = name.split('/').pop() ?? name

  if (base.toLowerCase().endsWith('.gbrjob'))
    return false

  return GERBER_EXT.test(base) || /(^|[-_])(F|B|In\d+)_(Cu|Mask|Paste|Silkscreen)|Edge_Cuts|(^|-)PTH|NPTH/i.test(base)
}

function detectKind(name: string): string {
  const text = name.toLowerCase()

  if (/npth/.test(text))
    return 'NPTH'
  if (/pth/.test(text) || /\.drl$|\.xln$/.test(text))
    return 'PTH'
  if (/f[._-]cu|\.gtl$/.test(text))
    return 'F_Cu'
  if (/b[._-]cu|\.gbl$/.test(text))
    return 'B_Cu'
  if (/in1[._-]cu/.test(text))
    return 'In1_Cu'
  if (/in2[._-]cu/.test(text))
    return 'In2_Cu'
  if (/f[._-]silkscreen|f[._-]silk|\.gto$/.test(text))
    return 'F_Silkscreen'
  if (/b[._-]silkscreen|b[._-]silk|\.gbo$/.test(text))
    return 'B_Silkscreen'
  if (/f[._-]mask|\.gts$/.test(text))
    return 'F_Mask'
  if (/b[._-]mask|\.gbs$/.test(text))
    return 'B_Mask'
  if (/f[._-]paste|\.gtp$/.test(text))
    return 'F_Paste'
  if (/b[._-]paste|\.gbp$/.test(text))
    return 'B_Paste'
  if (/edge[._-]cuts|\.gko$|\.gm1$/.test(text))
    return 'Edge_Cuts'

  return 'Other'
}

function decode(bytes: Uint8Array): string {
  return new TextDecoder('latin1').decode(bytes)
}

type RenderResult =
  | { ok: true; svg: string; viewBox: number[] }
  | { ok: false; error: string }

type GerberConverter = { viewBox?: number[] }
type GerberToSvg = (
  input: string,
  options: Record<string, unknown>,
  done: (err: Error | null, svg: string) => void,
) => GerberConverter

let gerberLib: GerberToSvg | null = null

/** The npm entry point is Node-only; its dist bundle is a classic script that
 *  defines `window.gerberToSvg`. Load it lazily, once. */
async function gerberToSvg(): Promise<GerberToSvg> {
  if (gerberLib)
    return gerberLib

  await loadScript('/vendor/gerber-to-svg.min.js')

  const loaded = (window as unknown as { gerberToSvg?: GerberToSvg }).gerberToSvg

  if (!loaded)
    throw new Error('không nạp được gerber-to-svg')

  gerberLib = loaded

  return loaded
}

async function renderOne(name: string, text: string, color: string): Promise<RenderResult> {
  let render: GerberToSvg

  try {
    render = await gerberToSvg()
  }
  catch (err) {
    return { ok: false, error: (err as Error).message }
  }

  return new Promise(resolve => {
    let converter: GerberConverter | null = null

    try {
      converter = render(
        text,
        { attributes: { color, width: '100%', height: '100%' }, id: name },
        (err: Error | null, svg: string) => {
          if (err || !svg || !converter?.viewBox)
            resolve({ ok: false, error: String(err ?? 'không tạo được SVG') })
          else
            resolve({ ok: true, svg, viewBox: [...converter.viewBox] })
        },
      )
    }
    catch (err) {
      resolve({ ok: false, error: (err as Error).message })
    }
  })
}

async function collect(files: File[]): Promise<RawFile[]> {
  const raw: RawFile[] = []

  for (const file of files) {
    if (/\.zip$/i.test(file.name)) {
      try {
        const zip = unzipSync(new Uint8Array(await file.arrayBuffer()))

        for (const [name, bytes] of Object.entries(zip)) {
          if (!name.endsWith('/') && isGerberName(name))
            raw.push({ name: name.split('/').pop() ?? name, bytes })
        }
      }
      catch {
        error.value = `Không đọc được zip: ${file.name}`
      }
    }
    else if (isGerberName(file.name)) {
      raw.push({ name: file.name, bytes: new Uint8Array(await file.arrayBuffer()) })
    }
  }

  return raw
}

async function loadRaw(raw: RawFile[]) {
  if (!raw.length) {
    error.value = 'Không tìm thấy file Gerber/Drill nào trong lựa chọn.'

    return
  }

  loading.value = true
  error.value = ''

  try {
    const next: LayerInfo[] = []

    for (const entry of raw) {
      const kind = detectKind(entry.name)
      const style = LAYER_STYLE[kind] ?? LAYER_STYLE.Other
      const rendered = await renderOne(entry.name, decode(entry.bytes), style.color)

      if (!rendered.ok) {
        error.value = `Không render được ${entry.name}: ${rendered.error}`

        continue
      }

      next.push({
        id: `${entry.name}-${next.length}`,
        file: entry.name,
        kind,
        color: style.color,
        order: style.order,
        alpha: style.alpha,
        svg: rendered.svg,
        viewBox: rendered.viewBox,
        bytes: entry.bytes.length,
        visible: style.on,
      })
    }

    next.sort((a, b) => a.order - b.order)
    layers.value = next
    revision.value += 1
  }
  finally {
    loading.value = false
  }
}

async function addFiles(files: File[]) {
  if (!files.length)
    return

  const raw = await collect(files)

  await loadRaw(raw)
}

async function pickDir() {
  const path = await pickNativePath('folder', 'Chọn thư mục gerber')

  if (!path)
    return

  dirDraft.value = path
  rememberSetting('gerber_dir', path)
  await openDir()
}

async function openDir() {
  const path = dirDraft.value.trim()

  if (!path)
    return

  dirLoading.value = true
  error.value = ''

  try {
    const files = await readLocalFiles(path, 'gerber')

    localStorage.setItem('res-gerber-dir', path)
    rememberSetting('gerber_dir', path)
    dirOpen.value = false
    await loadRaw(files.map(file => ({ name: file.name, bytes: file.bytes })))
  }
  catch (err) {
    error.value = (err as Error).message
  }
  finally {
    dirLoading.value = false
  }
}

async function onFile(event: Event) {
  const input = event.target as HTMLInputElement
  const files = Array.from(input.files ?? [])

  input.value = ''

  await addFiles(files)
}

async function onDrop(event: DragEvent) {
  dragging.value = false
  await addFiles(Array.from(event.dataTransfer?.files ?? []))
}

/** Union of ALL layers, so hiding a layer never moves the view. */
const unionAll = computed<number[] | null>(() => {
  if (!layers.value.length)
    return null

  const minX = Math.min(...layers.value.map(layer => layer.viewBox[0]))
  const minY = Math.min(...layers.value.map(layer => layer.viewBox[1]))
  const maxX = Math.max(...layers.value.map(layer => layer.viewBox[0] + layer.viewBox[2]))
  const maxY = Math.max(...layers.value.map(layer => layer.viewBox[1] + layer.viewBox[3]))

  return [minX, minY, maxX - minX, maxY - minY]
})

const boardSize = computed(() => {
  const box = unionAll.value

  if (!box)
    return ''

  return `${(box[2] / 1000).toFixed(2)} × ${(box[3] / 1000).toFixed(2)} mm`
})

function toSceneLayers() {
  return layers.value.map(layer => ({
    id: layer.id,
    svg: layer.svg,
    alpha: layer.alpha,
    order: layer.order,
    visible: layer.visible,
  }))
}

async function rebuildScene() {
  await nextTick()

  if (!viewport.value || !unionAll.value) {
    scene?.dispose()
    scene = null

    return
  }

  scene?.dispose()
  scene = new GerberScene(viewport.value)
  rendering.value = true

  try {
    await scene.setLayers(toSceneLayers(), unionAll.value)
  }
  finally {
    rendering.value = false
  }
}

watch(revision, () => void rebuildScene())

watch(layers, () => {
  if (!layers.value.length) {
    scene?.dispose()
    scene = null

    return
  }

  for (const layer of layers.value)
    scene?.setVisible(layer.id, layer.visible)
}, { deep: true })

function toggleAll(value: boolean) {
  layers.value = layers.value.map(layer => ({ ...layer, visible: value }))
}

function onWheel(event: WheelEvent) {
  event.preventDefault()
  scene?.zoomAt(event.deltaY < 0 ? 1.15 : 1 / 1.15, event.clientX, event.clientY)
}

function onPointerDown(event: PointerEvent) {
  dragStart = { x: event.clientX, y: event.clientY }
  ;(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId)
}

function onPointerMove(event: PointerEvent) {
  if (!dragStart)
    return

  scene?.panBy(event.clientX - dragStart.x, event.clientY - dragStart.y)
  dragStart = { x: event.clientX, y: event.clientY }
}

function onPointerUp() {
  dragStart = null
}

onBeforeUnmount(() => {
  scene?.dispose()
  scene = null
})

function clearAll() {
  layers.value = []
  error.value = ''
  revision.value += 1
}
</script>

<template>
  <div>
    <div class="d-flex flex-wrap align-center ga-2 mb-3">
      <VBtn
        variant="tonal"
        prepend-icon="ri-upload-2-line"
        @click="fileInput?.click()"
      >
        Chọn file / zip
      </VBtn>
      <VBtn
        variant="tonal"
        prepend-icon="ri-folder-open-line"
        @click="dirOpen = true"
      >
        Mở thư mục
      </VBtn>
      <VBtn
        v-if="layers.length"
        variant="text"
        prepend-icon="ri-delete-bin-6-line"
        color="error"
        @click="clearAll"
      >
        Xoá
      </VBtn>
      <VSpacer />
      <span
        v-if="layers.length"
        class="text-caption text-medium-emphasis"
      >
        {{ layers.length }} layer · {{ boardSize }}
      </span>
      <VBtn
        v-if="layers.length"
        size="small"
        variant="text"
        prepend-icon="ri-zoom-out-line"
        @click="scene?.zoomBy(1 / 1.25)"
      />
      <VBtn
        v-if="layers.length"
        size="small"
        variant="text"
        prepend-icon="ri-zoom-in-line"
        @click="scene?.zoomBy(1.25)"
      />
      <VBtn
        v-if="layers.length"
        size="small"
        variant="text"
        prepend-icon="ri-restart-line"
        @click="scene?.fit()"
      >
        Fit
      </VBtn>

      <input
        ref="fileInput"
        type="file"
        multiple
        accept=".gbr,.gtl,.gbl,.gto,.gbo,.gts,.gbs,.gko,.gm1,.gtp,.gbp,.drl,.xln,.zip"
        hidden
        @change="onFile"
      >
    </div>

    <VAlert
      v-if="error"
      type="warning"
      variant="tonal"
      density="compact"
      class="mb-3"
      closable
      @click:close="error = ''"
    >
      {{ error }}
    </VAlert>

    <div
      v-if="!layers.length"
      class="drop-zone pa-10 text-center"
      :class="{ dragging }"
      @click="fileInput?.click()"
      @dragover.prevent="dragging = true"
      @dragleave.prevent="dragging = false"
      @drop.prevent="onDrop"
    >
      <VProgressCircular
        v-if="loading"
        indeterminate
        color="primary"
      />
      <template v-else>
        <VIcon
          icon="ri-layout-masonry-line"
          size="56"
          color="primary"
        />
        <div class="text-h6 font-weight-bold mt-3">
          Kéo Gerber / Drill hoặc file .zip vào đây
        </div>
        <div class="text-body-2 text-medium-emphasis mt-1">
          Hỗ trợ .gbr, .gtl/.gbl, .gto/.gbo, .gts/.gbs, .gko, .drl, .xln, .zip — hoặc dùng
          “Mở thư mục” để trỏ tới thư mục gerber trên máy.
        </div>
      </template>
    </div>

    <VRow v-else>
      <VCol cols="12" md="3" lg="2">
        <VCard
          class="pa-3"
          elevation="0"
        >
          <div class="d-flex align-center mb-2">
            <span class="section-label flex-grow-1">Layers</span>
            <VBtn
              size="x-small"
              variant="text"
              @click="toggleAll(true)"
            >
              All
            </VBtn>
            <VBtn
              size="x-small"
              variant="text"
              @click="toggleAll(false)"
            >
              None
            </VBtn>
          </div>

          <div
            v-for="layer in layers"
            :key="layer.id"
            class="layer-row d-flex align-center ga-2"
          >
            <VCheckbox
              v-model="layer.visible"
              density="compact"
              hide-details
              class="flex-grow-0"
            />
            <span
              class="layer-dot"
              :style="{ background: layer.color }"
            />
            <div class="min-width-0 flex-grow-1">
              <div class="text-caption font-weight-medium">
                {{ LAYER_STYLE[layer.kind]?.label ?? layer.kind }}
              </div>
              <div
                class="text-caption text-medium-emphasis text-truncate"
                :title="layer.file"
              >
                {{ layer.file }}
              </div>
            </div>
          </div>
        </VCard>
      </VCol>

      <VCol cols="12" md="9" lg="10">
        <div
          ref="viewport"
          class="viewport"
          @wheel="onWheel"
          @pointerdown="onPointerDown"
          @pointermove="onPointerMove"
          @pointerup="onPointerUp"
          @pointercancel="onPointerUp"
          @dblclick="scene?.fit()"
        >
          <div
            v-if="rendering"
            class="raster-indicator"
          >
            <VProgressCircular
              indeterminate
              size="20"
              width="2"
              color="primary"
            />
            <span>Đang dựng texture…</span>
          </div>
        </div>
        <div class="text-caption text-medium-emphasis mt-2">
          Kéo để di chuyển · lăn chuột để zoom · double-click để reset. Layer được dựng bằng WebGL (GPU).
        </div>
      </VCol>
    </VRow>

    <VDialog
      v-model="dirOpen"
      max-width="560"
    >
      <VCard class="pa-2">
        <div class="d-flex align-center pa-2">
          <span class="text-h6">Mở thư mục gerber trên máy</span>
          <VSpacer />
          <IconBtn @click="dirOpen = false">
            <VIcon icon="ri-close-line" />
          </IconBtn>
        </div>

        <div class="pa-3">
          <div class="d-flex align-center ga-2">
            <VTextField
              v-model="dirDraft"
              label="Thư mục gerber"
              placeholder="Bấm nút bên phải để chọn thư mục"
              variant="outlined"
              density="comfortable"
              hide-details
              readonly
              @keydown.enter="openDir"
            />
            <VBtn
              variant="tonal"
              icon="ri-folder-open-line"
              title="Chọn thư mục gerber"
              :loading="dirLoading"
              @click="pickDir"
            />
          </div>
          <div class="text-caption text-medium-emphasis mt-2">
            App đọc trực tiếp trên máy qua backend loopback, không upload file đi đâu.
          </div>
          <div class="d-flex justify-end ga-2 mt-4">
            <VBtn
              variant="text"
              @click="dirOpen = false"
            >
              Huỷ
            </VBtn>
            <VBtn
              color="primary"
              :loading="dirLoading"
              @click="openDir"
            >
              Mở
            </VBtn>
          </div>
        </div>
      </VCard>
    </VDialog>
  </div>
</template>

<style scoped>
.drop-zone {
  cursor: pointer;
  border: 2px dashed rgba(var(--v-theme-primary), 0.35);
  background: rgba(var(--v-theme-on-surface), 0.025);
}

.drop-zone:hover,
.drop-zone.dragging {
  border-color: rgb(var(--v-theme-primary));
  background: rgba(var(--v-theme-primary), 0.06);
}

.section-label {
  font-size: 0.72rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  opacity: 0.6;
}

.layer-row + .layer-row {
  margin-block-start: 2px;
}

.layer-dot {
  inline-size: 10px;
  block-size: 10px;
  border-radius: 50%;
  flex: 0 0 auto;
}

.min-width-0 {
  min-inline-size: 0;
}

.viewport {
  position: relative;
  block-size: 72vh;
  overflow: hidden;
  border-radius: 10px;
  cursor: grab;
  background-color: #0b1220;
  background-image:
    linear-gradient(rgba(255, 255, 255, 0.045) 1px, transparent 1px),
    linear-gradient(90deg, rgba(255, 255, 255, 0.045) 1px, transparent 1px);
  background-size: 24px 24px;
  touch-action: none;
}

.viewport:active {
  cursor: grabbing;
}

.raster-indicator {
  position: absolute;
  inset-block-start: 10px;
  inset-inline-end: 12px;
  display: flex;
  gap: 8px;
  align-items: center;
  padding: 6px 10px;
  border-radius: 8px;
  color: #e2e8f0;
  font-size: 0.75rem;
  background: rgba(11, 18, 32, 0.75);
  pointer-events: none;
}
</style>
