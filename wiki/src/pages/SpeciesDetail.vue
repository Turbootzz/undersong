<script setup lang="ts">
import { computed } from "vue";
import speciesData from "../data/species.json";
import movesData from "../data/moves.json";
import locationsData from "../data/locations.json";
import { showSpoilers } from "../spoilers";

const props = defineProps<{ id: string }>();
const s = computed(() => speciesData.find((x) => x.id === props.id));
const learnset = computed(
  () => (s.value?.learnset ?? []) as [number, string][],
);
const moveName = (id: string) =>
  movesData.find((m) => m.id === id)?.name ?? id;
const statNames = ["HP", "ATK", "DEF", "SPA", "SPD", "SPE"];
const foundIn = computed(() =>
  locationsData.filter(
    (l) =>
      (showSpoilers.value || !l.spoiler) &&
      ((l.slots as [string, number, number, number][]).some((slot) => slot[0] === props.id) ||
        (l.night_slots as [string, number, number, number][]).some((slot) => slot[0] === props.id)),
  ),
);
const evolvesFrom = computed(() =>
  speciesData.find((x) => x.evolves_to && x.evolves_to[0] === props.id),
);
</script>

<template>
  <div v-if="!s" class="text-stone-500">Unknown voice.</div>
  <div v-else-if="s.spoiler && !showSpoilers" class="text-stone-500">
    This page hums beyond the spoiler line. Flip the toggle if you're sure.
  </div>
  <div v-else class="grid md:grid-cols-[260px_1fr] gap-8">
    <div>
      <img
        :src="`sprites/${s.region}/${s.id}.front.png`"
        :alt="s.name"
        class="w-48 h-48 mx-auto [image-rendering:pixelated]"
      />
      <h2 class="text-center text-xl text-gilt mt-2">{{ s.name }}</h2>
      <div class="text-center text-sm text-stone-400">{{ s.types.join(" / ") }} · {{ s.region }}</div>
      <p class="text-sm text-stone-300 mt-4 leading-relaxed">{{ s.entry }}</p>
      <div class="mt-4 text-xs text-stone-400 space-y-1">
        <div>catch rate {{ s.catch_rate }} · growth {{ s.growth }}</div>
        <div>ability: {{ s.abilities.join(", ") }}<span v-if="s.hidden_ability"> (hidden: {{ s.hidden_ability }})</span></div>
        <div v-if="evolvesFrom">evolves from <RouterLink :to="`/species/${evolvesFrom.id}`" class="text-gilt">{{ evolvesFrom.name }}</RouterLink></div>
        <div v-if="s.evolves_to">evolves into <RouterLink :to="`/species/${s.evolves_to[0]}`" class="text-gilt">{{ s.evolves_to[0] }}</RouterLink> ({{ s.evolves_to[1] }})</div>
      </div>
    </div>
    <div>
      <h3 class="text-gilt mb-2">Base stats</h3>
      <div v-for="(v, i) in s.base_stats" :key="i" class="flex items-center gap-2 mb-1">
        <span class="w-10 text-xs text-stone-400">{{ statNames[i] }}</span>
        <div class="h-2 bg-stone-800 rounded flex-1 max-w-72">
          <div class="h-full bg-gilt rounded" :style="{ width: Math.min(v / 1.2, 100) + '%' }" />
        </div>
        <span class="text-xs w-8">{{ v }}</span>
      </div>
      <h3 class="text-gilt mt-6 mb-2">Learnset</h3>
      <table class="text-sm w-full max-w-md">
        <tbody>
          <tr v-for="[lvl, mv] in learnset" :key="mv + String(lvl)" class="border-b border-stone-800/60">
            <td class="py-1 pr-4 text-stone-400 w-12">L{{ lvl }}</td>
            <td>{{ moveName(mv) }}</td>
          </tr>
        </tbody>
      </table>
      <h3 class="text-gilt mt-6 mb-2">Found in</h3>
      <div v-if="foundIn.length === 0" class="text-sm text-stone-500">
        Not in the wild — evolution, gift, or beyond the spoiler line.
      </div>
      <ul class="text-sm space-y-1">
        <li v-for="l in foundIn" :key="l.map">
          {{ l.name }}
          <span class="text-xs text-stone-500" v-if="(l.night_slots as [string, number, number, number][]).some((x) => x[0] === s!.id)">(night)</span>
        </li>
      </ul>
    </div>
  </div>
</template>
