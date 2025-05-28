<template>
  <div style="position: relative; height: 45px">
    <div class="labelText">{{ label }}</div>
    <div class="valueText">{{ textTruncated }}</div>
    <img class="valueText" :src="imageUrl" v-if="imageUrl" height="20px" />
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { truncate } from '@/utils/string';

const props = defineProps(['label', 'text', 'imageUrl']);

const textTruncated = computed(() => {
  return truncate(props.text, 42);
});
</script>

<style scoped>
.labelText {
  font-size: 0.7em;
  text-transform: uppercase;
  color: rgb(100, 100, 100);
  position: absolute;
  top: 0px;
  left: 0px;
}

.valueText {
  text-align: left;
  font-size: 1em;
  position: absolute;
  top: 15px;
  left: 0px;
}
</style>
