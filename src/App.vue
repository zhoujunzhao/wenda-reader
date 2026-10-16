<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

const greetMsg = ref("");
const name = ref("");

const tabs = ref([
  { id: 'tab-home', title: '微信爱读书', url: 'https://weread.qq.com/' }
]);
const activeTabId = ref('tab-home');

const createNewTab = (url: string, title: string) => {
  const uniqueId = `tab-reader-${Date.now()}`;
  tabs.value.push({ id: uniqueId, title: title || '阅读书籍', url });
  activeTabId.value = uniqueId;
};

const switchTab = (id: string) => { activeTabId.value = id; };

const closeTab = (id: string) => {
  const index = tabs.value.findIndex(t => t.id === id);
  if (index === -1) return;
  tabs.value.splice(index, 1);
  if (activeTabId.value === id && tabs.value.length > 0) {
    activeTabId.value = tabs.value[Math.max(0, index - 1)].id;
  }
};

// 转换网络路径到主进程的透明代理协议
const convertToProxyUrl = (url: string) => {
  return url.replace("https://", "weread-proxy://localhost/");
};

// 核心：拦截并劫持 iframe 内部超链接点击和单页路由变化
const injectIframeIntercepter = (event: any, tabId: any) => {
  try {
    const iframeWin = event.target.contentWindow;
    const iframeDoc = event.target.contentDocument || iframeWin.document;

    // 1. 劫持内部 A 标签点击
    iframeDoc.addEventListener('click', (e: { target: { closest: (arg0: string) => any; }; preventDefault: () => void; }) => {
      let target = e.target.closest('a');
      if (target && target.href) {
        const rawUrl = target.href.replace("weread-proxy://localhost/", "https://");
        if (rawUrl.includes('://qq.com')) {
          e.preventDefault();
          createNewTab(rawUrl, target.innerText.trim());
        }
      }
    }, true);

    // 2. 劫持内部 window.open
    iframeWin.open = (url: string) => {
      if (url) {
        const rawUrl = url.replace("weread-proxy://localhost/", "https://");
        createNewTab(rawUrl, '新标签页');
      }
      return null;
    };

    // 3. 监听内部 SPA 路由的改变（检测看书动作）
    let lastUrl = iframeWin.location.href;
    setInterval(() => {
      if (iframeWin.location.href !== lastUrl) {
        const oldUrl = lastUrl;
        lastUrl = iframeWin.location.href;
        if (lastUrl.includes('/web/reader/') && !oldUrl.includes('/web/reader/')) {
          const rawUrl = lastUrl.replace("weread-proxy://localhost/", "https://");
          iframeWin.history.replaceState(null, '', oldUrl); // 原位保留
          createNewTab(rawUrl, '读书中...');
        }
      }
    }, 400);

  } catch (err) {
    console.warn("跨域拦截前置触发（加载中）");
  }
};

async function greet() {
  // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
  greetMsg.value = await invoke("greet", { name: name.value });
}
</script>

<template>
  <div class="app-container">
    <!-- 顶部自定义导航操作区 (45px 严格限制) -->
    <div class="tab-bar-wrapper" data-tauri-drag-region>

      <!-- 左侧：多标签页卡片 -->
      <div class="tabs-scroll-area" @mousedown.stop>
        <div
            v-for="tab in tabs"
            :key="tab.id"
            class="tab-item"
            :class="{ 'active': activeTabId === tab.id }"
            @click="switchTab(tab.id)"
        >
          <span class="tab-title">{{ tab.title }}</span>
          <span
              v-if="tabs.length > 1"
              class="tab-close-btn"
              @click.stop="closeTab(tab.id)"
          >✕</span>
        </div>
      </div>
    </div>

    <!-- 底部微信读书多视窗隔离容器 (Vue 完全接管，不再遭遇 Rust 内核死锁) -->
    <div class="webview-container-host">
      <iframe
          v-for="tab in tabs"
          :key="tab.id"
          v-show="activeTabId === tab.id"
          :src="convertToProxyUrl(tab.url)"
          class="book-iframe-bridge"
          @load="injectIframeIntercepter($event, tab.id)"
      ></iframe>
    </div>
  </div>
</template>

<style scoped>
.app-container {
  display: flex;
  flex-direction: column;
  width: auto;
  height: auto;
  background-color: #f6f7f9;
  overflow: hidden;
  user-select: none;
}

.tab-bar-wrapper {
  display: flex;
  align-items: center;
  height: 45px;
  background-color: #ffffff;
  border-bottom: 1px solid #e3e5e7;
  padding-left: 12px;
  padding-right: 0;
  box-sizing: border-box;
}

.tabs-scroll-area {
  display: flex;
  gap: 6px;
  overflow-x: auto;
  max-width: 65%;
  align-items: center;
  height: 100%;
}

.tabs-scroll-area::-webkit-scrollbar { display: none; }

.tab-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 14px;
  height: 32px;
  background-color: #f1f2f3;
  border-radius: 6px;
  font-size: 13px;
  color: #61666d;
  cursor: pointer;
  white-space: nowrap;
  max-width: 150px;
}

.tab-item.active { background-color: #007aff; color: #ffffff; }

.tab-close-btn { font-size: 10px; padding: 2px; border-radius: 50%; }
.tab-close-btn:hover { background-color: rgba(0, 0, 0, 0.15); }

/* 底部网页沙箱宿主容器 */
.webview-container-host {
  flex: 1;
  width: 100%;
  height: calc(100vh - 45px);
  background-color: #ffffff;
}

.book-iframe-bridge {
  width: 100%;
  height: 100%;
  border: none;
  outline: none;
}
</style>
