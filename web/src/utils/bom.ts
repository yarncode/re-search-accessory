import type { Product } from '@/types'

export type BomStatus = 'pending' | 'ok' | 'review' | 'missing'

export type BomItem = {
  id: string
  ref: string
  qty: number
  input: string
  footprint: string
  status: BomStatus
  note: string
  product: Product | null
  candidates: Product[]
  extra: Record<string, string>
  purchase: 'need' | 'ordered' | 'received'
  orderedAt: string
  supplier: string
}

export type ColumnMap = {
  search: string
  qty: string
  ref: string
  footprint: string
}

export type BomDoc = {
  id: string
  name: string
  savedAt: string
  columns: ColumnMap
  items: BomItem[]
}

export type ParsedSheet = {
  headers: string[]
  rows: Record<string, string>[]
}

const PART_PATTERNS = [
  /(mpn|manufacturer\s*part|part\s*(number|no|#)?|mã\s*linh\s*kiện|ma\s*linh\s*kien|model|sku|lcsc|code)/i,
]

const VALUE_PATTERNS = [
  /(value|name|description|desc|tên|ten|giá\s*trị|gia\s*tri|linh\s*kiện|linh\s*kien)/i,
]

const FOOTPRINT_PATTERNS = [
  /(footprint|package|case|kiểu\s*đóng\s*gói|kieu\s*dong\s*goi|vỏ)/i,
]

const QTY_PATTERNS = [/^(qty|quantity|số\s*lượng|so\s*luong|sl|count|số\s*lượng\s*cần)$/i]
const REF_PATTERNS = [/^(ref|refs|designator|reference|ký\s*hiệu|ky\s*hieu|vị\s*trí|vi\s*tri|position)$/i]

export async function parseBomFile(file: File): Promise<ParsedSheet> {
  const buffer = await file.arrayBuffer()
  // SheetJS is ~350 KB: load it only when a file is actually imported, not when
  // the BOM page opens (it used to block the page transition).
  const XLSX = await import('xlsx')
  const workbook = XLSX.read(buffer, { type: 'array' })
  const sheetName = workbook.SheetNames[0]

  if (!sheetName)
    throw new Error('File không có sheet nào.')

  const matrix = XLSX.utils.sheet_to_json<unknown[]>(workbook.Sheets[sheetName], {
    header: 1,
    defval: '',
    blankrows: false,
  })

  if (!matrix.length)
    throw new Error('File không có dữ liệu.')

  // The header is usually the first row with at least two filled cells.
  let headerIndex = 0

  for (let index = 0; index < Math.min(matrix.length, 10); index += 1) {
    const filled = matrix[index].filter(cell => String(cell).trim() !== '').length

    if (filled >= 2) {
      headerIndex = index
      break
    }
  }

  const headers = matrix[headerIndex].map((cell, index) => {
    const label = String(cell).trim()

    return label || `Cột ${index + 1}`
  })

  const rows = matrix
    .slice(headerIndex + 1)
    .map((cells) => {
      const row: Record<string, string> = {}

      headers.forEach((header, index) => {
        row[header] = String(cells[index] ?? '').trim()
      })

      return row
    })
    .filter(row => Object.values(row).some(value => value !== ''))

  if (!rows.length)
    throw new Error('Không đọc được dòng dữ liệu nào.')

  return { headers, rows }
}

function firstMatch(headers: string[], patterns: RegExp[]): string {
  for (const pattern of patterns) {
    const found = headers.find(header => pattern.test(header.trim()))

    if (found)
      return found
  }

  return ''
}

export function detectColumns(headers: string[]): ColumnMap {
  const part = firstMatch(headers, PART_PATTERNS)
  const value = firstMatch(headers, VALUE_PATTERNS)
  const footprint = firstMatch(headers, FOOTPRINT_PATTERNS)
  const search = part || value || headers[0] || ''
  const qty = firstMatch(headers, QTY_PATTERNS)
  const ref = firstMatch(headers, REF_PATTERNS)

  return {
    search,
    footprint: footprint && footprint !== search ? footprint : '',
    qty,
    ref,
  }
}

/** KiCad footprints are library ids, not product names. Pull out the package
 *  tokens (0805, SOT-23, SOIC-8…) and the family words so they can act as a
 *  ranking signal / fallback query. */
const PACKAGE_RE = /(0201|0402|0603|0805|1206|1210|1812|2010|2512|1005|1608|2012|3216|3225|3528|4532|6032|7343|sot-?23|sot-?223|sot-?89|soic-?\d+|sop-?\d+|ssop-?\d+|tssop-?\d+|msop-?\d+|qfn-?\d*|dfn-?\d*|lqfp-?\d+|tqfp-?\d+|dip-?\d+|to-?220|to-?252|to-?92|do-?214|do-?41|sma|smb|smc|bga|wlcsp)/gi

const FAMILY_STOPWORDS = /^(smd|tht|metric|horizontal|vertical|package|lib|kicad|pad|hand|solder|connector|capacitor|resistor|inductor|diode|led|crystal|switch|button|conn)$/i

export function footprintTokens(value: string): string[] {
  const text = String(value ?? '').trim()

  if (!text)
    return []

  const tail = text.includes(':') ? text.split(':').pop() ?? '' : text
  const tokens: string[] = []

  for (const match of tail.matchAll(PACKAGE_RE)) {
    const token = match[0].toLowerCase().replace(/\s+/g, '-')

    if (!tokens.includes(token))
      tokens.push(token)
  }

  for (const part of tail.split(/[_\-\s:]+/)) {
    const token = part.trim().toLowerCase()

    const redundant = tokens.some(existing => existing === token || existing.startsWith(token) || token.startsWith(existing))
    const dimension = /mm$/i.test(token) || /^\d+(\.\d+)?(x\d+(\.\d+)?)+$/i.test(token)

    if (
      token.length >= 4
      && !FAMILY_STOPWORDS.test(token)
      && !/^\d+$/.test(token)
      && !dimension
      && !redundant
    ) {
      tokens.push(token)
    }
  }

  return tokens.slice(0, 5)
}

/** True only for real footprint strings; a value like "100nF" or "10k" is not one. */
export function looksLikeFootprint(value: string): boolean {
  const text = String(value ?? '').trim()

  if (!text)
    return false

  if (text.includes(':') || /[_/]/.test(text))
    return true

  return footprintTokens(text).some(token => /^(0[2-9]\d{2}|1[0-9]{3}|[2-9]\d{3}|soic|sop|ssop|tssop|msop|qfn|dfn|lqfp|tqfp|dip|sot|to-|do-|sma|smb|smc|bga|wlcsp)/.test(token))
}

/** Most useful single shop query from a footprint (e.g. "0805", "soic-16"). */
export function footprintQuery(value: string): string {
  return footprintTokens(value)[0] ?? ''
}

/** Primary plus one fallback query (soic-16 -> soic, CR1220_HOLDER -> cr1220, holder). */
export function footprintQueries(value: string): string[] {
  const tokens = footprintTokens(value)

  if (!tokens.length)
    return []

  const queries = [tokens[0]]
  const family = tokens[0].split('-')[0]

  if (family.length >= 3 && family !== tokens[0])
    queries.push(family)
  else {
    const next = tokens.find((token, index) => index > 0 && !/^\d+$/.test(token) && token.length >= 4)

    if (next)
      queries.push(next)
  }

  return queries.slice(0, 2)
}

/** 0..20: how many footprint tokens appear in the product text. */
export function footprintScore(footprint: string, product: Product): number {
  const tokens = footprintTokens(footprint)

  if (!tokens.length)
    return 0

  const haystack = normalizeToken(`${product.name} ${product.desc_small ?? ''} ${product.manufacturer_code ?? ''}`)
  let hits = 0

  for (const token of tokens) {
    const normalized = normalizeToken(token)

    if (haystack.includes(normalized)) {
      hits += 1
      continue
    }

    // "soic-16" should also match "16SOIC": check every part of the token.
    const parts = token.split('-').map(normalizeToken).filter(Boolean)

    if (parts.length > 1 && parts.every(part => haystack.includes(part)))
      hits += 1
  }

  return Math.min(20, hits * 10)
}

export function parseQty(value: string | undefined): number {
  const cleaned = String(value ?? '').replace(',', '.')
  const match = cleaned.match(/\d+(\.\d+)?/)
  const parsed = match ? Number(match[0]) : 1

  return Number.isFinite(parsed) && parsed > 0 ? parsed : 1
}

export function normalizeToken(value: string): string {
  return value.toLowerCase().replace(/[^a-z0-9]/g, '')
}

/** 100 = exact part number, 95 = exact name, 85/72 = close, 35 = weak. */
export function scoreProduct(input: string, product: Product): number {
  const query = normalizeToken(input)

  if (!query)
    return 0

  const code = product.manufacturer_code ? normalizeToken(product.manufacturer_code) : ''
  const name = normalizeToken(product.name)

  if (code && code === query)
    return 100

  if (name === query)
    return 95

  if (code && (code.includes(query) || query.includes(code)) && Math.abs(code.length - query.length) <= 6)
    return 85

  if (name.includes(query))
    return 72

  if (query.length >= 5 && name.includes(query.slice(0, Math.min(10, query.length))))
    return 55

  return 35
}

export function buildItems(
  rows: Record<string, string>[],
  columns: ColumnMap,
): BomItem[] {
  return rows.map((row, index) => ({
    id: `row-${index}-${Math.random().toString(36).slice(2, 7)}`,
    ref: columns.ref ? row[columns.ref] || String(index + 1) : String(index + 1),
    qty: parseQty(columns.qty ? row[columns.qty] : ''),
    input: columns.search ? row[columns.search] || '' : '',
    footprint: columns.footprint ? row[columns.footprint] || '' : '',
    status: 'pending',
    note: '',
    product: null,
    candidates: [],
    extra: row,
    purchase: 'need',
    orderedAt: '',
    supplier: '',
  }))
}

export function downloadCsv(fileName: string, header: string[], rows: (string | number)[][]) {
  const escape = (value: string | number) => `"${String(value ?? '').replace(/"/g, '""')}"`
  const lines = rows.map(row => row.map(escape).join(','))

  const blob = new Blob([`\uFEFF${header.join(',')}\n${lines.join('\n')}`], {
    type: 'text/csv;charset=utf-8',
  })
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')

  link.href = url
  link.download = fileName.endsWith('.csv') ? fileName : `${fileName}.csv`
  link.click()
  URL.revokeObjectURL(url)
}
