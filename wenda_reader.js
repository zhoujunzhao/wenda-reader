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

function update_wenda_style(mode) {
  register_css();

  if(!mode || mode === "default"){
    document.documentElement.classList.remove("wenda-reader-mode");
  }
  else {
    document.documentElement.style.setProperty("--wendaread-width", mode);
    document.documentElement.classList.add("wenda-reader-mode");
  }

  window.dispatchEvent(new Event('resize'));

}
