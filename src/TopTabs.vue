<template>
  <div class="header">
    <div class="header-row-1">
      <div class="tabs-container">
        <n-tabs
            v-model:value="activeTabId"
            type="card"
            tab-style="min-width: 80px;"
            :addable="addable"
            @close="closeTab"
            :closable="true"
            @update:value="changeTab"
            @add="onTabAdded"
            size="small"
            :center-active-tab="true"
        >
          <n-tab v-for="tab in tabs" :name="tab.id" :closable="tab.closable">
            <n-popover trigger="hover" :arrow-point-to-center="true" :flip="true" placement="right">
              <template #trigger>
                {{ tab.title.length>=5 ? tab.title.substring(0, 5) + '...' : tab.title }}
              </template>
              <span>{{ tab.title }}</span>
            </n-popover>

          </n-tab>
        </n-tabs>
      </div>
      <div class="settings-container">
        <n-popover trigger="hover" :arrow-point-to-center="true" :flip="true" placement="right">
          <template #trigger>
            <n-icon @click="onSettingClick" style="cursor: pointer;">
              <LineHorizontal320Regular />
            </n-icon>
          </template>
          设置
        </n-popover>
      </div>
    </div>
    <div class="header-row-2">
      <div class="address-container">
        <n-input v-model:value="activeUrl" placeholder="请输入网址" class="address-input" size="small" :disabled="addressDisabled" />
        <n-button size="small" @click="gotoNewUrl" :disabled="addressDisabled">Go</n-button>
      </div>
      <div class="style-container" v-show="widthModeShow">
        <n-radio
            :checked="widthValue === 'default'"
            value="default"
            label="默认"
            name="reader-width"
            @change="onWidthChange"
        />
        <n-radio
            :checked="widthValue ==='70%'"
            value="70%"
            label="70%"
            name="reader-width"
            @change="onWidthChange"
        />
        <n-radio
            :checked="widthValue === '95%'"
            value="95%"
            label="95%"
            name="reader-width"
            @change="onWidthChange"
        />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import {onMounted, onUnmounted, ref, watch} from "vue"
import {listen} from "@tauri-apps/api/event";
import {invoke} from "@tauri-apps/api/core";
import { LineHorizontal320Regular } from '@vicons/fluent'

const ID_FIRST_PAGE_ID: string = "tab-main";

interface Tab {
  id: string,
  title: string,
  url: string,
  closable: boolean,
  isShow: boolean,
}

const tabs = ref<Tab[]>([]);
const activeTabId = ref<string>();
const activeUrl = ref<string>();
const addable = ref<boolean>(true);
const addressDisabled = ref<boolean>(false);
const widthValue = ref<string>("default");
const widthModeShow = ref<boolean>(false);

const onSettingClick = () => {
  sendActionToBackend('openSetting', { });
};

const onWidthChange = (e: Event) => {
  const val = (e.target as HTMLInputElement).value;
  widthValue.value = val;
  sendActionToBackend('changeWidthMode', { "tabId": activeTabId.value, "mode":val });
};

const doWidthValue = (data:any) =>{
  if(data.tabId === activeTabId.value) {
    widthValue.value = data.mode;
  }
};

watch(activeUrl, () => {
  widthModeShow.value = activeUrl.value?.includes("weread.qq.com/web/reader") || false;
});

const gotoNewUrl = () => {
  if(!activeUrl.value) return;
  const index = tabs.value.findIndex(t => t.id === activeTabId.value);

  if (index >= 0) {
    tabs.value[index].url = activeUrl.value;
  }
  sendActionToBackend('openNewUrl', { "tabId": activeTabId.value, "url": activeUrl.value });
}

const onTabAdded = () =>{
  sendActionToBackend('addVoidTab', { });
}


const addTab = (tab: Tab) => {
  tabs.value.push(tab);
  if (tab.isShow) {
    activeTabId.value = tab.id;
    activeUrl.value = tabs.value.find(t => t.id === activeTabId.value)?.url;
    addressDisabled.value = tab.id === ID_FIRST_PAGE_ID;
  }
}

