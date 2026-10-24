<script lang="ts" setup>
import Footer from '@/layouts/components/Footer.vue'
import NavbarSearch from '@/layouts/components/NavbarSearch.vue'
import NavbarThemeSwitcher from '@/layouts/components/NavbarThemeSwitcher.vue'
import NavItems from '@/layouts/components/NavItems.vue'
import VerticalNavLayout from '@layouts/components/VerticalNavLayout.vue'
</script>

<template>
  <VerticalNavLayout>
    <!-- 👉 navbar -->
    <template #navbar="{ toggleVerticalOverlayNavActive }">
      <div class="d-flex h-100 align-center ga-3">
        <!-- 👉 Vertical nav toggle in overlay mode -->
        <IconBtn
          class="ms-n3 d-lg-none"
          @click="toggleVerticalOverlayNavActive(true)"
        >
          <VIcon icon="ri-menu-line" />
        </IconBtn>

        <NavbarSearch />

        <VSpacer />

        <IconBtn
          href="https://www.thegioiic.com/"
          target="_blank"
          rel="noopener noreferrer"
        >
          <VIcon icon="ri-external-link-line" />
        </IconBtn>

        <NavbarThemeSwitcher class="me-2" />
      </div>
    </template>

    <template #vertical-nav-header="{ toggleIsOverlayNavActive }">
      <RouterLink
        to="/"
        class="app-logo"
      >
        <VAvatar
          color="primary"
          rounded="lg"
          size="38"
        >
          <VIcon
            icon="ri-cpu-line"
            color="white"
            size="20"
          />
        </VAvatar>

        <h1 class="font-weight-medium leading-normal text-xl">
          re-search
        </h1>
      </RouterLink>

      <IconBtn
        class="d-block d-lg-none"
        @click="toggleIsOverlayNavActive(false)"
      >
        <VIcon icon="ri-close-line" />
      </IconBtn>
    </template>

    <template #vertical-nav-content>
      <NavItems />
    </template>

    <!-- 👉 Pages -->
    <slot />

    <!-- 👉 Footer -->
    <template #footer>
      <Footer />
    </template>
  </VerticalNavLayout>
</template>

<style lang="scss" scoped>
.app-logo {
  display: flex;
  align-items: center;
  column-gap: 0.75rem;

  h1 {
    font-size: 1.1rem;
  }
}
</style>
