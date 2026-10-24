<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import * as THREE from 'three'
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js'
import { apiGet } from '@/api'
import type { FootprintResolved } from '@/types'
import {
  applyKiCadTransform,
  base64ToBytes,
  disposeObject,
  loadStep,
  loadWrl,
} from './modelLoader'

const props = defineProps<{
  footprint: string
  project?: string
}>()

const host = ref<HTMLDivElement | null>(null)
const resolved = ref<FootprintResolved | null>(null)
const loading = ref(false)
const error = ref('')
const mounted = ref(false)
const bbox = ref('')
const dims = ref<{ x: number; y: number; z: number } | null>(null)

let renderer: THREE.WebGLRenderer | null = null
let scene: THREE.Scene | null = null
let camera: THREE.PerspectiveCamera | null = null
let controls: OrbitControls | null = null
let frameId = 0
let root: THREE.Group | null = null
let orientation: THREE.Group | null = null
let axes: THREE.AxesHelper | null = null
let dimensionHelpers: THREE.Object3D[] = []
let observer: ResizeObserver | null = null

function ensureScene() {
  const element = host.value

  if (!element || renderer)
    return

  renderer = new THREE.WebGLRenderer({ antialias: true })
  renderer.setPixelRatio(Math.min(2, window.devicePixelRatio))
  renderer.setSize(element.clientWidth, Math.max(1, element.clientHeight))
  renderer.setClearColor(0x0b1220, 1)
  element.appendChild(renderer.domElement)

  scene = new THREE.Scene()
  camera = new THREE.PerspectiveCamera(45, element.clientWidth / Math.max(1, element.clientHeight), 0.01, 10000)
  camera.position.set(6, 4, 8)

  controls = new OrbitControls(camera, renderer.domElement)
  controls.enableDamping = true

  scene.add(new THREE.HemisphereLight(0xffffff, 0x223044, 1.2))

  const key = new THREE.DirectionalLight(0xffffff, 1.7)

  key.position.set(40, 60, 40)
  scene.add(key)

  const fill = new THREE.DirectionalLight(0x88aaff, 0.7)

  fill.position.set(-50, -30, -20)
  scene.add(fill)

  // Grid in mm, 1 cm cells: parts are usually a few mm across.
  const grid = new THREE.GridHelper(40, 40, 0x334155, 0x1e293b)

  scene.add(grid)

  const animate = () => {
    frameId = requestAnimationFrame(animate)
    controls?.update()

    if (renderer && scene && camera)
      renderer.render(scene, camera)
  }

  animate()
}

function resize() {
  const element = host.value

  if (!element || !renderer || !camera)
    return

  const width = Math.max(1, element.clientWidth)
  const height = Math.max(1, element.clientHeight)

  renderer.setSize(width, height)
  camera.aspect = width / height
  camera.updateProjectionMatrix()
}

function fit() {
  if (!root || !camera || !controls)
    return

  const box = new THREE.Box3().setFromObject(root)

  if (box.isEmpty())
    return

  const size = box.getSize(new THREE.Vector3())
  const center = box.getCenter(new THREE.Vector3())
  const maxDim = Math.max(size.x, size.y, size.z, 0.5)
  const fov = (camera.fov * Math.PI) / 180
  const distance = (maxDim / 2 / Math.tan(fov / 2)) * 2.1

  camera.near = Math.max(0.001, maxDim / 500)
  camera.far = maxDim * 500
  camera.position.set(center.x + distance * 0.7, center.y + distance * 0.55, center.z + distance)
  camera.updateProjectionMatrix()
  controls.target.copy(center)
  controls.update()
}

function measure() {
  if (!root) {
    bbox.value = ''
    dims.value = null

    return
  }

  // The orientation group may have just been rotated: refresh its world matrix
  // before measuring, otherwise the box lags one step behind.
  root.updateMatrixWorld(true)

  const box = new THREE.Box3().setFromObject(root)

  if (box.isEmpty()) {
    bbox.value = ''
    dims.value = null

    return
  }

  const size = box.getSize(new THREE.Vector3())

  dims.value = { x: size.x, y: size.y, z: size.z }
  bbox.value = `X ${size.x.toFixed(2)} · Y ${size.y.toFixed(2)} · Z ${size.z.toFixed(2)} mm`

  // Colour-coded world axes (X red, Y green, Z blue) at the bbox corner.
  if (scene && axes) {
    scene.remove(axes)
    axes.geometry.dispose()
    ;(axes.material as THREE.Material).dispose()
    axes = null
  }

  if (scene) {
    const span = Math.max(size.x, size.y, size.z, 1)

    axes = new THREE.AxesHelper(span * 0.45)
    axes.position.set(box.min.x, box.min.y, box.min.z)
    scene.add(axes)
  }

  buildDimensionBox(box)
}

