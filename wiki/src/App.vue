<script setup lang="ts">
import { showSpoilers } from "./spoilers";
</script>

<template>
  <div class="min-h-screen bg-ink text-parchment font-mono">
    <header class="border-b-2 border-gilt/60 bg-stone-950/80 sticky top-0 z-10">
      <div class="mx-auto max-w-5xl px-4 py-3 flex items-center gap-6">
        <h1 class="text-gilt tracking-[0.3em] font-bold">UNDERSONG</h1>
        <nav class="flex gap-4 text-sm">
          <RouterLink to="/species" class="hover:text-gilt" active-class="text-gilt">Score</RouterLink>
          <RouterLink to="/types" class="hover:text-gilt" active-class="text-gilt">Types</RouterLink>
          <RouterLink to="/moves" class="hover:text-gilt" active-class="text-gilt">Moves</RouterLink>
          <RouterLink to="/locations" class="hover:text-gilt" active-class="text-gilt">Locations</RouterLink>
        </nav>
        <label class="ml-auto flex items-center gap-2 text-xs text-stone-400 cursor-pointer">
          <input type="checkbox" v-model="showSpoilers" class="accent-gilt" />
          show spoilers
        </label>
      </div>
    </header>
    <main class="mx-auto max-w-5xl px-4 py-6">
      <RouterView />
    </main>
    <footer class="mx-auto max-w-5xl px-4 py-8 text-xs text-stone-500">
      Generated from the game's content files — the RON is the score; this is the programme.
    </footer>
  </div>
</template>
