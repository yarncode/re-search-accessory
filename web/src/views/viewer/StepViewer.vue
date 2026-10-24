<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import * as THREE from 'three'
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js'
import occtimportjs from 'occt-import-js'
import wasmUrl from 'occt-import-js/dist/occt-import-js.wasm?url'
import { readLocalFiles, pickNativePath } from '@/api'

const loading = ref(false)
const error = ref('')
const stats = ref('')
const dragging = ref(false)
const host = ref<HTMLDivElement | null>(null)
const fileInput = ref<HTMLInputElement | null>(null)
const modelName = ref('')
const pathOpen = ref(false)
const pathLoading = ref(false)
const pathDraft = ref(localStorage.getItem('res-step-path') || '')

let renderer: THREE.WebGLRenderer | null = null
let scene: THREE.Scene | null = null
let camera: THREE.PerspectiveCamera | null = null
let controls: OrbitControls | null = null
let frameId = 0
let current: THREE.Group | null = null
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
  camera = new THREE.PerspectiveCamera(
    45,
    element.clientWidth / Math.max(1, element.clientHeight),
    0.1,
    100000,
  )
  camera.position.set(60, 45, 80)

  controls = new OrbitControls(camera, renderer.domElement)
  controls.enableDamping = true
  controls.dampingFactor = 0.08

  scene.add(new THREE.HemisphereLight(0xffffff, 0x223044, 1.2))

  const key = new THREE.DirectionalLight(0xffffff, 1.7)

  key.position.set(60, 100, 50)
  scene.add(key)

  const fill = new THREE.DirectionalLight(0x88aaff, 0.7)

  fill.position.set(-70, -40, -30)
  scene.add(fill)

  const grid = new THREE.GridHelper(200, 20, 0x334155, 0x1e293b)

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

function disposeGroup(group: THREE.Group) {
  group.traverse(child => {
    if (child instanceof THREE.Mesh) {
      child.geometry.dispose()

      const material = child.material

      if (Array.isArray(material))
        material.forEach(entry => entry.dispose())
      else
        material.dispose()
    }
  })
}

function frameCurrent() {
  if (!current || !camera || !controls)
    return

  const box = new THREE.Box3().setFromObject(current)

  if (box.isEmpty())
    return

  const size = box.getSize(new THREE.Vector3())
  const center = box.getCenter(new THREE.Vector3())
  const maxDim = Math.max(size.x, size.y, size.z) || 1
  const fov = (camera.fov * Math.PI) / 180
  const distance = (maxDim / 2 / Math.tan(fov / 2)) * 1.7

  camera.near = Math.max(0.01, maxDim / 500)
  camera.far = maxDim * 200
  camera.position.set(
    center.x + distance * 0.8,
    center.y + distance * 0.55,
    center.z + distance,
  )
  camera.updateProjectionMatrix()
  controls.target.copy(center)
  controls.update()
}

async function loadStep(buffer: ArrayBuffer) {
  ensureScene()

  if (!scene) {
    error.value = 'Không khởi tạo được WebGL.'

    return
  }

  loading.value = true
  error.value = ''
  stats.value = ''

  try {
    const occt = await occtimportjs({ locateFile: () => wasmUrl })
    const result = occt.ReadStepFile(new Uint8Array(buffer), null)

    if (!result?.meshes?.length)
      throw new Error('File STEP không có mesh nào.')

    if (current) {
      scene.remove(current)
      disposeGroup(current)
    }

    const group = new THREE.Group()
    let triangles = 0

    for (const mesh of result.meshes) {
      const position = mesh.attributes?.position?.array

      if (!position)
        continue

      const geometry = new THREE.BufferGeometry()

      geometry.setAttribute('position', new THREE.BufferAttribute(new Float32Array(position), 3))

      const normal = mesh.attributes?.normal?.array

      if (normal)
        geometry.setAttribute('normal', new THREE.BufferAttribute(new Float32Array(normal), 3))

      if (mesh.index?.array)
        geometry.setIndex(new THREE.BufferAttribute(new Uint32Array(mesh.index.array), 1))

      if (!normal)
        geometry.computeVertexNormals()

      const color = mesh.color
        ? new THREE.Color(mesh.color[0], mesh.color[1], mesh.color[2])
        : new THREE.Color('#9db8ff')
      const material = new THREE.MeshStandardMaterial({
        color,
        metalness: 0.15,
        roughness: 0.55,
      })

      group.add(new THREE.Mesh(geometry, material))
      triangles += (geometry.index?.count ?? geometry.attributes.position.count) / 3
    }

    scene.add(group)
    current = group
    frameCurrent()
    stats.value = `${result.meshes.length} part · ${Math.round(triangles).toLocaleString('vi-VN')} tam giác`
  }
  catch (err) {
    error.value = (err as Error).message
  }
  finally {
    loading.value = false
  }
}

async function pickStepFile() {
  const path = await pickNativePath('file', 'Chọn file STEP', { name: 'STEP', ext: 'step,stp' })

  if (!path)
    return

  pathDraft.value = path
  await openLocal()
}

async function pickStepFolder() {
  const path = await pickNativePath('folder', 'Chọn thư mục chứa STEP')

  if (!path)
    return

  pathDraft.value = path
  await openLocal()
}

