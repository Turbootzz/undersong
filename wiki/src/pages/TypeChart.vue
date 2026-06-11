<script setup lang="ts">
import chart from "../data/typechart.json";

const cellClass = (m: number) =>
  m === 0
    ? "bg-stone-800 text-stone-500"
    : m < 10
      ? "bg-red-900/60"
      : m > 10
        ? "bg-emerald-800/70"
        : "";
const label = (m: number) => (m === 10 ? "" : m === 0 ? "0" : m < 10 ? "½" : "2");
</script>

<template>
  <h2 class="text-gilt mb-4">What beats what (attacker → defender)</h2>
  <div class="overflow-x-auto">
    <table class="text-xs border-collapse">
      <thead>
        <tr>
          <th class="p-1"></th>
          <th v-for="d in chart.types" :key="d" class="p-1 rotate-0 text-gilt/80 font-normal">
            {{ d.slice(0, 3) }}
          </th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="(row, ai) in chart.multipliers" :key="ai">
          <th class="p-1 text-left text-gilt/80 font-normal pr-2">{{ chart.types[ai] }}</th>
          <td
            v-for="(m, di) in row"
            :key="di"
            class="w-8 h-8 text-center border border-stone-800"
            :class="cellClass(m)"
          >
            {{ label(m) }}
          </td>
        </tr>
      </tbody>
    </table>
  </div>
  <p class="text-xs text-stone-500 mt-3">blank = neutral · 2 = double · ½ = half · 0 = no effect</p>
</template>
