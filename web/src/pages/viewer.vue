<script setup lang="ts">
import { defineAsyncComponent, ref } from 'vue'
import { pickNativePath } from '@/api'
import { rememberSetting } from '@/settings'
import AsyncBlock from '@/components/AsyncBlock.vue'
import GerberViewer from '@/views/viewer/GerberViewer.vue'

// The Gerber tab is the default, so it ships eagerly. STEP (OpenCascade) and the
// footprint viewer (three.js + STEP/WRL) are big chunks: load them on first use,
// and warm them up on hover so clicking a tab does not stall.
const loadStep = () => import('@/views/viewer/StepViewer.vue')
const loadFootprint = () => import('@/views/viewer/FootprintViewer.vue')

const StepViewer = defineAsyncComponent({
  loader: loadStep,
  loadingComponent: AsyncBlock,
  delay: 80,
})

const FootprintViewer = defineAsyncComponent({
  loader: loadFootprint,
  loadingComponent: AsyncBlock,
  delay: 80,
})

const tab = ref<'gerber' | 'step' | 'footprint'>('gerber')

const fpName = ref(localStorage.getItem('res-fp-name') || 'Package_SO:SOIC-8_3.9x4.9mm_P1.27mm')
const fpProject = ref(localStorage.getItem('res-kicad-project') || '')
const applied = ref({
  name: fpName.value,
  project: fpProject.value,
})

function showFootprint() {
  localStorage.setItem('res-fp-name', fpName.value)
  localStorage.setItem('res-kicad-project', fpProject.value)
  applied.value = {
    name: fpName.value.trim(),
    project: fpProject.value.trim(),
  }

  if (fpProject.value.trim())
    rememberSetting('kicad_project', fpProject.value.trim())
}

async function pickProject() {
  const path = await pickNativePath('folder', 'Chọn thư mục project KiCad')

  if (!path)
    return

  fpProject.value = path
  showFootprint()
}
</script>

<template>
  <div>
    <div class="d-flex flex-wrap align-center ga-3 mb-2">
      <div>
        <h1 class="text-h5 font-weight-bold">
          Xem Gerber, 3D &amp; Footprint
        </h1>
        <div class="text-body-2 text-medium-emphasis">
          Mở thư mục gerber (nhiều file hoặc .zip), file STEP, hoặc xem 3D của một footprint KiCad.
        </div>
      </div>
    </div>

    <div class="d-flex flex-wrap align-center mb-3 page-sticky page-sticky--bordered">
      <VBtnGroup
        divided
        variant="outlined"
        class="flex-shrink-0 ms-auto"
      >
        <VBtn
          :variant="tab === 'gerber' ? 'flat' : 'outlined'"
          :color="tab === 'gerber' ? 'primary' : undefined"
          prepend-icon="ri-layout-masonry-line"
          @click="tab = 'gerber'"
        >
          Gerber
        </VBtn>
        <VBtn
          :variant="tab === 'step' ? 'flat' : 'outlined'"
          :color="tab === 'step' ? 'primary' : undefined"
          prepend-icon="ri-box-3-line"
          @mouseenter="loadStep()"
          @focus="loadStep()"
          @click="tab = 'step'"
        >
          STEP / 3D
        </VBtn>
        <VBtn
          :variant="tab === 'footprint' ? 'flat' : 'outlined'"
          :color="tab === 'footprint' ? 'primary' : undefined"
          prepend-icon="ri-focus-3-line"
          @mouseenter="loadFootprint()"
          @focus="loadFootprint()"
          @click="tab = 'footprint'"
        >
          Footprint 3D
        </VBtn>
      </VBtnGroup>
    </div>

    <template v-if="tab === 'footprint'">
      <VCard
        class="pa-3 mb-3"
        elevation="0"
      >
        <div class="d-flex flex-wrap align-center ga-2">
          <VTextField
            v-model="fpName"
            label="Footprint (Lib:Name)"
            density="compact"
            variant="outlined"
            hide-details
            style="min-inline-size: 260px; flex: 1"
            @keydown.enter="showFootprint"
          />
          <VTextField
            v-model="fpProject"
            label="Project KiCad (tuỳ chọn, cho KIPRJMOD)"
            density="compact"
            variant="outlined"
            hide-details
            readonly
            placeholder="Bấm nút thư mục để chọn"
            style="min-inline-size: 240px; flex: 1"
            @keydown.enter="showFootprint"
          />
          <VBtn
            variant="tonal"
            icon="ri-folder-open-line"
            title="Chọn thư mục project KiCad"
            @click="pickProject"
          />
          <VBtn
            color="primary"
            prepend-icon="ri-box-3-line"
            @click="showFootprint"
          >
            Xem 3D
          </VBtn>
        </div>
        <div class="text-caption text-medium-emphasis mt-2">
          App đọc fp-lib-table (global + project), lấy các dòng <code>(model ...)</code> trong file
          <code>.kicad_mod</code>, rồi dựng STEP/WRL với đúng offset/scale/rotate.
        </div>
      </VCard>

      <FootprintViewer
        :footprint="applied.name"
        :project="applied.project"
      />
    </template>

    <GerberViewer v-else-if="tab === 'gerber'" />
    <StepViewer v-else />
  </div>
</template>