function disposeHelper(object: THREE.Object3D) {
  object.traverse(child => {
    const mesh = child as THREE.Mesh

    mesh.geometry?.dispose()

    const material = mesh.material as THREE.Material & { map?: THREE.Texture | null }

    if (Array.isArray(material))
      material.forEach(item => { item.map?.dispose(); item.dispose() })
    else if (material) {
      material.map?.dispose()
      material.dispose()
    }
  })
}

function clearDimensionBox() {
  for (const helper of dimensionHelpers) {
    scene?.remove(helper)
    disposeHelper(helper)
  }

  dimensionHelpers = []
}

/** Wireframe box + three sprite labels (length along X, Y, Z). */
function buildDimensionBox(box: THREE.Box3) {
  clearDimensionBox()

  if (!scene)
    return

  const size = box.getSize(new THREE.Vector3())
  const span = Math.max(size.x, size.y, size.z, 1)
  const margin = span * 0.08

  const wire = new THREE.Box3Helper(box.clone(), new THREE.Color('#8ab4f8'))
  const wireMaterial = wire.material as THREE.LineBasicMaterial

  wireMaterial.transparent = true
  wireMaterial.opacity = 0.6
  wire.renderOrder = 900
  scene.add(wire)
  dimensionHelpers.push(wire)

  const label = (text: string, color: string, position: THREE.Vector3) => {
    const canvas = document.createElement('canvas')

    canvas.width = 512
    canvas.height = 128

    const ctx = canvas.getContext('2d')

    if (!ctx)
      return

    ctx.fillStyle = 'rgba(11, 18, 32, 0.82)'
    ctx.beginPath()
    ctx.roundRect(4, 4, 504, 120, 24)
    ctx.fill()
    ctx.strokeStyle = color
    ctx.lineWidth = 3
    ctx.stroke()
    ctx.fillStyle = color
    ctx.font = 'bold 60px "Segoe UI", system-ui, sans-serif'
    ctx.textAlign = 'center'
    ctx.textBaseline = 'middle'
    ctx.fillText(text, 256, 68)

    const texture = new THREE.CanvasTexture(canvas)

    texture.colorSpace = THREE.SRGBColorSpace

    const sprite = new THREE.Sprite(new THREE.SpriteMaterial({
      map: texture,
      transparent: true,
      depthTest: false,
      depthWrite: false,
      toneMapped: false,
    }))

    const height = span * 0.11

    sprite.scale.set(height * 4.2, height, 1)
    sprite.position.copy(position)
    sprite.renderOrder = 1000
    scene.add(sprite)
    dimensionHelpers.push(sprite)
  }

  const mid = box.getCenter(new THREE.Vector3())

  label(
    `X ${(size.x).toFixed(2)} mm`,
    '#ff6b6b',
    new THREE.Vector3(mid.x, box.max.y + margin, box.max.z + margin),
  )
  label(
    `Y ${(size.y).toFixed(2)} mm`,
    '#51cf66',
    new THREE.Vector3(box.max.x + margin, mid.y, box.max.z + margin),
  )
  label(
    `Z ${(size.z).toFixed(2)} mm`,
    '#4dabf7',
    new THREE.Vector3(box.max.x + margin, box.max.y + margin, mid.z),
  )
}

/** Rotate/flip the component itself, not the camera, so the grid stays put.
 *  The bounding box and the mm numbers are re-measured afterwards: they describe
 *  the part in its current orientation. */
function rotate(axis: 'x' | 'y' | 'z', degrees: number) {
  if (!orientation)
    return

  orientation.rotation[axis] += THREE.MathUtils.degToRad(degrees)
  measure()
}

function flip() {
  rotate('x', 180)
}

function resetOrientation() {
  orientation?.rotation.set(0, 0, 0)
  measure()
}

