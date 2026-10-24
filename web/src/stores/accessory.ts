import { computed, ref, watch } from 'vue'
import { defineStore } from 'pinia'
import { useRoute, useRouter } from 'vue-router'
import { apiGet } from '@/api'
import type {
  Category,
  CategoryLite,
  CategoryNode,
  FilterMeta,
  FiltersBundle,
  ListingResponse,
  Page,
  Product,
  SearchResponse,
  SupplierAvailability,
} from '@/types'

const DEFAULT_LIMIT = 40
const DEFAULT_SORT = 3

function num(value: unknown): number | null {
  if (value === undefined || value === null || value === '')
    return null

  const raw = Array.isArray(value) ? value[0] : value
  const parsed = Number(raw)

  return Number.isFinite(parsed) ? parsed : null
}

function buildTree(categories: Category[]): CategoryNode[] {
  const map = new Map<number, CategoryNode>()

  for (const category of categories)
    map.set(category.id, { ...category, children: [] })

  const roots: CategoryNode[] = []

  for (const category of categories) {
    const node = map.get(category.id)

    if (!node)
      continue

    const parent = category.parent_id ? map.get(category.parent_id) : undefined

    if (parent)
      parent.children.push(node)
    else
      roots.push(node)
  }

  const sortNodes = (nodes: CategoryNode[]) => {
    nodes.sort((a, b) => a.name.localeCompare(b.name, 'vi'))
    nodes.forEach(node => sortNodes(node.children))
  }

  sortNodes(roots)

  return roots
}

export type AccessoryUi = {
  q: string
  cat: number | null
  page: number
  limit: number
  sort: number
  supplier: number | null
  prid: number[]
  stock: boolean
  item: string | null
}