async function openLocal() {
  const path = pathDraft.value.trim()

  if (!path)
    return

  pathLoading.value = true
  error.value = ''

  try {
    const files = await readLocalFiles(path, 'step')
    const file = files[0]

    if (!file)
      throw new Error('Không có file STEP trong đường dẫn này')

    localStorage.setItem('res-step-path', path)
    modelName.value = file.name
    pathOpen.value = false
    await loadStep(file.bytes.slice().buffer)
  }
  catch (err) {
    error.value = (err as Error).message
  }
  finally {
    pathLoading.value = false
  }
}

async function onFile(event: Event) {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]

  input.value = ''

  if (file)
    await loadStep(await file.arrayBuffer())
}

async function onDrop(event: DragEvent) {
  dragging.value = false
  const file = event.dataTransfer?.files?.[0]

  if (file)
    await loadStep(await file.arrayBuffer())
}

onMounted(() => {
  ensureScene()
  resize()

  if (host.value && typeof ResizeObserver !== 'undefined') {
    observer = new ResizeObserver(() => resize())
    observer.observe(host.value)
  }
})

onBeforeUnmount(() => {
  cancelAnimationFrame(frameId)
  observer?.disconnect()

  if (current) {
    disposeGroup(current)
    current = null
  }

  controls?.dispose()

  if (renderer) {
    renderer.dispose()
    renderer.domElement.remove()
    renderer = null
  }
})
</script>

<template>
  <div>
    <div class="d-flex flex-wrap align-center ga-2 mb-3">
      <VBtn
        variant="tonal"
        prepend-icon="ri-upload-2-line"
        @click="fileInput?.click()"
      >
        Chọn file STEP
      </VBtn>
      <VBtn
        variant="tonal"
        prepend-icon="ri-folder-open-line"
        @click="pathOpen = true"
      >
        Mở từ máy
      </VBtn>
      <VBtn
        v-if="stats"
        variant="text"
        prepend-icon="ri-restart-line"
        @click="frameCurrent"
      >
        Fit
      </VBtn>
      <VSpacer />
      <span class="text-caption text-medium-emphasis">{{ modelName || stats || 'Chưa có mô hình' }}</span>

      <input
        ref="fileInput"
        type="file"
        accept=".step,.stp,.STEP,.STP"
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
      class="step-wrap"
      :class="{ dragging }"
      @dragover.prevent="dragging = true"
      @dragleave.prevent="dragging = false"
      @drop.prevent="onDrop"
    >
      <div
        ref="host"
        class="step-viewport"
      />

      <div
        v-if="!stats"
        class="overlay"
        @click="fileInput?.click()"
      >
        <VProgressCircular
          v-if="loading"
          indeterminate
          color="primary"
        />
        <template v-else>
          <VIcon
            icon="ri-box-3-line"
            size="56"
            color="primary"
          />
          <div class="text-h6 font-weight-bold mt-3">
            Kéo file STEP (.step / .stp) vào đây
          </div>
          <div class="text-body-2 text-medium-emphasis mt-1">
            Hoặc dùng “Mở từ máy” để trỏ tới file/thư mục STEP. Xem 3D bằng OpenCascade
            (WASM) — lần đầu cần tải ~10 MB.
          </div>
        </template>
      </div>
    </div>

    <div class="text-caption text-medium-emphasis mt-2">
      Kéo để xoay · lăn chuột để zoom · chuột phải để pan.
    </div>

    <VDialog
      v-model="pathOpen"
      max-width="600"
    >
      <VCard class="pa-2">
        <div class="d-flex align-center pa-2">
          <span class="text-h6">Mở STEP trên máy</span>
          <VSpacer />
          <IconBtn @click="pathOpen = false">
            <VIcon icon="ri-close-line" />
          </IconBtn>
        </div>

        <div class="pa-3">
          <VTextField
            v-model="pathDraft"
            label="File hoặc thư mục STEP"
            placeholder="Bấm nút bên dưới để chọn"
            variant="outlined"
            density="comfortable"
            hide-details
            readonly
            @keydown.enter="openLocal"
          />
          <div class="d-flex flex-wrap ga-2 mt-3">
            <VBtn
              variant="tonal"
              prepend-icon="ri-file-3-line"
              @click="pickStepFile"
            >
              Chọn file…
            </VBtn>
            <VBtn
              variant="tonal"
              prepend-icon="ri-folder-open-line"
              @click="pickStepFolder"
            >
              Chọn thư mục…
            </VBtn>
          </div>
          <div class="text-caption text-medium-emphasis mt-2">
            Nếu là thư mục, app lấy file STEP đầu tiên. Không upload file đi đâu.
          </div>
          <div class="d-flex justify-end ga-2 mt-4">
            <VBtn
              variant="text"
              @click="pathOpen = false"
            >
              Huỷ
            </VBtn>
            <VBtn
              color="primary"
              :loading="pathLoading"
              @click="openLocal"
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
.step-wrap {
  position: relative;
  block-size: 72vh;
  border-radius: 10px;
  overflow: hidden;
  background: #0b1220;
}

.step-viewport {
  position: absolute;
  inset: 0;
}

.step-viewport :deep(canvas) {
  display: block;
}

.overlay {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 4px;
  cursor: pointer;
  color: rgb(var(--v-theme-on-surface));
  background: rgba(250, 250, 252, 0.92);
}

.step-wrap.dragging .overlay {
  background: rgba(var(--v-theme-primary), 0.08);
}
</style>
