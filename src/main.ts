import { createApp } from "vue";
import App from "./App.vue";
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
} from "naive-ui";

const naive = create({
  components: [NButton,NInput,NTabs,NTab,NPopover,NFlex,NRadio,NSpace]
});

const app = createApp(App);

app.use(naive).mount("#app");