async function load() {
  const footprint = props.footprint.trim()

  if (!footprint)
    return

  loading.value = true
  error.value = ''
  resolved.value = null
  bbox.value = ''
  dims.value = null
  clearDimensionBox()

  try {
    const data = await apiGet<FootprintResolved>('/footprint3d', {
      fp: footprint,
      project: props.project?.trim() || undefined,
    })

    resolved.value = data

    if (!data.found)
      throw new Error(data.error || 'Không tìm thấy footprint')

    if (!data.models.length)
      throw new Error('Footprint không có model 3D')

    ensureScene()

    if (!scene)
      throw new Error('Không khởi tạo được WebGL')

    if (root) {
      scene.remove(root)
      disposeObject(root)
      root = null
      orientation = null
    }

    const group = new THREE.Group()
    let added = 0

    for (const entry of data.models) {
      if (!entry.data)
        continue

      const bytes = base64ToBytes(entry.data)
      let object: THREE.Object3D | null = null

      if (entry.format === 'step')
        object = await loadStep(bytes)
      else if (entry.format === 'wrl')
        object = loadWrl(bytes)

      if (!object)
        continue

      // KiCad reads VRML geometry in its legacy 2.54 mm unit; the model entry's
      // scale usually carries the matching 1/2.54 factor.
      if (entry.format === 'wrl')
        object.scale.multiplyScalar(2.54)

      // Model files are Z-up (KiCad's frame); the three.js scene is Y-up.
      const upright = new THREE.Group()

      upright.rotation.x = -Math.PI / 2
      upright.add(applyKiCadTransform(object, entry))
      group.add(upright)
      added += 1
    }

    if (!added)
      throw new Error('Không dựng được model 3D (file thiếu hoặc định dạng lạ)')

    // The orientation group carries user rotate/flip, outside the KiCad model
    // transform, so "lật" never changes the measured dimensions.
    const orient = new THREE.Group()

    orient.add(group)
    scene.add(orient)
    root = orient
    orientation = orient
    fit()
    measure()
  }
  catch (err) {
    error.value = (err as Error).message
  }
  finally {
    loading.value = false
  }
}

watch(
  () => [props.footprint, props.project],
  () => {
    if (mounted.value)
      void load()
  },
)

onMounted(() => {
  mounted.value = true
  ensureScene()
  resize()
  void load()

  if (host.value && typeof ResizeObserver !== 'undefined') {
    observer = new ResizeObserver(() => resize())
    observer.observe(host.value)
  }
})

onBeforeUnmount(() => {
  cancelAnimationFrame(frameId)
  observer?.disconnect()

  clearDimensionBox()

  if (root) {
    disposeObject(root)
    root = null
  }

  controls?.dispose()

  if (renderer) {
    renderer.dispose()
    renderer.domElement.remove()
    renderer = null
  }
})

const missingCount = computed(() => (resolved.value?.models ?? []).filter(entry => !entry.exists).length)

