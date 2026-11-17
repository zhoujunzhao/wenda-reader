import { createApp } from "vue";

import { createRouter, createWebHashHistory } from 'vue-router'

// import { zhCN, dateZhCN } from 'naive-ui'
// import { createTheme, inputDark, datePickerDark } from 'naive-ui'
// naive-ui 官网：https://www.naiveui.com/zh-CN/os-theme

import {
  create,
  NButton,
  NInput,
  NTabs,
  NTab,
  NPopover,
  NFlex,
  NRadio,
  NSpace,
  NCard
} from "naive-ui";

const naive = create({
  components: [NButton,NInput,NTabs,NTab,NPopover,NFlex,NRadio,NSpace,NCard]
});

import App from "./App.vue";
import TopTabs from "./TopTabs.vue";
import Setting from "./Setting.vue";

const routes = [
  {
    path: '/',
    name: 'Home',
    component: TopTabs
  },
  {
    path: '/setting',
    name: 'Setting',
    component: Setting
  }
];

const router = createRouter({
  history: createWebHashHistory(),
  routes
})

const app = createApp(App);

app.use(naive).use(router).mount("#app");
