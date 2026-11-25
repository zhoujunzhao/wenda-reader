use std::collections::HashMap;
use std::string::ToString;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::str::FromStr;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Webview, WebviewUrl, Window, Wry, async_runtime::JoinHandle, RunEvent};
use tauri::webview::{NewWindowResponse, PageLoadEvent};

use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_store::{Store, StoreExt};

use windows_sys::Win32::Foundation::{HWND, POINT};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetAncestor, GetCursorPos, GetWindow, WindowFromPoint, GA_ROOT, GW_OWNER, GetWindowThreadProcessId, GetClassNameW};

use chrono::{Utc};

use serde::{Deserialize, Serialize};
use serde_json::{json};


// 定义常数（基于逻辑像素）
const HIDE_REMAIN: f64 = 1.0; // 隐藏时留下的边缘高度
const TOP_VIEW_HEIGHT: f64 = 70.0; // 顶部视图高度
const CONFIG_FILE_NAME: &str = "settings.json"; // 配置文件名
const WECHAT_READER_DOMAIN: &str = "://qq.com"; // 用于判断是否为微信读书域名
const SCROLL_TITLE_PREFIX: &str = "__WENDA_SCROLL__:"; // 内部传递滚动坐标的临时标记

const MAX_TAB_COUNT: u32 = 9; // 最大标签页数 9为数之极

const ID_FIRST_PAGE_ID: &str = "tab-main"; // 需要同时修改前端
const ID_FIRST_PAGE_TITLE: &str = "书架";
const ID_FIRST_PAGE_URL: &str = "https://weread.qq.com/";
const WECHAT_READER_URL:&str = "weread.qq.com/web/reader";

const SIDE_WINDOW_SIZE:LogicalSize<f64> = LogicalSize::new(300.0, 300.0);

const BLANK_PAGE:&str = "about:blank";

const ID_TOP_VIEW: &str = "top-view";

const WINDOW_TITLE : &str = "偷偷读";

const SETTING_PAGE_URL:&str = "/setting";

#[derive(Default)]
pub struct TaskRegistry(pub Mutex<Vec<JoinHandle<()>>>);

impl TaskRegistry {
    // 注册单个异步任务句柄
    pub fn add(&self, handle: JoinHandle<()>) {
        if let Ok(mut list) = self.0.lock() {
            list.push(handle);
        }
    }

    // 批量中止所有任务
    pub fn abort_all(&self) {
        if let Ok(mut list) = self.0.lock() {
            for handle in list.drain(..) {
                handle.abort();
            }
            log::info!("所有后台异步任务已全部终止");
        }
    }
}

/// 边条窗体位置
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
enum SidePosition {
    TopLeft,
    #[default]
    TopCenter,TopRight
    ,Left,Right
}

