import { ref, watchEffect } from "vue";

const stored = localStorage.getItem("undersong.spoilers");
export const showSpoilers = ref(stored === "true");
watchEffect(() =>
  localStorage.setItem("undersong.spoilers", String(showSpoilers.value)),
);
