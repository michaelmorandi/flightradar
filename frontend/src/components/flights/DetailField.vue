<template>
  <div style="position: relative; height: 45px">
    <div class="labelText">{{ label }}</div>
    <div class="valueText" :class="{ 'has-tooltip': tooltip }">
      {{ textTruncated }}
      <span v-if="tooltip" class="tooltip-text">{{ tooltip }}</span>
    </div>
    <img class="valueText" :src="imageUrl" v-if="imageUrl" height="20px" />
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { truncate } from '@/utils/string';

const props = defineProps(['label', 'text', 'imageUrl', 'tooltip']);

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

.valueText.has-tooltip {
  cursor: help;
  position: relative;
}

.tooltip-text {
  visibility: hidden;
  opacity: 0;
  background-color: #333;
  color: #fff;
  text-align: center;
  padding: 5px 10px;
  border-radius: 4px;
  font-size: 0.85em;
  white-space: nowrap;
  position: absolute;
  z-index: 1000;
  bottom: 125%;
  left: 0;
  transition: opacity 0.2s;
}

.tooltip-text::after {
  content: '';
  position: absolute;
  top: 100%;
  left: 10px;
  border-width: 5px;
  border-style: solid;
  border-color: #333 transparent transparent transparent;
}

.valueText.has-tooltip:hover .tooltip-text {
  visibility: visible;
  opacity: 1;
}
</style>
