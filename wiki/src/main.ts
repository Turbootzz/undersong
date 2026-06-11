import { createApp } from "vue";
import { createRouter, createWebHashHistory } from "vue-router";
import App from "./App.vue";
import SpeciesList from "./pages/SpeciesList.vue";
import SpeciesDetail from "./pages/SpeciesDetail.vue";
import TypeChart from "./pages/TypeChart.vue";
import Moves from "./pages/Moves.vue";
import Locations from "./pages/Locations.vue";
import "./style.css";

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/", redirect: "/species" },
    { path: "/species", component: SpeciesList },
    { path: "/species/:id", component: SpeciesDetail, props: true },
    { path: "/types", component: TypeChart },
    { path: "/moves", component: Moves },
    { path: "/locations", component: Locations },
  ],
});

createApp(App).use(router).mount("#app");
