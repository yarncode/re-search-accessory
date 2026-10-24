export type Supplier = {
  id: number
  name: string
  slug?: string
  products_count?: string | number
}

export type PriceTier = { quantity: number; price: number }

export type Product = {
  id: number
  name: string
  slug: string
  image_id: number | null
  inventory: number
  price: number
  desc_small?: string | null
  manufacturer_code?: string | null
  unit?: string | null
  multiples?: number
  minimum_order_quantity?: number
  supplier?: Supplier | null
  prices?: PriceTier[]
  rohs?: boolean
}

export type Spec = { name: string; value: string }

export type ProductDetail = Product & {
  specs?: Spec[]
  url?: string
  images?: { product_id: number; image_id: number }[]
}

export type Page<T> = {
  data: T[]
  total: number
  total_page: number
  current_page: number
  per_page: number
}

export type ListingResponse = {
  data: { products: Page<Product> }
}

export type SearchResponse = {
  data: { products: Page<Product>; categories: CategoryLite[] }
}

export type CategoryLite = { id: number; name: string; slug?: string; image_id?: number }

export type Category = {
  id: number
  name: string
  slug: string
  parent_id: number
  level: number
  count: number
  image_id?: number
}

export type CategoryNode = Category & { children: CategoryNode[] }

export type PropertyValue = {
  id: number
  name: string
  products_count: number
  actual_count?: number
}

export type PropertyGroup = {
  id: number
  name: string
  childrens: PropertyValue[]
}

export type FilterMeta = {
  suppliers: Supplier[]
  category_properties: PropertyGroup[]
  totalInStock: number
  total: number
}

export type FiltersBundle = {
  filters: { data: FilterMeta } | null
  filtered: { data: { filtered: { id: number }[] } } | null
}

export type SupplierAvailability = {
  data: { filtered: { supplier_id: number; supplier_name: string; products_count: string }[] }
}

export type ProductDetailResponse = { data: { product: ProductDetail | null } }

export type JlcPart = {
  lcsc: string
  mpn: string
  package: string
  manufacturer: string
  description: string
  datasheet: string
  price: number
  stock: number
  category: string
}

export type JlcStatus = {
  available: boolean
  path: string
  size: number
  parts: number
  error: string | null
}

export type SavedBom = {
  id: string
  name: string
  saved_at: number
}

export type FootprintModel = {
  path: string
  exists: boolean
  size: number
  format: string
  offset: [number, number, number]
  scale: [number, number, number]
  rotate: [number, number, number]
  data: string | null
}

export type FootprintResolved = {
  footprint: string
  library: string
  found: boolean
  footprint_path: string | null
  models: FootprintModel[]
  error: string | null
  searched: string[]
}
