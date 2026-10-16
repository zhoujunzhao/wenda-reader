# 微信爱读书

方便自己上班时阅读，也可以分享给朋友。

## 介绍
主要功能就是窗口拖动到屏幕顶部边缘时，会自动隐藏，并留有一条小边缘。  
当鼠标移动到这个小边缘时，会自动显示。  
鼠标离开阅读区域时，窗体会自动隐藏。  




### 初始化
  bun install  

### 开发测试运行
  bun run tauri dev  

### 还未考虑在 Android 上的运行
  bun run tauri android dev  

### 为所有平台打包
  bun run tauri build  

### 只为 Windows 打包
#### 如果不需要任何安装包，只想得到一个可直接运行的绿色 .exe 文件，使用 --no-bundle 跳过所有打包步骤。
  bun run tauri build --target x86_64-pc-windows-msvc --no-bundle 