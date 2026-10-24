import * as THREE from 'three'

/**
 * GPU compositor for gerber layers.
 *
 * `gerber-to-svg` gives one SVG string per layer; instead of stacking 13 live
 * SVG documents in the DOM (Chromium re-rasterises them on every pan/zoom, which
 * is what made the old viewer crawl), each layer is rasterised once into a
 * texture and drawn as a textured quad by WebGL. Pan/zoom then only moves an
 * orthographic camera — GPU work, no layout, no re-rasterisation.
 */

export type GerberLayerInput = {
  id: string
  svg: string
  alpha: number
  order: number
  visible: boolean
}

type Entry = {
  input: GerberLayerInput
  mesh?: THREE.Mesh
  texture?: THREE.CanvasTexture
  pending?: Promise<void>
}

const MAX_TEXTURE = 3072
const MAX_TOTAL_PIXELS = 40_000_000
const MIN_VIEW_MM = 0.2
const MAX_VIEW_MM = 8000

export class GerberScene {
  private renderer: THREE.WebGLRenderer
  private scene = new THREE.Scene()
  private camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0.1, 100)
  private entries = new Map<string, Entry>()
  private sizeMm = { w: 1, h: 1 }
  private viewW = 1
  private center = new THREE.Vector2(0, 0)
  private frame = 0
  private observer: ResizeObserver | null = null
  private disposed = false
  /** Texture resolution for this set of layers, in pixels per mm. */
  private pxPerMm = 1

  constructor(private host: HTMLElement) {
    this.renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true, powerPreference: 'high-performance' })
    this.renderer.setPixelRatio(Math.min(2, window.devicePixelRatio || 1))
    this.renderer.setClearAlpha(0)
    this.renderer.domElement.style.display = 'block'
    this.renderer.domElement.style.width = '100%'
    this.renderer.domElement.style.height = '100%'
    host.appendChild(this.renderer.domElement)

    this.camera.position.set(0, 0, 10)

    this.observer = new ResizeObserver(() => this.resize())
    this.observer.observe(host)
    this.resize()
  }

  get ready(): boolean {
    return this.entries.size > 0 && [...this.entries.values()].every(entry => entry.texture || !entry.input.visible)
  }

  /** `box` is the union viewBox in gerber units (1/1000 mm). */
  async setLayers(layers: GerberLayerInput[], box: number[]): Promise<void> {
    this.clearMeshes()
    this.lastBox = box

    const w = Math.max(0.001, box[2] / 1000)
    const h = Math.max(0.001, box[3] / 1000)

    this.sizeMm = { w, h }

    // Keep the total texture budget bounded: 13 layers at full resolution would
    // be hundreds of MB of VRAM.
    const visibleCount = Math.max(1, layers.filter(layer => layer.visible).length)
    const boardPx = w * (MAX_TEXTURE / Math.max(w, h)) * h * (MAX_TEXTURE / Math.max(w, h))
    const budget = Math.sqrt(MAX_TOTAL_PIXELS / Math.max(1, boardPx * visibleCount))

    this.pxPerMm = (MAX_TEXTURE / Math.max(w, h)) * Math.min(1, budget)

    const pending: Promise<void>[] = []

    for (const layer of layers) {
      const entry: Entry = { input: layer }

      this.entries.set(layer.id, entry)

      if (layer.visible)
        pending.push(this.ensureTexture(entry, box))
    }

    this.fit()
    await Promise.all(pending)
  }

  setVisible(id: string, visible: boolean) {
    const entry = this.entries.get(id)

    if (!entry)
      return

    entry.input.visible = visible

    if (entry.mesh)
      entry.mesh.visible = visible

    if (visible && !entry.texture && !entry.pending) {
      const box = this.lastBox

      if (box)
        void this.ensureTexture(entry, box)
    }

    this.requestRender()
  }

  private lastBox: number[] | null = null

  private async ensureTexture(entry: Entry, box: number[]) {
    if (entry.texture || entry.pending)
      return entry.pending

    entry.pending = (async () => {
      try {
        const { w, h } = this.sizeMm
        const width = Math.max(64, Math.round(w * this.pxPerMm))
        const height = Math.max(64, Math.round(h * this.pxPerMm))
        const bitmap = await rasterizeSvg(entry.input.svg, box, width, height)
        const canvas = document.createElement('canvas')

        canvas.width = width
        canvas.height = height
        canvas.getContext('2d')?.drawImage(bitmap, 0, 0, width, height)
        bitmap.close?.()

        if (this.disposed)
          return

        const texture = new THREE.CanvasTexture(canvas)

        texture.colorSpace = THREE.SRGBColorSpace
        texture.anisotropy = this.renderer.capabilities.getMaxAnisotropy()
        texture.minFilter = THREE.LinearMipmapLinearFilter
        texture.magFilter = THREE.LinearFilter
        texture.needsUpdate = true

        const material = new THREE.MeshBasicMaterial({
          map: texture,
          transparent: true,
          opacity: entry.input.alpha,
          depthTest: false,
          depthWrite: false,
          toneMapped: false,
        })

        const mesh = new THREE.Mesh(new THREE.PlaneGeometry(w, h), material)

        mesh.position.set(0, 0, entry.input.order * 0.01)
        mesh.renderOrder = entry.input.order
        mesh.visible = entry.input.visible
        this.scene.add(mesh)

        entry.texture = texture
        entry.mesh = mesh
        this.requestRender()
      }
      finally {
        entry.pending = undefined
      }
    })()

    return entry.pending
  }

  /** Reset the camera so the whole board fills the viewport. */
  fit() {
    const { w, h } = this.sizeMm
    const aspect = this.aspect()
    const pad = 1.08

    let viewW = w * pad
    let viewH = viewW / aspect

    if (viewH < h * pad) {
      viewH = h * pad
      viewW = viewH * aspect
    }

    this.viewW = Math.min(MAX_VIEW_MM, Math.max(MIN_VIEW_MM, viewW))
    this.center.set(0, 0)
    this.updateCamera()
  }

  zoomAt(factor: number, clientX: number, clientY: number) {
    const rect = this.host.getBoundingClientRect()

    if (!rect.width || !rect.height)
      return

    const nx = (clientX - rect.left) / rect.width
    const ny = (clientY - rect.top) / rect.height
    const before = this.viewW
    const beforeH = before / this.aspect()
    const wx = this.center.x + (nx - 0.5) * before
    const wy = this.center.y - (ny - 0.5) * beforeH
    const next = Math.min(MAX_VIEW_MM, Math.max(MIN_VIEW_MM, before * factor))
    const nextH = next / this.aspect()

    this.viewW = next
    this.center.x = wx - (nx - 0.5) * next
    this.center.y = wy + (ny - 0.5) * nextH
    this.updateCamera()
  }

  /** Center anchor zoom, used by the +/- buttons. */
  zoomBy(factor: number) {
    const rect = this.host.getBoundingClientRect()

    this.zoomAt(factor, rect.left + rect.width / 2, rect.top + rect.height / 2)
  }

  panBy(dxPx: number, dyPx: number) {
    const rect = this.host.getBoundingClientRect()

    if (!rect.width || !rect.height)
      return

    const viewH = this.viewW / this.aspect()

    this.center.x -= (dxPx * this.viewW) / rect.width
    this.center.y += (dyPx * viewH) / rect.height
    this.updateCamera()
  }

  private aspect() {
    const rect = this.host.getBoundingClientRect()

    return Math.max(0.05, rect.width / Math.max(1, rect.height))
  }

  private resize() {
    const rect = this.host.getBoundingClientRect()

    if (!rect.width || !rect.height)
      return

    this.renderer.setSize(rect.width, rect.height, false)
    this.updateCamera()
  }

  private updateCamera() {
    const viewH = this.viewW / this.aspect()

    this.camera.left = this.center.x - this.viewW / 2
    this.camera.right = this.center.x + this.viewW / 2
    this.camera.top = this.center.y + viewH / 2
    this.camera.bottom = this.center.y - viewH / 2
    this.camera.updateProjectionMatrix()
    this.requestRender()
  }

  private requestRender() {
    if (this.frame || this.disposed)
      return

    this.frame = requestAnimationFrame(() => {
      this.frame = 0
      this.renderer.render(this.scene, this.camera)
    })
  }

  private clearMeshes() {
    for (const entry of this.entries.values())
      this.disposeEntry(entry)

    this.entries.clear()
  }

  private disposeEntry(entry: Entry) {
    if (entry.mesh) {
      this.scene.remove(entry.mesh)
      entry.mesh.geometry.dispose()
      const material = entry.mesh.material

      if (Array.isArray(material))
        material.forEach(item => item.dispose())
      else
        material.dispose()

      entry.mesh = undefined
    }

    entry.texture?.dispose()
    entry.texture = undefined
  }

  dispose() {
    this.disposed = true
    cancelAnimationFrame(this.frame)
    this.observer?.disconnect()
    this.clearMeshes()
    this.renderer.dispose()
    this.renderer.domElement.remove()
  }
}

/** Bake one layer SVG into an ImageBitmap at the requested pixel size. */
async function rasterizeSvg(svg: string, box: number[], width: number, height: number): Promise<ImageBitmap> {
  const parsed = new DOMParser().parseFromString(svg, 'image/svg+xml')
  const root = parsed.documentElement

  root.setAttribute('viewBox', box.join(' '))
  root.setAttribute('width', String(width))
  root.setAttribute('height', String(height))
  root.setAttribute('preserveAspectRatio', 'none')

  const text = new XMLSerializer().serializeToString(root)
  const url = URL.createObjectURL(new Blob([text], { type: 'image/svg+xml' }))

  try {
    const image = new Image()

    image.decoding = 'async'
    image.src = url
    await image.decode()

    return await createImageBitmap(image)
  }
  finally {
    URL.revokeObjectURL(url)
  }
}
