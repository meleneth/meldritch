<script setup lang="ts">
import DefaultTheme from 'vitepress/theme'
import { useRoute, withBase } from 'vitepress'
import { computed } from 'vue'

const route = useRoute()
const activePage = computed(() =>
  route.path === withBase('/') || route.path.startsWith(withBase('/meldritch2'))
)
</script>

<template>
  <DefaultTheme.Layout>
    <template #layout-top>
      <a class="development-banner" :href="activePage ? withBase('/meldritch2') : withBase('/archive/meldritch1')">
        <span class="development-dot" aria-hidden="true"></span>
        {{ activePage ? 'Meldritch 2 · active development' : 'Meldritch 1 · archived research · Meldritch 2 is active' }}
      </a>
    </template>
  </DefaultTheme.Layout>
</template>