const changeTab = () => {
  sendActionToBackend('tabSelected', { "tabId": activeTabId.value });
  activeUrl.value = tabs.value.find(t => t.id === activeTabId.value)?.url;

  addressDisabled.value = activeTabId.value === ID_FIRST_PAGE_ID;
}


const closeTab = (id: string) => {
  const index = tabs.value.findIndex(t => t.id === id);
  if (index === -1) return;
  tabs.value.splice(index, 1);
  if (activeTabId.value === id && tabs.value.length > 0) {
    const tab = tabs.value[Math.max(0, index - 1)];
    activeTabId.value = tab.id;
    activeUrl.value = tab.url;
  }
  sendActionToBackend('tabClosed', { "closeTabId": id, "activeTabId": activeTabId.value });
};

const selectTab = (data: any) => {
  if(data.id === activeTabId.value){
    return;
  }
  const index = tabs.value.findIndex(t => t.id === data.id);
  if (index >= 0) {
    activeTabId.value = tabs.value[index].id;
    activeUrl.value = tabs.value[index].url;
    sendActionToBackend('tabSelected', { "tabId": activeTabId.value });
  }
};

const modifyTabTitle = (tabData:any) => {
  const { tabId, title } = tabData;
  const tab = tabs.value.find(t => t.id === tabId);
  if (tab) {
    tab.title = title;
  }
};

const modifyTabUrl = (tabData:any) => {
  const { tabId, url } = tabData;
  const tab = tabs.value.find(t => t.id === tabId);
  if (tab) {
    tab.url = url;
  }
  if(activeTabId.value == tabId){
    activeUrl.value = url;
  }
};

const isAddTab = (data:any) => {
  addable.value = data.isAdd;
};

/**
 * 通过后端全局显示提示信息
 * @param title
 * @param message
 */
// const alert = (title: string, message: string) => {
//   sendActionToBackend('alertShow', { "title": title, "message": message });
// };


// region 与后端交互

const actionDispatcher = {
  addTab,
  closeTab,
  modifyTabTitle,
  selectTab,
  isAddTab,
  modifyTabUrl,
  doWidthValue,
};

interface BackendActionPayload {
  action: string;
  data: any; // 这里的 data 也可以根据具体业务进一步细化
}

const sendActionToBackend = async (action: string, data: any) => {
  console.warn(`调用成功: ${action}, ${JSON.stringify(data)}`);
  await invoke('receive_front_top_action', { "action": action, "data": data });
};

let unlistenFn: (() => void) | null = null;
let idx:number = 1
onMounted(async () => {
  try {
    unlistenFn = await listen<BackendActionPayload>('top-view-action', (event) => {
      const { action, data } = event.payload;
      idx++;
      if (action && action in actionDispatcher) {
        const key = action as keyof typeof actionDispatcher;
        actionDispatcher[key](data);
      } else {
        console.warn(`未找到后端指定的 Action 处理方法: ${action}`);
      }
    });
    await sendActionToBackend('beReady', {});
  } catch (error) {
    console.error("监听器注册失败:", error);
  }
});

onUnmounted(() => {
  if (unlistenFn) unlistenFn();
});

// endregion 与后端交互

</script>

<style scoped>
html body{
  overflow: hidden;
}

.header{
  display: flex;
  flex-direction: column;
  width: 100vw;
  height: 100vh;
  background-color: #f6f7f9;
  overflow: hidden;
  user-select: none;
}

.header-row-1{
  display: flex;
  flex-direction: row;
  width: 99vw;
  height: 38px;
  overflow: hidden;
  user-select: none;
  margin-top: 2px;
  margin-bottom: 2px;
}

.header-row-2{
  display: flex;
  flex-direction: row;
  width: 99vw;
  height: 32px;
  overflow: hidden;
  user-select: none;
}

.tabs-container {
  width: calc(99vw - 18px);
  height: 100vh;
  overflow: hidden;
  user-select: none;
}

.settings-container{
  width: 18px;
  height: 100vh;
  overflow: hidden;
  user-select: none;
  padding-top: 12px;
}

.address-container{
  width: 60vw;
}

.style-container{
  text-align: right;
  width: calc(99vw - 600px);
}

.address-input{
  width:600px;
  margin-left: 5px;
}
</style>
