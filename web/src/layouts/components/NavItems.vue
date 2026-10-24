<script lang="ts" setup>
import VerticalNavSectionTitle from '@/@layouts/components/VerticalNavSectionTitle.vue'
import VerticalNavLink from '@layouts/components/VerticalNavLink.vue'

const topCategories = [
  { id: 1, name: 'IC - Mạch Tích Hợp' },
  { id: 2, name: 'Mạch Điện, Module' },
  { id: 3, name: 'Linh Kiện Thụ Động' },
  { id: 7, name: 'Diode, Transistor, FET' },
  { id: 4, name: 'LCD, LED, Cảm Ứng' },
  { id: 5, name: 'Cổng Kết Nối, Jack' },
  { id: 6, name: 'Dây Điện, Cáp' },
  { id: 920, name: 'Cảm Biến' },
  { id: 966, name: 'Nguồn, Pin, Biến Áp' },
  { id: 1116, name: 'Cơ Điện Tử, Nút Nhấn' },
  { id: 935, name: 'Điện Công Nghiệp' },
  { id: 940, name: 'Thiết Bị Đo' },
  { id: 1117, name: 'Khí Nén, Thủy Lực' },
  { id: 965, name: 'Dụng Cụ, Tools' },
  { id: 967, name: 'Phụ Kiện Máy Tính' },
  { id: 1118, name: 'An Toàn, Phòng Sạch' },
]
</script>

<template>
  <VerticalNavSectionTitle :item="{ heading: 'Tra cứu' }" />
  <VerticalNavLink
    :item="{
      title: 'Tìm linh kiện',
      to: '/',
      icon: 'ri-search-line',
    }"
  />

  <VerticalNavLink
    :item="{
      title: 'Phân tích BOM',
      to: '/bom',
      icon: 'ri-file-list-3-line',
    }"
  />

  <VerticalNavLink
    :item="{
      title: 'Xem Gerber / 3D',
      to: '/viewer',
      icon: 'ri-box-3-line',
    }"
  />

  <VerticalNavSectionTitle :item="{ heading: 'Danh mục' }" />
  <VerticalNavLink
    v-for="category in topCategories"
    :key="category.id"
    :item="{
      title: category.name,
      href: `/?cat=${category.id}`,
      icon: 'ri-folder-3-line',
    }"
  />
</template>
