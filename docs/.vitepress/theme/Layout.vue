<script setup lang="ts">
import DefaultTheme from 'vitepress/theme'
import { withBase } from 'vitepress'
</script>

<template>
  <DefaultTheme.Layout>
    <template #layout-top>
      <a class="development-banner" :href="withBase('/guide/status.html')">
        <span class="development-dot" aria-hidden="true"></span>
        In active development · Nothing is stable yet
      </a>
    </template>
  </DefaultTheme.Layout>
</template>
