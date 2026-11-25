// ================= 1. 微信读书宽度调节 =================
function register_css() {
  if (document.getElementById("wenda-reader-style")) {
    return;
  }
  const oStyle = document.createElement("style");
  oStyle.id = "wenda-reader-style";
  oStyle.textContent = `
        :root {
            --wendaread-width: 70%;
        }

        html.wenda-reader-mode .readerContent {
            width: calc(100vw - 15px) !important;
            max-width: calc(100vw - 15px) !important;
            min-height: 100vh !important;
            margin: 0 !important;
            overflow-x: hidden !important;
        }

        html.wenda-reader-mode .app_content {
            width: var(--wendaread-width) !important;
            max-width: var(--wendaread-width) !important;
            margin-left: auto !important;
            margin-right: auto !important;
            min-height: 100vh !important;
            overflow-x: hidden !important;
        }

        html.wenda-reader-mode .readerTopBar {
            max-width: var(--wendaread-width) !important;
        }

        html.wenda-reader-mode .readerControls {
            left: auto !important;
            right: 0 !important;
        }
    `;
  document.head.appendChild(oStyle);
}

let preMode = "";
function update_wenda_style(mode) {
  register_css();
  if (!mode) {
    mode = "default";
  }
  if (mode === preMode) return;

  if (mode === "default") {
    if (document.documentElement.classList.contains("wenda-reader-mode")) {
      document.documentElement.classList.remove("wenda-reader-mode");
    }
  } else {
    document.documentElement.style.setProperty("--wendaread-width", mode);
    document.documentElement.classList.add("wenda-reader-mode");
  }
  preMode = mode;
  window.dispatchEvent(new Event('resize'));
}

// ================= 2. 供 Rust 后端调用的滚动位置记录与恢复 =================
(function () {
  // 记录最后一次用户触发滚动的容器（兼容小说网站的局部 div 滚动）
  window.__wendaLastScrollEl = null;

  window.addEventListener("scroll", (e) => {
    const target = e.target;
    if (target && target !== document && target !== document.documentElement && target !== document.body) {
      window.__wendaLastScrollEl = target;
    } else {
      window.__wendaLastScrollEl = null;
    }
  }, { capture: true, passive: true });

  // 提取元素简易 CSS 选择器
  window.__wendaGetSelector = function (el) {
    if (!el || el === document.body || el === document.documentElement) return "";
    if (el.id) return "#" + el.id;
    if (el.className && typeof el.className === "string") {
      const classes = el.className.trim().split(/\s+/).filter(Boolean);
      if (classes.length > 0) return "." + classes.join(".");
    }
    return el.tagName ? el.tagName.toLowerCase() : "";
  };

  // 获取当前页面滚动状态（返回 [scrollTop, selector]）
  window.__wendaGetScrollState = function () {
    const el = window.__wendaLastScrollEl;
    if (el && document.contains(el) && el.scrollTop > 0) {
      return {
        top: el.scrollTop,
        selector: window.__wendaGetSelector(el)
      };
    }
    const winTop = window.scrollY || document.documentElement.scrollTop || document.body.scrollTop || 0;
    return {
      top: winTop,
      selector: ""
    };
  };

  // 恢复滚动位置（支持异步内容撑开重试）
  window.wenda_restore_scroll = function (targetTop, selector) {
    if (typeof targetTop !== "number" || targetTop <= 0) return;

    let attempts = 0;
    const maxAttempts = 20; // 最多轮询 20 次（约 4 秒）

    const timer = setInterval(() => {
      attempts++;
      let targetEl = null;
      let maxScroll = 0;

      if (selector) {
        targetEl = document.querySelector(selector);
        if (targetEl) {
          maxScroll = targetEl.scrollHeight - targetEl.clientHeight;
        }
      } else {
        targetEl = window;
        maxScroll = Math.max(
          document.body.scrollHeight,
          document.documentElement.scrollHeight
        ) - window.innerHeight;
      }

      if (targetEl && maxScroll >= targetTop - 10) {
        if (selector) {
          targetEl.scrollTop = targetTop;
          window.__wendaLastScrollEl = targetEl;
        } else {
          window.scrollTo({ top: targetTop, behavior: "instant" });
        }
        clearInterval(timer);
      } else if (attempts >= maxAttempts) {
        if (targetEl) {
          if (selector) {
            targetEl.scrollTop = targetTop;
            window.__wendaLastScrollEl = targetEl;
          } else {
            window.scrollTo({ top: targetTop, behavior: "instant" });
          }
        }
        clearInterval(timer);
      }
    }, 200);
  };
})();