function formatSize(bytes: number): string {
  if (bytes >= 1024 * 1024)
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`

  return `${Math.max(1, Math.round(bytes / 1024))} KB`
}
</script>

<template>
  <div>
    <div class="d-flex flex-wrap align-center ga-2 mb-2">
      <VIcon
        icon="ri-box-3-line"
        color="primary"
        size="18"
      />
      <span class="text-body-2 font-weight-medium">{{ footprint }}</span>
      <VSpacer />
      <span
        v-if="bbox"
        class="text-caption text-medium-emphasis"
        title="Kích thước bao của model (X × Z × Y, mm)"
      >
        {{ bbox }}
      </span>
      <span class="text-caption text-medium-emphasis">
        {{ resolved?.models.length ?? 0 }} model<template v-if="missingCount"> · {{ missingCount }} thiếu</template>
      </span>
      <VBtn
        size="small"
        variant="text"
        icon="ri-anticlockwise-line"
        title="Xoay trái 90°"
        :disabled="!resolved?.found"
        @click="rotate('y', -90)"
      />
      <VBtn
        size="small"
        variant="text"
        icon="ri-clockwise-line"
        title="Xoay phải 90°"
        :disabled="!resolved?.found"
        @click="rotate('y', 90)"
      />
      <VBtn
        size="small"
        variant="text"
        icon="ri-arrow-up-down-line"
        title="Nghiêng 90°"
        :disabled="!resolved?.found"
        @click="rotate('x', 90)"
      />
      <VBtn
        size="small"
        variant="text"
        icon="ri-refresh-line"
        title="Lật 180° (xem mặt dưới)"
        :disabled="!resolved?.found"
        @click="flip"
      />
      <VBtn
        size="small"
        variant="text"
        icon="ri-restart-line"
        title="Về hướng ban đầu"
        :disabled="!resolved?.found"
        @click="resetOrientation"
      />
      <VBtn
        size="small"
        variant="text"
        prepend-icon="ri-focus-3-line"
        :disabled="!resolved?.found"
        @click="fit"
      >
        Fit
      </VBtn>
    </div>

    <div class="fp-layout">
      <aside class="fp-meta">
        <div
          v-if="resolved?.footprint_path"
          class="text-caption text-medium-emphasis mb-2 text-truncate"
          :title="resolved.footprint_path"
        >
          {{ resolved.footprint_path }}
        </div>

        <div
          v-for="(entry, index) in resolved?.models ?? []"
          :key="`${entry.path}-${index}`"
          class="fp-model"
        >
          <VIcon
            :icon="entry.exists ? 'ri-checkbox-circle-fill' : 'ri-error-warning-line'"
            :color="entry.exists ? 'success' : 'warning'"
            size="15"
          />
          <div class="fp-model-text">
            <div class="text-caption font-weight-medium text-uppercase">
              {{ entry.format }} · {{ formatSize(entry.size) }}
            </div>
            <div
              class="text-caption text-medium-emphasis text-truncate"
              :title="entry.path"
            >
              {{ entry.path }}
            </div>
          </div>
        </div>

        <div
          v-if="resolved && !resolved.models.length"
          class="text-caption text-medium-emphasis"
        >
          Footprint không có model 3D.
        </div>

        <div
          v-if="dims"
          class="fp-dims"
        >
          <div class="text-caption font-weight-medium mb-1">
            Kích thước model (trục 3D)
          </div>
          <div class="fp-dim-row">
            <span><i class="axis-dot axis-x" />X (rộng)</span><b>{{ dims.x.toFixed(2) }} mm</b>
          </div>
          <div class="fp-dim-row">
            <span><i class="axis-dot axis-y" />Y (cao)</span><b>{{ dims.y.toFixed(2) }} mm</b>
          </div>
          <div class="fp-dim-row">
            <span><i class="axis-dot axis-z" />Z (sâu)</span><b>{{ dims.z.toFixed(2) }} mm</b>
          </div>
        </div>
      </aside>

      <div class="fp-canvas-col">
        <div
          ref="host"
          class="fp-viewport"
        />

        <div
          v-if="loading || error || !resolved"
          class="fp-overlay"
        >
          <template v-if="loading">
            <VProgressCircular
              indeterminate
              color="primary"
            />
            <span class="text-caption text-medium-emphasis mt-2">Đang dựng 3D…</span>
          </template>
          <template v-else-if="error">
            <VIcon
              icon="ri-error-warning-line"
              color="warning"
              size="40"
            />
            <div class="text-body-2 mt-2">
              {{ error }}
            </div>
            <ul class="text-caption text-medium-emphasis mt-2 searched">
              <li
                v-for="line in resolved?.searched ?? []"
                :key="line"
              >
                {{ line }}
              </li>
            </ul>
          </template>
        </div>

        <div class="text-caption text-medium-emphasis mt-2">
          Trục 3D:
          <span class="axis-x">X</span> ·
          <span class="axis-y">Y (cao)</span> ·
          <span class="axis-z">Z</span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.fp-layout {
  display: grid;
  grid-template-columns: 260px minmax(0, 1fr);
  gap: 12px;
}

.fp-meta {
  min-inline-size: 0;
  max-block-size: 62vh;
  overflow-y: auto;
}

.fp-model {
  display: flex;
  gap: 6px;
  align-items: flex-start;
  padding-block: 3px;
}

.fp-model-text {
  min-inline-size: 0;
}

.fp-dims {
  margin-block-start: 14px;
  padding-block-start: 10px;
  border-block-start: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
}

.fp-dim-row {
  display: flex;
  justify-content: space-between;
  gap: 10px;
  font-size: 0.78rem;
  color: rgba(var(--v-theme-on-surface), var(--v-medium-emphasis-opacity));
}

.fp-dim-row span {
  display: inline-flex;
  gap: 6px;
  align-items: center;
}

.axis-dot {
  display: inline-block;
  inline-size: 8px;
  block-size: 8px;
  border-radius: 50%;
}

.axis-x {
  background: #ef4444;
}

.axis-y {
  background: #22c55e;
}

.axis-z {
  background: #3b82f6;
}

.fp-dim-row b {
  color: rgb(var(--v-theme-on-surface));
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}

.fp-canvas-col {
  position: relative;
  min-inline-size: 0;
}

.fp-viewport {
  block-size: 62vh;
  border-radius: 10px;
  overflow: hidden;
  background-color: #0b1220;
  background-image:
    linear-gradient(rgba(255, 255, 255, 0.045) 1px, transparent 1px),
    linear-gradient(90deg, rgba(255, 255, 255, 0.045) 1px, transparent 1px);
  background-size: 24px 24px;
}

.fp-overlay {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 2px;
  padding: 24px;
  text-align: center;
  color: rgb(var(--v-theme-on-surface));
  background: rgba(250, 250, 252, 0.94);
}

.searched {
  padding-inline-start: 18px;
  word-break: break-all;
  max-inline-size: 640px;
}

@media (max-width: 900px) {
  .fp-layout {
    grid-template-columns: 1fr;
  }

  .fp-meta {
    max-block-size: none;
  }

  .fp-viewport {
    block-size: 50vh;
  }
}
</style>
