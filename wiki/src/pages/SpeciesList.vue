<script setup lang="ts">
import { computed, ref } from "vue";
import speciesData from "../data/species.json";
import { showSpoilers } from "../spoilers";

const query = ref("");
const typeFilter = ref("");
const types = [
  "Feral","Ember","Tide","Bloom","Gale","Stone","Frost","Volt","Venom","Phantom","Alloy","Resonant",
];
const visible = computed(() =>
  speciesData.filter((s) => {
    if (s.spoiler && !showSpoilers.value) return false;
    if (typeFilter.value && !s.types.includes(typeFilter.value)) return false;
    const q = query.value.toLowerCase();
    return !q || s.name.toLowerCase().includes(q) || s.id.includes(q);
  }),
);
</script>

<template>
  <div class="flex gap-3 mb-4 flex-wrap">
    <input
      v-model="query"
      placeholder="search the score..."
      class="bg-stone-900 border border-stone-700 rounded px-3 py-1.5 text-sm w-64 focus:border-gilt outline-none"
    />
    <select
      v-model="typeFilter"
      class="bg-stone-900 border border-stone-700 rounded px-2 py-1.5 text-sm"
    >
      <option value="">all types</option>
      <option v-for="t in types" :key="t" :value="t">{{ t }}</option>
    </select>
    <span class="text-xs text-stone-500 self-center">{{ visible.length }} voices</span>
  </div>
  <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 gap-3">
    <RouterLink
      v-for="s in visible"
      :key="s.id"
      :to="`/species/${s.id}`"
      class="bg-stone-900/70 border border-stone-800 rounded-lg p-3 hover:border-gilt transition-colors"
    >
      <img
        :src="`sprites/${s.region}/${s.id}.front.png`"
        :alt="s.name"
        class="w-20 h-20 mx-auto [image-rendering:pixelated]"
        loading="lazy"
      />
      <div class="text-center mt-2 text-sm">{{ s.name }}</div>
      <div class="text-center text-xs text-gilt/80">{{ s.types.join(" / ") }}</div>
    </RouterLink>
  </div>
</template>
