import { computed, ref, watch } from 'vue'
import { defineStore } from 'pinia'
import { apiGet, apiSend } from '@/api'
import type { JlcPart, JlcStatus, Product, SavedBom, SearchResponse } from '@/types'
import {
  buildItems,
  downloadCsv,
  footprintQueries,
  footprintScore,
  looksLikeFootprint,
  scoreProduct,
  type BomItem,
  type ColumnMap,
} from '@/utils/bom'

const CONCURRENCY = 3
const HISTORY_LIMIT = 60
const AUTOSAVE_MS = 500

type DocPayload = {
  version: number
  fileName: string
  columns: ColumnMap
  boards: number
  scrap: number
  project: string
  headers: string[]
  rawRows: Record<string, string>[]
  items: BomItem[]
  docId?: string | null
  docName?: string
}

type DraftResponse = { draft: { payload: string; updated_at: number } | null }

export const PURCHASE_LABEL: Record<BomItem['purchase'], string> = {
  need: 'Cần mua',
  ordered: 'Đã đặt',
  received: 'Đã nhận',
}

export const useBomStore = defineStore('bom', () => {
  const fileName = ref('')
  const headers = ref<string[]>([])
  const rawRows = ref<Record<string, string>[]>([])
  const columns = ref<ColumnMap>({ search: '', qty: '', ref: '', footprint: '' })
  const boards = ref(1)
  const scrap = ref(5)
  const project = ref('')
  const items = ref<BomItem[]>([])

  const analyzing = ref(false)
  const done = ref(0)
  const cancelled = ref(false)

  const currentId = ref<string | null>(null)
  const currentName = ref('')
  const savedDocs = ref<SavedBom[]>([])
  const draftAt = ref<number | null>(null)
  const docsReady = ref(false)

  const jlcStatus = ref<JlcStatus | null>(null)
  const jlcParts = ref<Record<string, JlcPart[]>>({})
  const jlcLoading = ref(false)
  const fxRate = ref(Number(localStorage.getItem('res-fx-rate') || 26000))

  const alternates = ref<Record<number, Product[]>>({})
  const alternatesLoading = ref<Record<number, boolean>>({})

  const undoStack = ref<string[]>([])
  const redoStack = ref<string[]>([])

  /* ---------------- payload / autosave ---------------- */
  function payloadString(): string {
    const payload: DocPayload = {
      version: 2,
      fileName: fileName.value,
      columns: columns.value,
      boards: boards.value,
      scrap: scrap.value,
      project: project.value,
      headers: headers.value,
      rawRows: rawRows.value,
      items: items.value,
      docId: currentId.value,
      docName: currentName.value,
    }

    return JSON.stringify(payload)
  }

  function applyPayload(text: string) {
    try {
      const doc = JSON.parse(text) as Partial<DocPayload>

      fileName.value = doc.fileName ?? ''
      headers.value = doc.headers ?? []
      rawRows.value = doc.rawRows ?? []
      columns.value = {
        search: doc.columns?.search ?? '',
        qty: doc.columns?.qty ?? '',
        ref: doc.columns?.ref ?? '',
        footprint: doc.columns?.footprint ?? '',
      }
      boards.value = doc.boards && doc.boards > 0 ? doc.boards : 1
      scrap.value = Number.isFinite(doc.scrap) ? Number(doc.scrap) : 5
      project.value = doc.project ?? ''
      currentId.value = doc.docId ?? null
      currentName.value = doc.docName ?? ''
      items.value = (doc.items ?? []).map(item => ({
        ...item,
        footprint: item.footprint ?? '',
        purchase: item.purchase ?? 'need',
        orderedAt: item.orderedAt ?? '',
        supplier: item.supplier ?? '',
      }))
      done.value = 0
    }
    catch {
      // corrupt payload: keep the current state
    }
  }

  let autosaveTimer: number | undefined
  let saving = false
  let dirtyAgain = false

  async function saveDraft() {
    if (saving) {
      dirtyAgain = true

      return
    }

    saving = true

    try {
      const payload = payloadString()
      const response = await apiSend<{ updated_at: number }>('PUT', '/bom/draft', { payload })

      draftAt.value = response.updated_at

      // A named BOM is updated silently too: once the user names it, no further
      // manual save is ever needed.
      if (currentId.value) {
        await apiSend('PUT', `/bom/docs/${encodeURIComponent(currentId.value)}`, {
          name: currentName.value,
          payload,
        })
      }
    }
    catch {
      // autosave failures are non-fatal
    }
    finally {
      saving = false

      if (dirtyAgain) {
        dirtyAgain = false
        scheduleSave()
      }
    }
  }

  function scheduleSave() {
    window.clearTimeout(autosaveTimer)
    autosaveTimer = window.setTimeout(() => void saveDraft(), AUTOSAVE_MS)
  }

  /** Write the pending change immediately (tab hidden / app closing). */
  function flushSave() {
    window.clearTimeout(autosaveTimer)
    void saveDraft()
  }

  watch([items, columns, boards, scrap, fileName, project], scheduleSave, { deep: true })

  /* ---------------- undo / redo ---------------- */
  function pushHistory() {
    undoStack.value = [...undoStack.value.slice(-(HISTORY_LIMIT - 1)), payloadString()]
    redoStack.value = []
  }

  function restore(text: string) {
    applyPayload(text)
    scheduleSave()
  }

  function undo() {
    const previous = undoStack.value.at(-1)

    if (!previous)
      return

    undoStack.value = undoStack.value.slice(0, -1)
    redoStack.value = [...redoStack.value, payloadString()]
    restore(previous)
  }

  function redo() {
    const next = redoStack.value.at(-1)

    if (!next)
      return

    redoStack.value = redoStack.value.slice(0, -1)
    undoStack.value = [...undoStack.value, payloadString()]
    restore(next)
  }

  /* ---------------- documents ---------------- */
  async function refreshDocs() {
    try {
      const response = await apiGet<{ docs: SavedBom[] }>('/bom/docs')

      savedDocs.value = response.docs ?? []
    }
    catch {
      savedDocs.value = []
    }
    finally {
      docsReady.value = true
    }
  }

  async function saveAs(name: string) {
    if (!items.value.length)
      return

    const id = currentId.value ?? `bom-${Date.now()}`
    const label = name.trim() || fileName.value || 'BOM'
    const response = await apiSend<{ id: string }>('POST', '/bom/docs', {
      id,
      name: label,
      payload: payloadString(),
    })

    currentId.value = response.id
    currentName.value = label
    scheduleSave()
    await refreshDocs()
  }

  async function loadDoc(id: string) {
    const response = await apiGet<{ doc: { payload: string; name: string } }>(
      `/bom/docs/${encodeURIComponent(id)}`,
    )

    pushHistory()
    applyPayload(response.doc.payload)
    currentId.value = id
    currentName.value = response.doc.name
    scheduleSave()
  }

  async function deleteDoc(id: string) {
    await apiSend('DELETE', `/bom/docs/${encodeURIComponent(id)}`)

    if (currentId.value === id) {
      currentId.value = null
      currentName.value = ''
      scheduleSave()
    }

    await refreshDocs()
  }

  let initialized = false

  async function init() {
    if (initialized)
      return

    initialized = true
    await Promise.all([refreshDocs(), loadJlcStatus()])

    // Closing the tab/app right after an edit must not lose it.
    window.addEventListener('visibilitychange', () => {
      if (document.visibilityState === 'hidden')
        flushSave()
    })
    window.addEventListener('pagehide', flushSave)

    try {
      const response = await apiGet<DraftResponse>('/bom/draft')

      if (response.draft?.payload && !items.value.length) {
        applyPayload(response.draft.payload)
        draftAt.value = response.draft.updated_at
      }
    }
    catch {
      // no draft yet
    }
  }

  /* ---------------- import / mapping ---------------- */
  function setParsed(
    name: string,
    parsedHeaders: string[],
    rows: Record<string, string>[],
    map: ColumnMap,
  ) {
    pushHistory()
    fileName.value = name
    headers.value = parsedHeaders
    rawRows.value = rows
    columns.value = { ...map }
    items.value = buildItems(rows, map)
    currentId.value = null
    currentName.value = ''
    done.value = 0
    scheduleSave()
  }

  function setColumn(key: keyof ColumnMap, value: string) {
    pushHistory()
    columns.value = { ...columns.value, [key]: value }
    items.value = buildItems(rawRows.value, columns.value)
    currentId.value = null
    done.value = 0
    scheduleSave()
  }

  function setBoards(value: number) {
    boards.value = Number.isFinite(value) && value > 0 ? Math.round(value) : 1
  }

  function setScrap(value: number) {
    scrap.value = Number.isFinite(value) && value >= 0 ? value : 0
  }

  function setProject(value: string) {
    project.value = value.trim()
    scheduleSave()
  }

  /* ---------------- quantities ---------------- */
  function requiredQty(item: BomItem): number {
    const total = Math.max(0, item.qty) * Math.max(1, boards.value)

    return Math.max(1, Math.ceil(total * (1 + Math.max(0, scrap.value) / 100)))
  }

  function orderQty(item: BomItem): number {
    const need = requiredQty(item)

    if (!item.product)
      return need

    const multiple = Math.max(1, item.product.multiples ?? 1)
    const moq = Math.max(1, item.product.minimum_order_quantity ?? 1)

    return Math.max(moq, Math.ceil(need / multiple) * multiple)
  }

  function shortage(item: BomItem): boolean {
    return Boolean(item.product && item.product.inventory < orderQty(item))
  }

  /* ---------------- matching ---------------- */
  function find(id: string) {
    return items.value.find(item => item.id === id)
  }

  async function searchProducts(query: string): Promise<Product[]> {
    const body = await apiGet<SearchResponse>('/search', { q: query, limit: 6 })

    return body.data?.products?.data ?? []
  }

  async function matchItem(item: BomItem) {
    const primary = item.input.trim()
    const footprint = (item.footprint ?? '').trim()
    const primaryIsFootprint = looksLikeFootprint(primary)

    const queries: string[] = []

    if (primary) {
      const candidates = primaryIsFootprint ? footprintQueries(primary) : [primary]

      for (const query of candidates) {
        if (query && !queries.includes(query))
          queries.push(query)
      }
    }

    if (footprint && footprint !== primary) {
      const fallbacks = footprintQueries(footprint).slice(0, primary ? 1 : 2)

      for (const query of fallbacks) {
        if (query && !queries.includes(query))
          queries.push(query)
      }
    }

    queries.splice(3)

    if (!queries.length) {
      item.status = 'missing'
      item.product = null
      item.candidates = []
      item.note = 'Thiếu dữ liệu'

      return
    }

    const pool = new Map<number, Product>()

    for (let index = 0; index < queries.length; index += 1) {
      const products = await searchProducts(queries[index])

      for (const product of products) {
        if (!pool.has(product.id))
          pool.set(product.id, product)
      }

      if (index === 0 && primary && !primaryIsFootprint) {
        const bestText = products.reduce(
          (max, product) => Math.max(max, scoreProduct(primary, product)),
          0,
        )

        if (bestText >= 95)
          break
      }
    }

    const footprintSignal = footprint || (primaryIsFootprint ? primary : '')
    const scored = [...pool.values()]
      .map(product => {
        const text = primary && !primaryIsFootprint ? scoreProduct(primary, product) : 0
        const fp = footprintSignal ? footprintScore(footprintSignal, product) : 0

        return { product, text, fp, total: text + fp }
      })
      .sort((a, b) => b.total - a.total)

    item.candidates = scored.slice(0, 4).map(entry => entry.product)
    const best = scored[0]

    if (!best) {
      item.status = 'missing'
      item.product = null
      item.note = 'Không tìm thấy'
    }
    else if (best.text >= 95) {
      item.status = 'ok'
      item.product = best.product
      item.note = 'Khớp chính xác'
    }
    else if (best.text >= 70 && best.fp >= 10) {
      item.status = 'ok'
      item.product = best.product
      item.note = 'Khớp giá trị + footprint'
    }
    else if (best.text >= 70) {
      item.status = 'review'
      item.product = best.product
      item.note = 'Khớp giá trị, khác footprint'
    }
    else if (best.fp >= 10) {
      item.status = 'review'
      item.product = best.product
      item.note = 'Chỉ khớp footprint'
    }
    else if (best.text >= 40) {
      item.status = 'review'
      item.product = best.product
      item.note = 'Cần kiểm tra'
    }
    else {
      item.status = 'missing'
      item.product = null
      item.note = 'Không tìm thấy'
    }
  }

  async function analyzeItem(id: string) {
    const item = find(id)

    if (!item)
      return

    pushHistory()
    item.status = 'pending'
    item.note = 'Đang tìm…'

    try {
      await matchItem(item)
    }
    catch (err) {
      item.status = 'missing'
      item.note = (err as Error).message
    }
  }

  async function analyze() {
    if (!items.value.length || analyzing.value)
      return

    pushHistory()
    analyzing.value = true
    cancelled.value = false
    done.value = 0

    for (const item of items.value) {
      item.status = 'pending'
      item.note = ''
      item.product = null
      item.candidates = []
    }

    const queue = [...items.value]
    const total = queue.length

    const worker = async () => {
      while (!cancelled.value) {
        const item = queue.shift()

        if (!item)
          return

        try {
          await matchItem(item)
        }
        catch (err) {
          item.status = 'missing'
          item.note = (err as Error).message
        }

        done.value += 1
      }
    }

    await Promise.all(Array.from({ length: Math.min(CONCURRENCY, total) }, () => worker()))
    analyzing.value = false
  }

  function cancel() {
    cancelled.value = true
  }

  /* ---------------- item edits ---------------- */
  function toggleOk(id: string) {
    const item = find(id)

    if (!item)
      return

    pushHistory()
    item.status = item.status === 'ok' ? (item.product ? 'review' : 'missing') : 'ok'
  }

  function pick(id: string, product: Product) {
    const item = find(id)

    if (!item)
      return

    pushHistory()
    item.product = product
    item.status = 'ok'
    item.note = 'Chọn thủ công'
    item.candidates = [product, ...item.candidates.filter(candidate => candidate.id !== product.id)]
  }

  function setQty(id: string, qty: number) {
    const item = find(id)

    if (!item)
      return

    pushHistory()
    item.qty = Number.isFinite(qty) && qty > 0 ? qty : 1
  }

  function setInput(id: string, value: string) {
    const item = find(id)

    if (!item)
      return

    pushHistory()
    item.input = value
    item.status = 'pending'
    item.note = 'Đã sửa từ khoá'
  }

  function setPurchase(id: string, purchase: BomItem['purchase']) {
    const item = find(id)

    if (!item)
      return

    pushHistory()
    item.purchase = purchase

    if (purchase === 'ordered' && !item.orderedAt)
      item.orderedAt = new Date().toISOString().slice(0, 10)
  }

  function cyclePurchase(id: string) {
    const item = find(id)

    if (!item)
      return

    setPurchase(id, item.purchase === 'need' ? 'ordered' : item.purchase === 'ordered' ? 'received' : 'need')
  }

  function setOrderedAt(id: string, value: string) {
    const item = find(id)

    if (!item)
      return

    pushHistory()
    item.orderedAt = value
  }

  function setSupplier(id: string, value: string) {
    const item = find(id)

    if (!item)
      return

    pushHistory()
    item.supplier = value
  }

  /* ---------------- alternates / optimisation ---------------- */
  async function loadAlternates(productId: number) {
    if (alternates.value[productId] !== undefined || alternatesLoading.value[productId])
      return

    alternatesLoading.value = { ...alternatesLoading.value, [productId]: true }

    try {
      const response = await apiGet<{ data: { products: Product[] } }>('/alternates', {
        product_id: productId,
      })

      alternates.value = { ...alternates.value, [productId]: response.data?.products ?? [] }
    }
    catch {
      alternates.value = { ...alternates.value, [productId]: [] }
    }
    finally {
      alternatesLoading.value = { ...alternatesLoading.value, [productId]: false }
    }
  }

  function bestCandidate(item: BomItem): Product | null {
    const need = orderQty(item)
    const pool = new Map<number, Product>()

    if (item.product)
      pool.set(item.product.id, item.product)

    for (const product of alternates.value[item.product?.id ?? -1] ?? [])
      pool.set(product.id, product)

    for (const product of item.candidates)
      pool.set(product.id, product)

    const list = [...pool.values()]
    const enough = list.filter(product => product.inventory >= need)
    const inStock = list.filter(product => product.inventory > 0)
    const ranked = [...(enough.length ? enough : inStock.length ? inStock : list)]
      .sort((a, b) => a.price - b.price)

    return ranked[0] ?? null
  }

  async function optimizeRow(id: string): Promise<number> {
    const item = find(id)

    if (!item?.product)
      return 0

    await loadAlternates(item.product.id)
    const best = bestCandidate(item)

    if (!best || best.id === item.product.id)
      return 0

    const savings = (item.product.price - best.price) * orderQty(item)

    if (savings <= 0)
      return 0

    pushHistory()
    item.product = best
    item.status = 'ok'
    item.note = 'Tối ưu giá'

    return savings
  }

  async function optimizeAll(): Promise<number> {
    const rows = items.value.filter(item => item.product)

    if (!rows.length)
      return 0

    pushHistory()

    let savings = 0
    const queue = [...rows]

    const worker = async () => {
      for (;;) {
        const item = queue.shift()

        if (!item?.product)
          return

        await loadAlternates(item.product.id)
        const best = bestCandidate(item)

        if (!best || best.id === item.product.id)
          continue

        const diff = (item.product.price - best.price) * orderQty(item)

        if (diff > 0) {
          savings += diff
          item.product = best
          item.status = 'ok'
          item.note = 'Tối ưu giá'
        }
      }
    }

    await Promise.all([worker(), worker()])

    return savings
  }

  /* ---------------- JLCPCB comparison ---------------- */
  async function loadJlcStatus() {
    try {
      jlcStatus.value = await apiGet<JlcStatus>('/jlcpcb/status')
    }
    catch {
      jlcStatus.value = null
    }
  }

  function jlcFor(item: BomItem): JlcPart | null {
    const mpn = item.product?.manufacturer_code?.trim()

    if (!mpn)
      return null

    return jlcParts.value[mpn]?.[0] ?? null
  }

  async function compareJlc() {
    if (!items.value.length || jlcLoading.value)
      return

    await loadJlcStatus()

    if (!jlcStatus.value?.available)
      return

    jlcLoading.value = true
    const mpns = [...new Set(
      items.value
        .map(item => item.product?.manufacturer_code?.trim())
        .filter((value): value is string => Boolean(value)),
    )]
    const queue = [...mpns]

    const worker = async () => {
      for (;;) {
        const mpn = queue.shift()

        if (!mpn)
          return

        if (jlcParts.value[mpn])
          continue

        try {
          const response = await apiGet<{ parts: JlcPart[] }>('/jlcpcb/lookup', { mpn, limit: 3 })

          jlcParts.value = { ...jlcParts.value, [mpn]: response.parts ?? [] }
        }
        catch {
          jlcParts.value = { ...jlcParts.value, [mpn]: [] }
        }
      }
    }

    await Promise.all([worker(), worker(), worker()])
    jlcLoading.value = false
  }

  function setFxRate(value: number) {
    if (!Number.isFinite(value) || value <= 0)
      return

    fxRate.value = value
    localStorage.setItem('res-fx-rate', String(value))
  }

  /* ---------------- summary ---------------- */
  const counts = computed(() => ({
    total: items.value.length,
    ok: items.value.filter(item => item.status === 'ok').length,
    review: items.value.filter(item => item.status === 'review').length,
    missing: items.value.filter(item => item.status === 'missing').length,
    pending: items.value.filter(item => item.status === 'pending').length,
    shortage: items.value.filter(item => shortage(item)).length,
    ordered: items.value.filter(item => item.purchase === 'ordered').length,
    received: items.value.filter(item => item.purchase === 'received').length,
  }))

  const totalCost = computed(() =>
    items.value.reduce((sum, item) => sum + (item.product ? item.product.price * orderQty(item) : 0), 0),
  )

  const jlcDelta = computed(() => {
    let delta = 0

    for (const item of items.value) {
      const jlc = jlcFor(item)

      if (!jlc || !item.product)
        continue

      delta += (jlc.price * fxRate.value - item.product.price) * orderQty(item)
    }

    return delta
  })

  const jlcCount = computed(() => items.value.filter(item => jlcFor(item)).length)

  /* ---------------- export / reset ---------------- */
  function exportCsv() {
    const header = [
      'Ref', 'Qty/board', 'Boards', 'Required', 'Order qty', 'Purchase', 'Ordered at', 'Supplier',
      'Input', 'Footprint', 'Match', 'Matched name', 'MPN', 'Brand', 'Unit price', 'Ext price',
      'Stock', 'Shortage', 'JLC LCSC', 'JLC price USD', 'JLC stock', 'URL',
    ]

    const rows = items.value.map(item => {
      const jlc = jlcFor(item)
      const qty = orderQty(item)

      return [
        item.ref,
        item.qty,
        boards.value,
        requiredQty(item),
        qty,
        PURCHASE_LABEL[item.purchase],
        item.orderedAt,
        item.supplier,
        item.input,
        item.footprint,
        item.status,
        item.product?.name ?? '',
        item.product?.manufacturer_code ?? '',
        item.product?.supplier?.name ?? '',
        item.product?.price ?? 0,
        (item.product?.price ?? 0) * qty,
        item.product?.inventory ?? '',
        shortage(item) ? 'THIEU' : '',
        jlc?.lcsc ?? '',
        jlc?.price ?? '',
        jlc?.stock ?? '',
        item.product ? `https://www.thegioiic.com/${item.product.slug}` : '',
      ]
    })

    downloadCsv(fileName.value || 'bom', header, rows)
  }

  function reset() {
    pushHistory()
    fileName.value = ''
    headers.value = []
    rawRows.value = []
    items.value = []
    currentId.value = null
    currentName.value = ''
    done.value = 0
    scheduleSave()
  }

  return {
    // state
    fileName,
    headers,
    columns,
    boards,
    scrap,
    project,
    items,
    analyzing,
    done,
    currentId,
    currentName,
    savedDocs,
    draftAt,
    docsReady,
    jlcStatus,
    jlcParts,
    jlcLoading,
    fxRate,
    alternates,
    alternatesLoading,
    undoStack,
    redoStack,
    // derived
    counts,
    totalCost,
    jlcDelta,
    jlcCount,
    // lifecycle
    init,
    // docs
    refreshDocs,
    saveAs,
    loadDoc,
    deleteDoc,
    // import / mapping
    setParsed,
    setColumn,
    setBoards,
    setScrap,
    setProject,
    // quantities
    requiredQty,
    orderQty,
    shortage,
    // analysis
    analyze,
    analyzeItem,
    cancel,
    // edits
    toggleOk,
    pick,
    setQty,
    setInput,
    setPurchase,
    cyclePurchase,
    setOrderedAt,
    setSupplier,
    // alternates / optimisation
    loadAlternates,
    optimizeRow,
    optimizeAll,
    // jlc
    loadJlcStatus,
    jlcFor,
    compareJlc,
    setFxRate,
    // misc
    undo,
    redo,
    pushHistory,
    exportCsv,
    reset,
  }
})
