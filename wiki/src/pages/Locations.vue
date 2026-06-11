<script setup lang="ts">
import { computed } from "vue";
import locationsData from "../data/locations.json";
import speciesData from "../data/species.json";
import { showSpoilers } from "../spoilers";

const visible = computed(() =>
  locationsData.filter((l) => showSpoilers.value || !l.spoiler),
);
const name = (id: string) => speciesData.find((s) => s.id === id)?.name ?? id;
type Slot = [string, number, number, number];
const slots = (l: (typeof locationsData)[number]) => l.slots as Slot[];
const nightSlots = (l: (typeof locationsData)[number]) => l.night_slots as Slot[];
</script>

<template>
  <h2 class="text-gilt mb-4">Where the wild voices sing</h2>
  <div class="space-y-6">
    <div v-for="l in visible" :key="l.map" class="bg-stone-900/60 border border-stone-800 rounded-lg p-4">
      <h3 class="text-parchment">{{ l.name }} <span class="text-xs text-stone-500">({{ l.region }})</span></h3>
      <div class="mt-2 grid sm:grid-cols-2 gap-4 text-sm">
        <div>
          <div class="text-xs text-gilt/70 mb-1">day</div>
          <ul>
            <li v-for="[id, lo, hi, w] in slots(l)" :key="'d' + id + String(lo)" class="flex justify-between">
              <RouterLink :to="`/species/${id}`" class="hover:text-gilt">{{ name(id) }}</RouterLink>
              <span class="text-stone-500">L{{ lo }}–{{ hi }} · {{ w }}%</span>
            </li>
          </ul>
        </div>
        <div v-if="l.night_slots.length">
          <div class="text-xs text-gilt/70 mb-1">night</div>
          <ul>
            <li v-for="[id, lo, hi, w] in nightSlots(l)" :key="'n' + id + String(lo)" class="flex justify-between">
              <RouterLink :to="`/species/${id}`" class="hover:text-gilt">{{ name(id) }}</RouterLink>
              <span class="text-stone-500">L{{ lo }}–{{ hi }} · {{ w }}%</span>
            </li>
          </ul>
        </div>
      </div>
    </div>
  </div>
</template>
