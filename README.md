# 偷偷读

方便自己上班时看点书。

## 介绍
1、主要功能就鼠标离开阅读窗口时，窗口会自动隐藏，并在屏幕顶部留有一条小边缘。  
当鼠标移动到这个小边缘时，阅读窗口会自动显示。  

2、可以打开多个标签同时看多本书。

3、可以加宽微信读书的阅读区域。  

4、只支持Windows

5、自动保存书籍阅读记录，下次打开时会从上次离开的地方继续阅读。


## 开发方式  
#### 初始化
  bun install  

#### 开发测试运行
  bun run tauri dev  

#### 还未考虑在 Android 上的运行
  bun run tauri android dev  

#### 为所有平台打包
  bun run tauri build  

#### 只为 Windows 打包
##### 如果不需要任何安装包，只想得到一个可直接运行的绿色 .exe 文件，使用 --no-bundle 跳过所有打包步骤。
  bun run tauri build --target x86_64-pc-windows-msvc --no-bundle 


#### 记录 
1、在 src-tauri 文件夹中运行 cargo update 将获取所有依赖项的最新可用语义兼容版本。   
2、建立图标 bun run tauri icon WendaReader.png  