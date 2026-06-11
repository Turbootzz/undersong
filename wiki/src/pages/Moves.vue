<script setup lang="ts">
import { computed, ref } from "vue";
import movesData from "../data/moves.json";

const query = ref("");
const visible = computed(() =>
  movesData
    .filter((m) => !query.value || m.name.toLowerCase().includes(query.value.toLowerCase()))
    .sort((a, b) => a.name.localeCompare(b.name)),
);
</script>

<template>
  <input
    v-model="query"
    placeholder="search moves..."
    class="bg-stone-900 border border-stone-700 rounded px-3 py-1.5 text-sm w-64 mb-4 focus:border-gilt outline-none"
  />
  <table class="text-sm w-full">
    <thead class="text-gilt/80 text-left">
      <tr>
        <th class="py-1">Move</th><th>Type</th><th>Cat</th><th class="text-right">Pow</th>
        <th class="text-right">Acc</th><th class="text-right">PP</th><th>Sound</th>
      </tr>
    </thead>
    <tbody>
      <tr v-for="m in visible" :key="m.id" class="border-b border-stone-800/60">
        <td class="py-1">{{ m.name }}</td>
        <td class="text-stone-400">{{ m.type }}</td>
        <td class="text-stone-400">{{ m.category.slice(0, 4) }}</td>
        <td class="text-right">{{ m.power || "—" }}</td>
        <td class="text-right">{{ m.accuracy }}</td>
        <td class="text-right">{{ m.pp }}</td>
        <td class="text-center">{{ m.sound ? "♪" : "" }}</td>
      </tr>
    </tbody>
  </table>
</template>
