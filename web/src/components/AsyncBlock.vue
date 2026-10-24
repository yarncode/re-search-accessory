<script setup lang="ts">
defineProps<{ label?: string }>()
</script>

<template>
  <div class="async-block">
    <VProgressCircular
      indeterminate
      color="primary"
      size="32"
      width="3"
    />
    <span class="text-caption text-medium-emphasis">{{ label || 'Đang tải…' }}</span>
  </div>
</template>

<style scoped>
.async-block {
  display: flex;
  flex-direction: column;
  gap: 10px;
  align-items: center;
  justify-content: center;
  min-block-size: 38vh;
  padding: 24px;
}
</style>
