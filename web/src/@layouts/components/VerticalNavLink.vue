<script lang="ts" setup>
import { computed } from 'vue'
import { RouterLink, useRouter } from 'vue-router'
import type { NavLink } from '@layouts/types'

const props = defineProps<{
  item: NavLink
}>()

const router = useRouter()

// Warm up the heavier route chunks on hover/focus so the click itself does not
// have to fetch + parse them while the screen is switching.
const routePrefetch: Record<string, () => Promise<unknown>> = {
  '/bom': () => import('@/pages/bom.vue'),
  '/viewer': () => import('@/pages/viewer.vue'),
}

function prefetch() {
  const to = typeof props.item.to === 'string' ? props.item.to : ''

  routePrefetch[to]?.()
}

const linkComponent = computed(() => (props.item.to ? RouterLink : 'a'))
const linkProps = computed(() => (props.item.to
  ? { to: props.item.to, target: props.item.target }
  : { href: props.item.href, target: props.item.target }))

// An internal `href` (the category list) must not trigger a full page reload:
// a reload restarts the SPA and used to reset the theme. Keep the <a> markup
// and route through the router instead.
function onClick(event: MouseEvent) {
  const href = props.item.href

  if (!href?.startsWith('/') || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey)
    return

  event.preventDefault()
  void router.push(href)
}
</script>

<template>
  <li
    class="nav-link"
    :class="{ disabled: item.disable }"
  >
    <Component
      :is="linkComponent"
      v-bind="linkProps"
      @click="onClick"
      @mouseenter="prefetch"
      @focus="prefetch"
    >
      <VIcon
        :icon="item.icon || 'ri-checkbox-blank-circle-line'"
        class="nav-item-icon"
      />
      <!-- 👉 Title -->
      <span class="nav-item-title">
        {{ item.title }}
      </span>
      <span
        class="nav-item-badge"
        :class="item.badgeClass"
      >
        {{ item.badgeContent }}
      </span>
    </Component>
  </li>
</template>

<style lang="scss">
.layout-vertical-nav {
  .nav-link a {
    display: flex;
    align-items: center;
    cursor: pointer;
  }
}
</style>
