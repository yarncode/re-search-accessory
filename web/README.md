# re-search-accessory — web

Frontend của **re-search-accessory**, dựng trên **Materio – Vuetify Vuejs Admin Template (free)**
của ThemeSelection (MIT).

- Vue 3.5 + TypeScript + Vite 5
- Vuetify 3.7 + icon Remix (`@iconify-json/ri`, sinh bởi `postinstall`)
- Pinia + vue-router

## Lệnh

```bash
npm install     # chạy postinstall: sinh src/plugins/iconify/icons.css
npm run dev     # http://127.0.0.1:5173, proxy /api -> 127.0.0.1:8787
npm run build   # -> dist/ (backend Rust sẽ serve thư mục này)
npm run typecheck
```

## Cấu trúc phần app

- `src/pages/index.vue` — trang tra cứu (home hero + danh sách + bộ lọc).
- `src/pages/bom.vue` — phân tích BOM từ CSV/Excel (SheetJS), matching, lưu localStorage, xuất CSV.
- `src/stores/accessory.ts` — URL query là nguồn state duy nhất; mọi thao tác ghi lại query
  rồi watcher tải dữ liệu (nhờ vậy back/forward và link chia sẻ đều hoạt động).
- `src/views/accessory/` — các component tra cứu; `src/views/bom/` — dòng BOM + tìm thủ công.
- `src/stores/bom.ts`, `src/utils/bom.ts` — state BOM và parser/scoring.
- `src/layouts/components/NavbarSearch.vue` — ô tìm kiếm + gợi ý trong navbar Materio.
- `src/layouts/components/NavItems.vue` — nav dọc: danh mục cấp 1.

## Ghi công

Materio – Vuetify Vuejs Admin Template (Free), Copyright (c) 2022 ThemeSelection,
MIT License. Xem `LICENSE`.
