<script setup lang="ts">
import {onMounted, onUnmounted, ref} from "vue"
import {listen} from "@tauri-apps/api/event";
import {invoke} from "@tauri-apps/api/core";


const sidePositionValue = ref<string>("TopCenter");

const sidePositionChange = (e: Event) => {
  sidePositionValue.value = (e.target as HTMLInputElement).value;
  sendActionToBackend('setSidePosition', { "sidePosition": sidePositionValue.value });
};

const onSettingLoad = (data:any) => {
  if(data.sidePosition){
    sidePositionValue.value = data.sidePosition;
  }
};

// region 与后端交互

const actionDispatcher = {
  onSettingLoad,
};

interface BackendActionPayload {
  action: string;
  data: any; // 这里的 data 也可以根据具体业务进一步细化
}

const sendActionToBackend = async (action: string, data: any) => {
  await invoke('receive_front_setting_action', { "action": action, "data": data });
};

let unlistenFn: (() => void) | null = null;
let idx:number = 1
onMounted(async () => {
  try {
    console.warn(`ini调用成功`);
    unlistenFn = await listen<BackendActionPayload>('setting-view-action', (event) => {
      const { action, data } = event.payload;
      console.warn(`调用成功:${idx}: ${JSON.stringify(event.payload)}`);
      idx++;
      if (action && action in actionDispatcher) {
        const key = action as keyof typeof actionDispatcher;
        actionDispatcher[key](data);
      } else {
        console.warn(`未找到后端指定的 Action 处理方法: ${action}`);
      }
    });
    await sendActionToBackend('beSettingReady', {});
  } catch (error) {
    console.error("监听器注册失败:", error);
  }
});

onUnmounted(() => {
  if (unlistenFn) unlistenFn();
});

// endregion 与后端交互
</script>

<template>
  <n-card title="隐藏位置">
    <n-radio
        :checked="sidePositionValue === 'TopLeft'"
        value="TopLeft" label="顶部靠左" name="sidePosition" @change="sidePositionChange"
    />
    <n-radio
        :checked="sidePositionValue ==='TopCenter'"
        value="TopCenter" label="顶部中间" name="sidePosition" @change="sidePositionChange"
    />
    <n-radio
        :checked="sidePositionValue === 'TopRight'"
        value="TopRight" label="顶部靠右" name="sidePosition" @change="sidePositionChange"
    />
    <n-radio
        :checked="sidePositionValue ==='Left'"
        value="Left" label="左边" name="sidePosition" @change="sidePositionChange"
    />
    <n-radio
        :checked="sidePositionValue === 'Right'"
        value="Right" label="右边" name="sidePosition" @change="sidePositionChange"
    />
  </n-card>
</template>

<style scoped>

</style>