export const useAccessoryStore = defineStore('accessory', () => {
  const route = useRoute()
  const router = useRouter()

  /* ---------------- URL is the single source of truth ---------------- */
  const ui = computed<AccessoryUi>(() => {
    const q = route.query.q
    const prid = String(route.query.prid ?? '')

    return {
      q: typeof q === 'string' ? q.trim() : '',
      cat: num(route.query.cat),
      page: Math.max(1, num(route.query.page) ?? 1),
      limit: num(route.query.limit) ?? DEFAULT_LIMIT,
      sort: num(route.query.sort) ?? DEFAULT_SORT,
      supplier: num(route.query.brand),
      prid: prid.split(',').map(Number).filter(value => Number.isFinite(value) && value > 0),
      stock: route.query.stock === '1',
      item: typeof route.query.item === 'string' ? route.query.item : null,
    }
  })

  function navigate(patch: Record<string, string | number | null>) {
    const query: Record<string, string> = {}

    for (const [key, value] of Object.entries(route.query)) {
      if (typeof value === 'string')
        query[key] = value
    }

    for (const [key, value] of Object.entries(patch)) {
      if (value === null || value === '')
        delete query[key]
      else
        query[key] = String(value)
    }

    void router.replace({ query })
  }

  /* ---------------- actions ---------------- */
  const setSearch = (text: string) => navigate({ q: text.trim() || null, page: null, item: null })
  const clearSearch = () => navigate({ q: null, page: null })

  const selectCategory = (id: number) => navigate({
    cat: id,
    q: null,
    page: null,
    brand: null,
    prid: null,
    stock: null,
    item: null,
  })

  const goHome = () => navigate({ cat: null, q: null, page: null, item: null })

  const setPage = (page: number) => navigate({ page: page > 1 ? page : null })
  const setLimit = (limit: number) => navigate({ limit: limit === DEFAULT_LIMIT ? null : limit, page: null })
  const setSort = (sort: number) => navigate({ sort: sort === DEFAULT_SORT ? null : sort, page: null })
  const setSupplier = (id: number | null) => navigate({ brand: id, page: null })

  const togglePrid = (id: number) => {
    const next = ui.value.prid.includes(id)
      ? ui.value.prid.filter(value => value !== id)
      : [...ui.value.prid, id]

    navigate({ prid: next.length ? next.join(',') : null, page: null })
  }

  const setStock = (value: boolean) => navigate({ stock: value ? '1' : null, page: null })
  const clearFilters = () => navigate({ brand: null, prid: null, stock: null, page: null })
  const openDetail = (slug: string) => navigate({ item: slug })
  const closeDetail = () => navigate({ item: null })

  const reload = () => {
    void loadProducts()
    void loadFilters()
  }

  /* ---------------- catalogue ---------------- */
  const categories = ref<Category[]>([])
  const catalogLoading = ref(true)
  const supplierCount = ref<number | null>(null)
  let catalogTask: Promise<void> | null = null

  async function ensureCatalog() {
    if (!catalogTask) {
      catalogTask = (async () => {
        try {
          const body = await apiGet<{ data: { categories: Category[] } }>('/categories')

          categories.value = body.data?.categories ?? []
        }
        catch {
          categories.value = []
        }
        finally {
          catalogLoading.value = false
        }

        try {
          const body = await apiGet<{ data: { suppliers: Page<{ id: number }> } }>('/suppliers', { limit: 1 })

          supplierCount.value = body.data?.suppliers?.total ?? null
        }
        catch {
          supplierCount.value = null
        }
      })()
    }

    return catalogTask
  }

  const tree = computed(() => buildTree(categories.value))
  const categoryById = computed(() => new Map(categories.value.map(item => [item.id, item])))
  const nodeById = computed(() => {
    const map = new Map<number, CategoryNode>()

    const walk = (nodes: CategoryNode[]) => {
      for (const node of nodes) {
        map.set(node.id, node)
        walk(node.children)
      }
    }

    walk(tree.value)

    return map
  })

  /* ---------------- products ---------------- */
  const result = ref<{ products: Page<Product>; categories: CategoryLite[] } | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)
  let productAbort: AbortController | null = null
  let productSeq = 0
  let loadingTimer: number | undefined

  async function loadProducts() {
    productAbort?.abort()

    const state = ui.value

    if (!state.q && state.cat === null) {
      window.clearTimeout(loadingTimer)
      loading.value = false
      result.value = null
      error.value = null

      return
    }

    const seq = ++productSeq
    const controller = new AbortController()

    productAbort = controller
    error.value = null

    // Fast responses should not flash a spinner or dim the grid: only show the
    // loading state when a request actually takes a while.
    window.clearTimeout(loadingTimer)
    loadingTimer = window.setTimeout(() => {
      if (seq === productSeq)
        loading.value = true
    }, 180)

    const common = { page: state.page, limit: state.limit }
    const extra = { order_sort: state.sort || undefined, in_stock: state.stock ? 1 : undefined }

    try {
      const body = state.q
        ? await apiGet<SearchResponse>('/search', { q: state.q, ...common, ...extra }, controller.signal)
        : await apiGet<ListingResponse>(
          `/categories/${state.cat}/products`,
          {
            ...common,
            ...extra,
            supplier: state.supplier ?? undefined,
            prid: state.prid.join(',') || undefined,
          },
          controller.signal,
        )

      const suggested: CategoryLite[] = 'categories' in body.data && Array.isArray(body.data.categories)
        ? body.data.categories
        : []

      if (seq === productSeq)
        result.value = { products: body.data.products, categories: suggested }
    }
    catch (err) {
      if ((err as Error).name !== 'AbortError' && seq === productSeq)
        error.value = (err as Error).message
    }
    finally {
      if (seq === productSeq) {
        window.clearTimeout(loadingTimer)
        loading.value = false
      }
    }
  }

  /* ---------------- filters ---------------- */
  const filterMeta = ref<FilterMeta | null>(null)
  const filtersLoading = ref(false)
  const availablePrids = ref<Set<number> | null>(null)
  const availableSuppliers = ref<Set<number> | null>(null)
  let filterAbort: AbortController | null = null

  async function loadFilters() {
    filterAbort?.abort()

    const state = ui.value
    const filtersActive = state.prid.length > 0 || state.stock

    if (!state.cat || state.q) {
      filterMeta.value = null
      availablePrids.value = null
      availableSuppliers.value = null

      return
    }

    const controller = new AbortController()

    filterAbort = controller
    filtersLoading.value = true

    const params = {
      supplier: state.supplier ?? undefined,
      prid: state.prid.join(',') || undefined,
      in_stock: state.stock ? 1 : undefined,
    }

    try {
      const [bundle, suppliers] = await Promise.all([
        apiGet<FiltersBundle>(`/categories/${state.cat}/filters`, params, controller.signal),
        apiGet<SupplierAvailability>(
          `/categories/${state.cat}/suppliers`,
          { prid: params.prid, in_stock: params.in_stock },
          controller.signal,
        ).catch(() => null),
      ])

      filterMeta.value = bundle.filters?.data ?? null

      const ids = bundle.filtered?.data?.filtered

      availablePrids.value = filtersActive && ids ? new Set(ids.map(item => item.id)) : null

      const available = suppliers?.data?.filtered

      availableSuppliers.value = filtersActive && available
        ? new Set(available.map(item => item.supplier_id))
        : null
    }
    catch (err) {
      if ((err as Error).name !== 'AbortError')
        filterMeta.value = null
    }
    finally {
      if (!controller.signal.aborted)
        filtersLoading.value = false
    }
  }

  const listingKey = computed(() => JSON.stringify([
    ui.value.q,
    ui.value.cat,
    ui.value.page,
    ui.value.limit,
    ui.value.sort,
    ui.value.supplier,
    ui.value.prid,
    ui.value.stock,
  ]))

  const filtersKey = computed(() => JSON.stringify([
    ui.value.cat,
    ui.value.q,
    ui.value.supplier,
    ui.value.prid,
    ui.value.stock,
  ]))

  watch(listingKey, () => void loadProducts(), { immediate: true })
  watch(filtersKey, () => void loadFilters(), { immediate: true })

  /* ---------------- derived ---------------- */
  const view = computed<'home' | 'category' | 'search'>(() =>
    ui.value.q ? 'search' : ui.value.cat !== null ? 'category' : 'home',
  )

  const products = computed(() => result.value?.products.data ?? [])
  const total = computed(() => result.value?.products.total ?? 0)
  const totalPage = computed(() => result.value?.products.total_page ?? 0)
  const currentPage = computed(() => result.value?.products.current_page ?? ui.value.page)
  const activeCategory = computed(() => (ui.value.cat !== null ? categoryById.value.get(ui.value.cat) : undefined))
  const activeNode = computed(() => (ui.value.cat !== null ? nodeById.value.get(ui.value.cat) : undefined))
  const activeFilters = computed(() =>
    (ui.value.supplier ? 1 : 0) + ui.value.prid.length + (ui.value.stock ? 1 : 0),
  )
  const headerTitle = computed(() =>
    view.value === 'search' ? `“${ui.value.q}”` : activeCategory.value?.name ?? 'Sản phẩm',
  )

  return {
    // state
    ui,
    categories,
    catalogLoading,
    supplierCount,
    tree,
    categoryById,
    nodeById,
    result,
    loading,
    error,
    filterMeta,
    filtersLoading,
    availablePrids,
    availableSuppliers,
    // derived
    view,
    products,
    total,
    totalPage,
    currentPage,
    activeCategory,
    activeNode,
    activeFilters,
    headerTitle,
    // actions
    ensureCatalog,
    setSearch,
    clearSearch,
    selectCategory,
    goHome,
    setPage,
    setLimit,
    setSort,
    setSupplier,
    togglePrid,
    setStock,
    clearFilters,
    openDetail,
    closeDetail,
    reload,
  }
})