impl FromStr for SidePosition {
    type Err = String; // 转换失败时的错误类型

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // 建议统一转小写匹配，增强容错性
        match s.trim().to_lowercase().as_str() {
            "topleft" => Ok(SidePosition::TopLeft),
            "topcenter" => Ok(SidePosition::TopCenter),
            "topright" => Ok(SidePosition::TopRight),
            "left" => Ok(SidePosition::Left),
            "right" => Ok(SidePosition::Right),
            _ => Err(format!("未知的隐藏边条位置: {}", s)),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
enum TabAttrName{
    Title,
    Url,
    Closable,
    ReaderMode,
    ScrollPos,
}

// region 结构声明
#[derive(Debug, Serialize, Deserialize, Clone)]
struct Tab {
    id: String,
    title: String,
    url: String,
    closable: bool,
    reader_mode: String, // 阅读宽度模式，只能微信读书有效。
    #[serde(default)]
    scroll_top: f64, // 新增：滚动条纵向位置（仅非微信读书网页有效）
    #[serde(default)]
    scroll_selector: String, // 新增：局部滚动容器选择器
}

impl Tab {
    fn new(_id: &str, _title: &str, _url: &str, _closable: bool, _reader_mode: &str) -> Tab {
        Self {
            id: _id.to_string(),
            title: _title.to_string(),
            url: _url.to_string(),
            closable: _closable,
            reader_mode: if _reader_mode.is_empty() { "default".to_string() } else {_reader_mode.to_string()},
            scroll_top: 0.0,
            scroll_selector: String::new(),
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct AppData {
    config_file_path: String,               // 配置文件保存路径
    window_size: LogicalSize<f64>,
    selected_tab_id: String,                // 当前选中的标签页 ID
    open_tabs: HashMap<String, Tab>,        // 已打开的标签页
    side_position: SidePosition,            // 边条的位置
    is_update:bool,                         // AppData中的数据是否有变更，用于判断是否需要保存配置文件，true:有变更，false:无变更，默认false
}

struct AppGlobalState {
    inner: Mutex<AppData>,
}

impl AppGlobalState {
    /// 初始化
    fn new(initial_data: AppData) -> Self {
        Self {
            inner: Mutex::new(initial_data),
        }
    }

    /// 获取全部数据
    #[allow(dead_code)]
    fn get(&self) -> AppData {
        self.inner.lock().unwrap().clone()
    }

    fn get_config_file_path(&self) -> String {
        self.inner.lock().unwrap().config_file_path.clone()
    }

    fn set_config_file_path(&self, path: &str){
        self.inner.lock().unwrap().config_file_path = path.parse().unwrap();
    }

    fn get_window_size(&self) -> LogicalSize<f64> {
        self.inner.lock().unwrap().window_size.clone()
    }

    fn set_window_size(&self, size: LogicalSize<f64>) {
        let mut inner = self.inner.lock().unwrap();
        inner.window_size = size;
        inner.is_update = true;
    }

    fn get_selected_tab_id(&self) -> String {
        self.inner.lock().unwrap().selected_tab_id.clone()
    }
    fn set_selected_tab_id(&self, tab_id: &str) {
        let mut inner = self.inner.lock().unwrap();
        inner.selected_tab_id = tab_id.parse().unwrap();
        inner.is_update = true;
    }

    fn get_open_tabs(&self) -> HashMap<String, Tab> {
        self.inner.lock().unwrap().open_tabs.clone()
    }

    fn get_open_tabs_tab(&self, tab_id: &str) -> Option<Tab> {
        self.inner.lock().unwrap().open_tabs.get(tab_id).cloned()
    }

    fn get_open_tabs_count(&self) -> i32 {
        self.inner.lock().unwrap().open_tabs.len() as i32
    }

    fn set_open_tabs_add(&self, tab_id: &str, tab: Tab, _is_update:bool){
        let mut inner = self.inner.lock().unwrap();
        let tabs = &mut inner.open_tabs;
        tabs.entry(tab_id.parse().unwrap()).and_modify(|e| *e = tab.clone()).or_insert(tab.clone());
        inner.is_update = _is_update;
    }

    fn set_open_tabs_modify_val(&self, tab_id: &str, attr_name: TabAttrName, val:serde_json::Value){
        if let Some(tab) = self.get_open_tabs_tab(tab_id) {
            let mut tab = tab.clone();
            match attr_name {
                TabAttrName::Title => tab.title = serde_json::from_value(val).unwrap(),
                TabAttrName::Url => tab.url = serde_json::from_value(val).unwrap(),
                TabAttrName::Closable => tab.closable = serde_json::from_value(val).unwrap(),
                TabAttrName::ReaderMode => tab.reader_mode = serde_json::from_value(val).unwrap(),
                TabAttrName::ScrollPos => {
                    if let (Some(top), Some(selector)) = (
                        val.get("top").and_then(|v| v.as_f64()),
                        val.get("selector").and_then(|v| v.as_str()),
                    ) {
                        tab.scroll_top = top;
                        tab.scroll_selector = selector.to_string();
                    }
                },
            }
            self.set_open_tabs_add(tab_id, tab, true);
        }
    }

    fn set_open_tabs_remove(&self, tab_id: &str) -> Option<Tab> {
        let mut inner = self.inner.lock().unwrap();
        if inner.open_tabs.contains_key(tab_id) {
            let removed_tab = inner.open_tabs.remove(tab_id);
            inner.is_update = true;
            return removed_tab
        }
        None
    }

    fn set_open_tabs_clear(&self){
        let mut inner = self.inner.lock().unwrap();
        inner.open_tabs.clear();
        inner.is_update = true;
    }

    fn get_side_position(&self) -> SidePosition {
        self.inner.lock().unwrap().side_position.clone()
    }

    fn set_side_position(&self, side_position: SidePosition) {
        let mut inner = self.inner.lock().unwrap();
        inner.side_position = side_position;
    }

    fn get_is_update(&self) -> bool{
        self.inner.lock().unwrap().is_update
    }

    fn set_is_update(&self, is_update: bool){
        self.inner.lock().unwrap().is_update = is_update;
    }
}

// endregion 结构声明

pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(tauri_plugin_log::log::LevelFilter::Info)
                .build(),
        )
        .manage(AppGlobalState::new(AppData{
            config_file_path: CONFIG_FILE_NAME.to_string(),
            window_size: LogicalSize::new(1200.0, 1000.0),
            selected_tab_id: ID_FIRST_PAGE_ID.parse().unwrap(),
            open_tabs: HashMap::new(),
            side_position: SidePosition::TopCenter,
            is_update: false,
        }))
        .setup(move |app| {
            let mut main_window_size = LogicalSize::new(1200.0, 1000.0);

            // region 应用参数初始化
            let app_handle = app.handle();
            let app_store = app_handle.state::<AppGlobalState>();

            if let Ok(local_data_dir) = app_handle.path().app_local_data_dir() {
                app_store.set_config_file_path(&*local_data_dir
                    .join("config")
                    .join(CONFIG_FILE_NAME)
                    .into_string()
                    .unwrap()
                );
            }

            let _app = app_handle.clone();

            let (screen_width, screen_height) = get_screen_size(_app);
            if let Some(his_size) = read_window_size(&app_handle) {
                main_window_size = his_size;
            } else {
                main_window_size = LogicalSize::new(main_window_size.width, screen_height - 100.0);
                app_store.set_window_size(main_window_size);
            }

            // endregion 应用参数初始化

            let _side_x = screen_width / 2.0 - SIDE_WINDOW_SIZE.width / 2.0;
            let side_window = tauri::window::WindowBuilder::new(app, "side_window")
                .always_on_top(true)
                .inner_size(SIDE_WINDOW_SIZE.width, SIDE_WINDOW_SIZE.height)
                .maximizable(false)
                .minimizable(false)
                .decorations(false)
                .shadow(false)
                .closable(false)
                .resizable(false)
                .skip_taskbar(true)
                .visible(false)
                .build()?;

            // region 托盘
            let open_cfg_i = MenuItem::with_id(app, "tray-open-config", "打开配置目录", true, None::<&str>).unwrap();
            let quit_i = MenuItem::with_id(app, "tray-quit", "退出", true, None::<&str>).unwrap();
            // let show_i = MenuItem::with_id(app, "show", "显示主界面", true, None::<&str>).unwrap();
            let menu = Menu::with_id_and_items(app, "tray-menu", &[&open_cfg_i,&quit_i]).unwrap();
            let _tray = TrayIconBuilder::new()
                // 请确保你的 src-tauri/icons 目录下有对应的托盘图标（如 tray.png 或 icon.ico）
                .icon(app.default_window_icon().unwrap().clone())
                .show_menu_on_left_click(false)
                .menu(&menu)
                .tooltip(WINDOW_TITLE)
                .on_menu_event(|app_handle, event| {
                    match event.id.as_ref() {
                        "tray-open-config" => {
                            let config_path = app_handle.state::<AppGlobalState>().get_config_file_path();
                            let path = std::path::Path::new(&config_path);

                            // 如果配置文件已存在，打开文件夹并高亮选中该文件；否则直接打开其所在目录
                            if path.exists() {
                                let _ = std::process::Command::new("explorer")
                                    .arg(format!("/select,{}", config_path))
                                    .spawn();
                            } else if let Some(parent_dir) = path.parent() {
                                let _ = std::fs::create_dir_all(parent_dir); // 确保目录存在
                                let _ = std::process::Command::new("explorer")
                                    .arg(parent_dir)
                                    .spawn();
                            }
                        }
                        "tray-quit" => {
                            for i in 1..=MAX_TAB_COUNT {
                                if let Some(webview) = app_handle.get_webview(&get_tab_id(i)) {
                                    capture_webview_scroll(&webview);
                                }
                            }
                            std::thread::sleep(Duration::from_millis(150));

                            flush_config_to_disk(app_handle);

                            if let Some(_) = app_handle.tray_by_id("tray-menu") {
                                let _ = app_handle.remove_tray_by_id("tray-menu");
                            }
                            let windows = app_handle.webview_windows();
                            for (_name, window) in windows {
                                let _ = window.close();
                            }

                            if let Some(window) = app_handle.get_window("side_window") {
                                let _ = window.close();
                            }
                            if let Some(window) = app_handle.get_window("main") {
                                let _ = window.close();
                            }

                            app_handle.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    match event {
                        TrayIconEvent::DoubleClick { button, .. } => {
                            // 2. 确认触发双击的是鼠标左键
                            if button == MouseButton::Left {
                                if let Some(window) = tray.app_handle().get_window("main") {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                        }
                        _ => {}
                    }
                })
                .build(app)
                .unwrap();
            // endregion 托盘

            // region 主窗口 操作
            let main_window = tauri::WindowBuilder::new(app, "main")
                .title(WINDOW_TITLE)
                .inner_size(main_window_size.width, main_window_size.height)
                .always_on_top(true)
                .center()
                .maximizable(false)
                .minimizable(false)
                .decorations(true)
                .closable(false)
                .skip_taskbar(true)
                .build()
                .expect("创建空白窗口失败。");

            let _window = main_window.clone();
            main_window.on_window_event(move |event| {
                if let tauri::WindowEvent::Resized(new_size) = event {
                    for webview in _window.webviews() {
                        if webview.label() == ID_TOP_VIEW {
                            webview.set_size(LogicalSize::new(new_size.width, TOP_VIEW_HEIGHT as u32)).unwrap();
                        } else {
                            if webview.size().unwrap().height > 1 && webview.size().unwrap().width > 1 {
                                webview.set_size(LogicalSize::new(
                                        new_size.width,
                                        new_size.height - TOP_VIEW_HEIGHT as u32,
                                    ))
                                    .unwrap();
                            }
                        }
                    }
                    let _size = LogicalSize::new(new_size.width as f64, new_size.height as f64);
                    _window.app_handle().state::<AppGlobalState>().set_window_size(_size);
                }
            });

            let _side_position = get_side_position(main_window.clone());

            set_side_position(&side_window, _side_position, screen_width, screen_height);

            // endregion 主窗口 操作

            // region webview 操作
            let _top_webview = main_window.add_child(
                tauri::webview::WebviewBuilder::new(
                    ID_TOP_VIEW,
                    WebviewUrl::App(Default::default()),
                )
                .devtools(true)
                .auto_resize(),
                LogicalPosition::new(0.0, 0.0),
                LogicalSize::new(main_window_size.width, TOP_VIEW_HEIGHT),
            )?;

            let _window = main_window.clone();
            let _window2 = main_window.clone();
            main_window.add_child(
                tauri::webview::WebviewBuilder::new(
                    ID_FIRST_PAGE_ID,
                    WebviewUrl::External(ID_FIRST_PAGE_URL.parse().unwrap()),
                )
                .devtools(true)
                .auto_resize()
                .on_navigation(move |url: &tauri::Url| {
                    let (state, tab_id) = is_open_new_tab(&_window, url.to_string());
                    return if state == 0 {
                        // 表示在当前webview打开
                        true
                    } else if state == 1 {
                        // 表示在新标签面打开，同时阻断当前webview打开
                        let tab = Tab::new("", "加载中", url.as_str(), true, "default");
                        add_tab(&_window, tab, false,false);
                        send_is_tab_add(&_window, -1);
                        false
                    } else if state == 2 {
                        // 表示激活已存在的标签，同时阻断当前webview打开
                        let payload = json!({
                            "id": tab_id
                        });
                        send_action_to_top_view(&_window, "selectTab", payload);
                        false
                    } else {
                        true
                    };
                })
                .on_new_window(move |new_url, _features| {
                    if let Some(web_view) = _window2.get_webview(ID_FIRST_PAGE_ID){
                        web_view.navigate(new_url.clone()).unwrap();
                        let payload = json!({
                            "tabId": ID_FIRST_PAGE_ID,
                            "url": new_url.as_str(),
                        });
                        send_action_to_top_view(&_window2, "modifyTabUrl", payload);
                    }
                    return NewWindowResponse::Deny;
                }),

                LogicalPosition::new(0.0, TOP_VIEW_HEIGHT),
                LogicalSize::new(main_window_size.width,main_window_size.height - TOP_VIEW_HEIGHT),
            )?;
            let _window = main_window.clone();
            for i in 1..=MAX_TAB_COUNT {
                let _window = main_window.clone();
                let webview_label = get_tab_id(i);

                let label_for_closure = webview_label.clone();
                let _app_handle_for_closure = app_handle.clone();

                // region 创建子webview
                let _sub_webview = main_window.add_child(
                    tauri::webview::WebviewBuilder::new(
                        webview_label,
                        WebviewUrl::External(BLANK_PAGE.parse().unwrap()),
                    )
                    .devtools(true)
                    .initialization_script(include_str!("../scripts/wenda_reader.js"))
                        .on_document_title_changed(|webview, title| {
                            // 1. 拦截内部滚动坐标回传标记（优先处理，不触发任何其他 Webview 查询）
                            if let Some(payload_str) = title.strip_prefix(SCROLL_TITLE_PREFIX) {
                                if let Ok(scroll_val) = serde_json::from_str::<serde_json::Value>(payload_str) {
                                    let state = webview.app_handle().state::<AppGlobalState>();
                                    state.set_open_tabs_modify_val(
                                        webview.label(),
                                        crate::TabAttrName::ScrollPos,
                                        scroll_val,
                                    );
                                }
                                return;
                            }

                            if title.is_empty() || title == BLANK_PAGE {
                                return;
                            }

                            // 安全获取 URL，避免 COM 回调重入或返回 Err 时 unwrap 崩溃
                            if let Ok(current_url) = webview.url() {
                                if current_url.as_str().ends_with(SETTING_PAGE_URL) {
                                    return;
                                }
                            }

                            let label = webview.label().to_string();
                            let app_handle = webview.app_handle().clone();

                            let payload = json!({
                            "tabId": label,
                            "title": title,
                        });

                            let state = app_handle.state::<AppGlobalState>();
                            state.set_open_tabs_modify_val(
                                &label,
                                crate::TabAttrName::Title,
                                serde_json::Value::String(title.clone()),
                            );

                            if let Some(window) = app_handle.get_window("main") {
                                send_action_to_top_view(&window, "modifyTabTitle", payload);
                                modify_window_title(window, &label, &title);
                            }
                        })

                        .on_navigation(move |url: &tauri::Url| {
                        let _win = _window.clone();
                        log::info!("dao：{}",url);
                        let _url = url.as_str();

                        let state = _app_handle_for_closure.state::<AppGlobalState>();
                        if let Some(tab) = state.get_open_tabs_tab(&label_for_closure){
                            let tab_url = tab.url.clone();
                            if tab_url.to_lowercase() != _url.to_lowercase() {
                                log::info!("url发生了变化：{} -> {}", tab_url, _url);
                                state.set_open_tabs_modify_val(&label_for_closure, crate::TabAttrName::Url, serde_json::Value::String(_url.parse().unwrap()));

                                state.set_open_tabs_modify_val(
                                    &label_for_closure,
                                    crate::TabAttrName::ScrollPos,
                                    json!({ "top": 0.0, "selector": "" }),
                                );

                                let payload = json!({
                                    "tabId": label_for_closure,
                                    "url": _url,
                                });
                                send_action_to_top_view(&_win, "modifyTabUrl", payload);
                            }
                        }
                        true
                    })
                    .on_page_load(|webview, payload| {
                        if let PageLoadEvent::Finished = payload.event() {
                            if let Ok(current_url) = webview.url() {
                                let url_str = current_url.as_str();
                                if url_str.contains(WECHAT_READER_URL) {
                                    // 微信读书：恢复阅读宽度模式
                                    if let Some(tab) = webview
                                        .app_handle()
                                        .state::<AppGlobalState>()
                                        .get_open_tabs_tab(webview.label())
                                    {
                                        let mode = tab.reader_mode;
                                        let js_code = format!("update_wenda_style('{}');", mode);
                                        let _ = webview.eval(&js_code);
                                    }
                                } else if should_record_scroll(url_str) {
                                    // 非微信读书网页：如果该 Tab 记录了滚动位置，则自动恢复
                                    if let Some(tab) = webview
                                        .app_handle()
                                        .state::<AppGlobalState>()
                                        .get_open_tabs_tab(webview.label())
                                    {
                                        if tab.scroll_top > 0.0 {
                                            let selector_json =
                                                serde_json::to_string(&tab.scroll_selector)
                                                    .unwrap_or_else(|_| "\"\"".to_string());
                                            let js_code = format!(
                                                "if (typeof wenda_restore_scroll === 'function') {{ wenda_restore_scroll({}, {}); }}",
                                                tab.scroll_top, selector_json
                                            );
                                            let _ = webview.eval(&js_code);
                                        }
                                    }
                                }
                            }

                        }
                    })
                    .auto_resize(),
                    LogicalPosition::new(0.0, TOP_VIEW_HEIGHT),
                    LogicalSize::new(1,1,
                    ),
                )?;
                // endregion 创建子webview

                _sub_webview.hide().unwrap();
                _sub_webview.set_size(LogicalSize::new(
                    main_window_size.width,
                    main_window_size.height - TOP_VIEW_HEIGHT,
                )).unwrap();
            }
            // endregion webview 操作

            // 初始化硬件状态查询器
            // let device_state = DeviceState::new();

            // 记录上一次的状态，避免控制台疯狂刷屏重复日志
            let mut last_valid_status = false;
            let mut trigger_show_time = get_timestamp() - 1000;

            let tab_app_handle = app_handle.clone();

            let registry = TaskRegistry::default();

            // 异步写入配置
            let task_write_config = tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_secs(60)).await;
                loop {
                    for i in 1..=MAX_TAB_COUNT {
                        if let Some(webview) = tab_app_handle.get_webview(&get_tab_id(i)) {
                            capture_webview_scroll(&webview);
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(200)).await;

                    flush_config_to_disk(&tab_app_handle);

                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            });

            registry.add(task_write_config);

            #[cfg(target_os = "windows")]
            let hwnd_main_raw = main_window.hwnd().unwrap().0 as isize;
            let hwnd_side_raw = side_window.hwnd().unwrap().0 as isize;

            // 开启纯 Rust 的后台异步轮询线程
            let task_window_position = tauri::async_runtime::spawn(async move {
                loop {
                    match main_window.is_always_on_top() {
                        Ok(false) => { let _ = main_window.set_always_on_top(true); }
                        Err(_) => break,
                        _ => {}
                    }
                    match side_window.is_always_on_top() {
                        Ok(false) => { let _ = side_window.set_always_on_top(true); }
                        Err(_) => break,
                        _ => {}
                    }

                    let is_top = is_window_under_mouse(hwnd_main_raw, hwnd_side_raw);

                    if is_top != last_valid_status && (get_timestamp() - trigger_show_time) > 800
                    {
                        if is_top {
                            // 鼠标在当前窗体内
                            trigger_show_time = get_timestamp();

                            main_window.show().expect("设置主窗体显示时失败！");
                            side_window.hide().expect("边条窗体设置显隐藏失败！");
                        } else {
                            side_window.show().expect("边条窗体设置显示失败！");
                            main_window.hide().expect("主窗体设置隐藏失败！");
                        }
                        last_valid_status = is_top;
                    }

                    tokio::time::sleep(Duration::from_millis(80)).await;
                }
            });

            registry.add(task_window_position);

            app.manage(registry);

            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_log::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            receive_front_top_action,
            receive_front_setting_action
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app_handle, event| {
            if let RunEvent::ExitRequested { .. } = event {
                if let Some(registry) = app_handle.try_state::<TaskRegistry>() {
                    registry.abort_all();
                }
            }
        });
}

fn alert_show(window: Window, title: &str, message: &str) {
    let new_title = if title.is_empty() { "提示" } else { title };
    window
        .dialog()
        .message(message) // 消息内容
        .title(new_title) // 标题
        .parent(&window)
        .kind(MessageDialogKind::Info) // 弹窗类型（图标）
        .show(|_| {});
}

/// 通过给定的 URL，判断是否需要在新的标签页打开
/// 返回 0 表示在当前webview打开
/// 返回 1 表示在新标签面打开，同时阻断当前webview打开
/// 返回 2 表示激活已存在的标签，同时阻断当前webview打开
/// 返回的第二个参数为 tabId
fn is_open_new_tab(window: &Window, url: String) -> (i8, String) {
    let mut target_url = url.clone().trim().to_lowercase();
    if target_url.ends_with("/") {
        target_url = url.strip_suffix("/").unwrap().to_string();
    }
    let state = window.app_handle().state::<AppGlobalState>();
    let tabs = state.get_open_tabs();
    for tab in tabs.values() {
        if tab.url.trim().to_lowercase() == target_url {
            // 这本书已经在标签页中
            return (2, tab.id.clone());
        }
    }
    if target_url.contains(WECHAT_READER_URL) {
        (1, "".to_string())
    } else {
        (0, "".to_string())
    }
}

/// 获取屏幕宽高，返回的是逻辑像素
fn get_screen_size(app: AppHandle) -> (f64, f64) {
    if let Ok(Some(monitor)) = app.primary_monitor() {
        let size = monitor.size();
        let scale_factor = monitor.scale_factor();

        // 物理像素（如 1920x1080）
        let phys_w = size.width as f64;
        let phys_h = size.height as f64;

        // 💡 联动前文高分屏避坑：将其转换为网页/布局通用的【逻辑像素】
        let logical_w = phys_w/ scale_factor;
        let logical_h = phys_h/ scale_factor;
        (logical_w, logical_h) // 返回计算好的逻辑宽高
    } else {
        (1920.0, 1080.0)
    }
}

// region tab操作
///
/// is_init: 是否是初始化时添加的标签页,true:初始化时添加，false:非初始化时添加
fn add_tab(window: &Window, mut tab: Tab, is_show: bool,is_init:bool) {
    let (tab_id, webview) = get_idle_webview(window, tab.id.clone());
    if !tab_id.is_empty() && webview.is_some() {
        tab.id = tab_id;
        webview.unwrap().navigate(tab.url.parse().unwrap()).unwrap();
        let _tab = tab.clone();
        let state = window.app_handle().state::<AppGlobalState>();
        state.set_open_tabs_add(&*_tab.id, _tab.clone(), !is_init);

        let _tab_id = tab.id.clone();
        if is_show{
            switch_webview(window.clone(), "".parse().unwrap(), _tab_id, false);
        }
        // 添加前端标签
        let mut payload = json!(tab);
        payload["isShow"] = json!(is_show);
        send_action_to_top_view(window, "addTab", payload);
    }
}

/// 判断标签页数量是否达到了最大值
/// tabs_size: 标签页数量，当小于0时，方法会自己从全局状态中获取标签页数量
/// 返回 true 表示未达到最大值，false 表示已达到或者超过最大值
fn send_is_tab_add(window: &Window,mut tabs_size:i32) {
    if tabs_size < 0 {
        tabs_size = window.app_handle().state::<AppGlobalState>().get_open_tabs_count();
    }
    let payload = json!({
                            "isAdd": tabs_size < MAX_TAB_COUNT as i32,
                        });
    let _win = window.clone();
    send_action_to_top_view(&_win, "isAddTab", payload);
}
// endregion tab操作

// region 前端动作处理

// region TopView前端动作处理

fn send_action_to_top_view(window: &Window, event_name: &str, data: serde_json::Value) {
    let payload = json!({
        "action": event_name,
        "data": data
    });
    if window.get_webview(ID_TOP_VIEW).is_none() {
        return;
    }

    window
        .app_handle()
        .emit_to(ID_TOP_VIEW, "top-view-action", payload)
        .unwrap();
}

///
/// 接收前端TopView动作
#[tauri::command(async)]
fn receive_front_top_action(window: Window, action: String, data: serde_json::Value) {

    match action.as_str() {

        "beReady" => {
            do_ready_notify(window);
        }
        "tabSelected" => {
            let tab_id = data.get("tabId").unwrap().as_str().unwrap().to_string();
            do_tab_selected(window, tab_id);
        }
        "tabClosed" => {
            let close_tab_id = data.get("closeTabId").unwrap().as_str().unwrap().to_string();
            let active_tab_id = data
                .get("activeTabId")
                .unwrap()
                .as_str()
                .unwrap()
                .to_string();
            do_tab_closed(window, close_tab_id, active_tab_id);
        }
        "alertShow" => {
            let title = data.get("title").unwrap().as_str().unwrap();
            let message = data.get("message").unwrap().as_str().unwrap();
            alert_show(window, title, message);
        }
        "addVoidTab" => {
            do_add_void_tab(window);
        }
        "openNewUrl" => {
            let tab_id = data.get("tabId").unwrap().as_str().unwrap();
            let url = data.get("url").unwrap().as_str().unwrap().to_string();
            do_open_new_url(window, tab_id.parse().unwrap(), url);
        }
        "changeWidthMode" => {
            let tab_id = data.get("tabId").unwrap().as_str().unwrap();
            let mode = data.get("mode").unwrap().as_str().unwrap();
            do_change_width_mode(window, tab_id, mode);
        }
        "openSetting" =>{
            do_open_setting(window);
        }
        "goHome" => {
            do_go_home(window);
        }
        _ => {}
    }
}

fn do_go_home(window: Window) {
    if let Some(web_view) = window.get_webview(ID_FIRST_PAGE_ID) {
        let _ = web_view.navigate(ID_FIRST_PAGE_URL.parse().unwrap());
    }
}

///
/// 打开设置页面，并且只能打开一个
fn do_open_setting(window: Window) {
    let app_store = window.app_handle().state::<AppGlobalState>();
    let tabs_size = app_store.get_open_tabs_count();
    if tabs_size >= MAX_TAB_COUNT as i32 {
        alert_show(window, "提示", "标签页数量达到最大值，无法打开设置页面");
        return;
    }
    let tabs = app_store.get_open_tabs();
    let tab = tabs.values().find(|tab| tab.url.ends_with(SETTING_PAGE_URL));
    if let Some(tab) = tab {
        let payload = json!({
                            "id": tab.id
                        });
        send_action_to_top_view(&window, "selectTab", payload);
        return;
    }
    log::info!("打开设置页面");
    let url = get_all_route_url(window.clone(), SETTING_PAGE_URL);
    if url.is_empty() {
        log::info!("获取设置页面 Url 失败，不能为空");
        return;
    }
    add_tab(&window, Tab::new("", "设置", &*url, true, ""), true, false);
}

fn get_all_route_url(window: Window, sub_url: &str) -> String {

    if let Some(webview) = window.get_webview(ID_TOP_VIEW) {
        // 2. 获取当前窗口的完整 Url 对象
        log::info!("获取当前窗口的完整 Url 对象{}", webview.url().unwrap().to_string());
        if let Ok(mut current_url) = webview.url() {
            // 3. 设置哈希路由路径（注意：set_fragment 不需要加 '#' 号）
            current_url.set_fragment(Some(sub_url));
            let url = current_url.to_string();
            return url;

        }
    }
    return "".to_string();

}

fn do_add_void_tab(window: Window) {
    let tabs_size = window.app_handle().state::<AppGlobalState>().get_open_tabs_count();
    if tabs_size >= MAX_TAB_COUNT as i32 {
        send_is_tab_add(&window, tabs_size);
        alert_show(window, "提示", "标签页数量达到最大值，无法添加新标签页");
        return;
    }
    let (tab_id, webview) = get_idle_webview(&window, "".to_string());
    if tab_id.is_empty() || webview.is_none() {
        alert_show(window, "提示", "无法添加新标签页,请重试。");
        return;
    }
    add_tab(&window, Tab::new(&*tab_id, "新页", BLANK_PAGE, true, ""), true, false);
    send_is_tab_add(&window, -1);
}

fn do_open_new_url(window: Window,tab_id:String, url: String) {
    if let Some(webview) = window.get_webview(&*tab_id) && !url.is_empty() {
        let state = window.app_handle().state::<AppGlobalState>();
        state.set_open_tabs_modify_val(webview.label(), crate::TabAttrName::Url, serde_json::Value::String(url.clone()));
        webview.navigate(url.parse().unwrap()).unwrap();
    }
}

fn do_ready_notify(window: Window) {

    let payload = json!({
        "id": ID_FIRST_PAGE_ID,
        "title": ID_FIRST_PAGE_TITLE,
        "url": ID_FIRST_PAGE_URL,
        "closable":false,
        "isShow":true
    });
    send_action_to_top_view(&window, "addTab", payload);
    let _win = window.clone();
    let state = _win.app_handle().state::<AppGlobalState>();
    state.set_open_tabs_clear();

    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;

        let store = get_config_store(window.app_handle());

        // 读取配置
        if let Some(val) = store.get("historyTabs") {
            let tabs: HashMap<String, Tab> = serde_json::from_value(val).unwrap_or(HashMap::new());
            log::info!("历史标签数量：{}", tabs.values().len());
            let mut list: Vec<String> = vec![];
            let mut i = 1;
            for val in tabs.values() {
                tokio::time::sleep(Duration::from_millis(500)).await;

                if i >= MAX_TAB_COUNT {
                    break;
                }
                if list.contains(&val.url){
                    continue;
                }
                add_tab(&window, val.clone(), false,true);
                list.push(val.clone().url);
                i += 1;
            }
            send_is_tab_add(&window, -1);
        }

    });
}

fn do_change_width_mode(window: Window,tab_id:&str,mode:&str) {
    if let Some(webview) = window.get_webview(tab_id) {
        // let mut js_content = std::fs::read_to_string("C:\\Users\\Administrator\\Git\\wenda-reader\\wenda_reader.js").unwrap();

        // 👈 关键：在脚本末尾强制追加 sourceURL 标记
        // js_content.push_str("\n//# sourceURL=debugger/injected_wenda_reader.js");
        if webview.url().unwrap().as_str().contains(WECHAT_READER_URL) {
            let js_code = format!("update_wenda_style('{}');", mode);
            webview.eval(&js_code).expect("js 方法update_wenda_style执行失败");
            log::info!("js 方法update_wenda_style 执行成功，mode:{}", mode);
            window.app_handle().state::<AppGlobalState>().set_open_tabs_modify_val(webview.label(), crate::TabAttrName::ReaderMode, serde_json::Value::String(mode.to_string()));
        }
        else{
            log::warn!("不是微信读书页，不执行js。");
        }
    }
    else{
        log::warn!("无法执行js，没找到标签页。");
    }
}

fn get_side_position(window: Window) -> SidePosition {
    let store = get_config_store(window.app_handle());
    let state = window.app_handle().state::<AppGlobalState>();
    if let Some(val) = store.get("sidePosition") {
        state.set_side_position(serde_json::from_value(val).unwrap());
    }
    state.get_side_position()
}

fn do_tab_selected(window: Window, tab_id: String) {
    switch_webview(window.clone(), "".parse().unwrap(), tab_id, false);
}

fn do_tab_closed(window: Window, close_tab_id: String, active_tab_id: String) {
    let _win = window.clone();
    let _close_tab_id = close_tab_id.clone();
    switch_webview(_win, _close_tab_id, active_tab_id, true);

    tauri::async_runtime::spawn(async move {
        let app_store = window.app_handle().state::<AppGlobalState>();
        app_store.set_open_tabs_remove(&close_tab_id);
        send_is_tab_add(&window, -1);
    });
}

// endregion TopView前端动作处理

// region Setting前端动作处理

fn send_action_to_setting_view(window: &Window, event_name: &str, data: serde_json::Value) {
    let payload = json!({
        "action": event_name,
        "data": data
    });

    let app_store = window.app_handle().state::<AppGlobalState>();
    let tabs = app_store.get_open_tabs();
    let tab = tabs.values().find(|tab| tab.url.ends_with(SETTING_PAGE_URL));
    if let Some(tab) = tab {
        window
            .app_handle()
            .emit_to(tab.id.clone(), "setting-view-action", payload)
            .unwrap();

    }


}

/*
 * 接收前端Setting动作
 */
#[tauri::command(async)]
fn receive_front_setting_action(window: Window, action: String, data: serde_json::Value) {
    match action.as_str() {

        "beSettingReady" => {
            do_setting_ready_notify(window);
        }
        "setSidePosition" => {
            let side_position = data.get("sidePosition").unwrap().as_str().unwrap();
            do_set_new_side_position(window, side_position);
        }

        _ => {}
    }
}

/**
 * 设置页面就绪通知
*/
fn do_setting_ready_notify(setting_window: Window) {

    let mut payload = json!({});

    let store = get_config_store(setting_window.app_handle());

    // 读取配置
    if let Some(val) = store.get("sidePosition") {
        payload["sidePosition"] = val.clone();
    }

    send_action_to_setting_view(&setting_window, "onSettingLoad", payload);
}

/**
* 设置窗体隐藏位置
 */
fn do_set_new_side_position(window: Window, side_position: &str) {
    if side_position.is_empty() {
        return;
    }
    log::info!("设置窗体隐藏位置:{}", side_position);
    if let Ok(_side_position) = SidePosition::from_str(side_position) {
        let app_store = window.app_handle().state::<AppGlobalState>();
        app_store.set_side_position(_side_position.clone());
        let side_window = window.clone().app_handle().get_window("side_window").unwrap();
        let (screen_width, screen_height) = get_screen_size(window.app_handle().clone());
        set_side_position(&side_window, _side_position, screen_width, screen_height);
    }
    else {
        log::warn!("设置窗体隐藏位置失败，无效的位置：{}", side_position);
    }

}
// endregion Setting前端动作处理


// endregion 前端动作处理

/// 切换 webview
fn switch_webview(window: Window, mut close_tab_id: String, mut show_tab_id:String, is_close_clear:bool) {
    let selected_tab_id = window.app_handle().state::<AppGlobalState>().get_selected_tab_id();
    if close_tab_id.is_empty(){
        close_tab_id = selected_tab_id.clone();
    }

    if let Some(close_webview) = window.get_webview(&*close_tab_id) {
        if !is_close_clear {
            capture_webview_scroll(&close_webview);
        }

        if is_close_clear {
            close_webview.navigate(BLANK_PAGE.parse().unwrap()).unwrap();
        }
        close_webview.hide().unwrap();
        if show_tab_id != selected_tab_id {

            if let Some(open_webview) = window.get_webview(&*show_tab_id) {
                if open_webview.url().unwrap().as_str().contains(WECHAT_READER_URL) {
                    if let Some(tab) = open_webview.app_handle().state::<AppGlobalState>().get_open_tabs_tab(&*show_tab_id) {
                        let mut mode = tab.reader_mode;
                        if mode.is_empty(){
                            mode = "default".to_string();
                        }
                        let js_code = format!("update_wenda_style('{}');", mode);
                        open_webview.eval(&js_code).expect("js 方法update_wenda_style执行失败");
                        log::info!("js 方法update_wenda_style 执行成功，mode:{}", mode);

                        let payload = json!({
                            "tabId": show_tab_id,
                            "mode": mode,
                        });
                        send_action_to_top_view(&window, "doWidthValue", payload);
                    }
                }
                open_webview.show().unwrap();

            }
            else {
                show_tab_id = ID_FIRST_PAGE_ID.to_string();
                if let Some(open_webview) = window.get_webview(ID_FIRST_PAGE_ID) {
                    open_webview.show().unwrap();
                }
            }
            let _id = show_tab_id.clone();
            let app_store = window.app_handle().state::<AppGlobalState>();
            app_store.set_selected_tab_id(&*show_tab_id.clone());
            modify_window_title(window, &*_id, "");
        }
    }
}

fn modify_window_title(window: Window, tab_id: &str, _title:&str) {

    let app_store = window.app_handle().state::<AppGlobalState>();
    let selected_tab_id = app_store.get_selected_tab_id();
    if selected_tab_id != tab_id {
        return;
    }
    let mut new_title: String = WINDOW_TITLE.to_string();
    if _title.is_empty() {
        if let Some(tab) = app_store.get_open_tabs_tab(&selected_tab_id){
            new_title = format!("{} - {}", WINDOW_TITLE, tab.title);
        }
    }
    else{
        new_title = format!("{} - {}", WINDOW_TITLE, _title);
    }
    window.set_title(&new_title).unwrap();
}


/// 判断鼠标是否在指定窗口的矩形范围内
#[cfg(target_os = "windows")]
fn is_window_under_mouse(hwnd_main: isize,hwnd_side: isize) -> bool {
    unsafe {
        let mut point = POINT { x: 0, y: 0 };
        // 只有成功拿到鼠标位置才继续
        if GetCursorPos(&mut point) != 0 {
            let hwnd_under_mouse = WindowFromPoint(point);
            // 第一步：比对原窗口指针地址
            let hwnd_root = GetAncestor(hwnd_under_mouse, GA_ROOT) as isize;
            if hwnd_root == hwnd_main || hwnd_root == hwnd_side {
                return true;
            }

            // 第二步：解决右键菜单等情况
            let mut class_name = [0u16; 256];
            let mut is_menu = false;
            let len = GetClassNameW(hwnd_under_mouse, class_name.as_mut_ptr(), class_name.len() as i32);
            if len > 0 && String::from_utf16_lossy(&class_name[..len as usize]) == "#32768" {
                is_menu = true;
            }
            else {
                let mut current_depth = 0;
                let max_safe_depth = 10;

                let mut hwnd_target = hwnd_under_mouse;
                let mut owner_hwnd = GetWindow(hwnd_target, GW_OWNER);
                while owner_hwnd != std::ptr::null_mut() && current_depth < max_safe_depth {
                    hwnd_target = owner_hwnd;
                    current_depth += 1;
                    owner_hwnd = GetWindow(hwnd_target, GW_OWNER); // 继续向上追溯，直到没有 Owner
                }

                let root_hwnd = hwnd_target as isize;
                if root_hwnd == hwnd_main || root_hwnd == hwnd_side {
                    return true;
                }
            }

            // 第三步：解决 Rust 提示框等情况
            let main_hwnd = hwnd_main;
            let mut our_process_id = 0u32;
            GetWindowThreadProcessId(main_hwnd as HWND, &mut our_process_id);

            let mut target_process_id = 0u32;
            GetWindowThreadProcessId(hwnd_under_mouse, &mut target_process_id);
            return target_process_id == our_process_id && !is_menu;
        }
    }
    false
}

// region 配置文件操作

#[allow(dead_code)]
/// 写入配置
fn write_config(app_handle: &tauri::AppHandle, data: (String, String)) {
    let app_handle = app_handle.clone();
    let _data = data.clone();
    tauri::async_runtime::spawn(async move {
        let store = get_config_store(&app_handle);
        let (key, value) = _data;
        // 写入配置
        store.set(key, json!(value));
        store.save().unwrap(); // 保存到硬盘
    });
}

/// 读取配置
#[allow(dead_code)]
fn read_config(app_handle: &tauri::AppHandle, key: String, def_value: String) -> String {
    let store = get_config_store(app_handle);

    // 读取配置
    if let Some(val) = store.get(key) {
        return val.as_str().unwrap_or("null").to_string();
    }
    def_value.to_string()
}

fn read_window_size(app_handle: &tauri::AppHandle) -> Option<LogicalSize<f64>> {
    let store = get_config_store(app_handle);

    // 读取配置
    if let Some(val) = store.get("windowSize") {
        if let Ok(size) = serde_json::from_value(val) {
            return Some(serde_json::from_value(size).unwrap());
        }
        return None;
    }
    None
}

fn get_config_store(app_handle: &tauri::AppHandle) -> Arc<Store<Wry>> {
    let file_path = app_handle.state::<AppGlobalState>().get_config_file_path();
    app_handle.store(file_path).unwrap()
}

// endregion 配置文件操作

fn get_tab_id(idx: u32) -> String {
    format!("tab-{}", idx)
}

/// 获取空闲的 webview
/// tab_id 可为空，为空则自动选择一个 webview，否则强制使用指定的 webview
/// 返回值为 (tab_id, webview)
fn get_idle_webview(window: &Window, tab_id: String) -> (String, Option<Webview>) {
    let state = window.app_handle().state::<AppGlobalState>();
    let tabs = state.get_open_tabs();
    if !tab_id.is_empty() && !tabs.contains_key(&tab_id) {
        let webview = window.get_webview(&*tab_id);
        if webview.is_none() {
            return (tab_id, None);
        }
        return (tab_id, webview);
    }
    for i in 1..=MAX_TAB_COUNT {
        let tab_id = get_tab_id(i);
        if !tabs.contains_key(&tab_id) {
            let webview = window.get_webview(&*tab_id);
            if webview.is_none() {
                return (tab_id, None);
            }
            return (tab_id, webview);
        }
    }
    alert_show(window.clone(), "提示", &format!("最大只能打开{}个标签", MAX_TAB_COUNT),
    );
    (String::from(""), None)
}

fn set_side_position(side_window:&Window,side_position: SidePosition,screen_width:f64, screen_height:f64){
    let mut x = screen_width / 2.0 - SIDE_WINDOW_SIZE.width / 2.0;
    let mut y = HIDE_REMAIN - SIDE_WINDOW_SIZE.height;

    match side_position {
        SidePosition::TopLeft => {
            x = screen_width / 4.0 - SIDE_WINDOW_SIZE.width / 2.0;
        },

        SidePosition::TopRight => {
            x = (screen_width / 4.0) * 3.0 + SIDE_WINDOW_SIZE.width / 2.0;
        },
        SidePosition::Left => {
            x = HIDE_REMAIN - SIDE_WINDOW_SIZE.width;
            y = screen_height / 2.0 - SIDE_WINDOW_SIZE.height / 2.0;
        },
        SidePosition::Right => {
            x = screen_width - HIDE_REMAIN;
            y = screen_height / 2.0 - SIDE_WINDOW_SIZE.height / 2.0;
        },
        SidePosition::TopCenter => {},
    }

    side_window.set_position(
        LogicalPosition::new(x, y)
    ).expect("初始化设置边窗位置失败");
}

/// 获取当前时间字符串
// fn get_current_time_str() -> String {
//     Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
// }

/// 返回当前时间的时间戳（毫秒级）
fn get_timestamp() -> i64 {
    Utc::now().timestamp_millis()
}

/// 立即将内存中的变更同步写入配置文件
fn flush_config_to_disk(app_handle: &tauri::AppHandle) {
    let app_store = app_handle.state::<AppGlobalState>();
    if !app_store.get_is_update() {
        return;
    }

    let file_path = app_store.get_config_file_path();
    let tabs = app_store.get_open_tabs();
    let new_size = app_store.get_window_size();
    let side_position = app_store.get_side_position();

    if let Ok(store) = app_handle.store(file_path) {
        store.set("historyTabs", json!(tabs));
        store.set("windowSize", json!(new_size));
        store.set("sidePosition", json!(side_position));
        if let Err(e) = store.save() {
            log::error!("保存配置文件失败: {:?}", e);
        } else {
            log::info!("配置文件已成功保存到磁盘");
            app_store.set_is_update(false);
        }
    }
}

/// 判断指定 URL 是否需要记录滚动条位置（排除微信读书、空白页及设置页）
fn should_record_scroll(url: &str) -> bool {
    let lower = url.trim().to_lowercase();
    !lower.is_empty()
        && lower != BLANK_PAGE
        && !lower.contains(WECHAT_READER_DOMAIN)
        && !lower.ends_with(SETTING_PAGE_URL)
}

/// 命令指定 Webview 立即上报当前滚动位置并更新到对应的 Tab 属性中
fn capture_webview_scroll(webview: &Webview) {
    if let Ok(url) = webview.url() {
        if !should_record_scroll(url.as_str()) {
            return;
        }
        let js_code = format!(
            r#"(function() {{
                if (typeof window.__wendaGetScrollState === 'function') {{
                    const state = window.__wendaGetScrollState();
                    const origTitle = document.title;
                    document.title = "{}" + JSON.stringify(state);
                    document.title = origTitle;
                }}
            }})();"#,
            SCROLL_TITLE_PREFIX
        );
        let _ = webview.eval(&js_code);
    }
}


