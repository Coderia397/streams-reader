// @ts-check
/** The context menu: "Read this page", "Read selection", "Read linked page". */

export const MenuIds = Object.freeze({
  page: "read-page",
  selection: "read-selection",
  link: "read-link",
});

/** Recreates the menu items (Chrome keeps them across worker restarts, not across updates). */
export function installMenus() {
  chrome.contextMenus.removeAll(() => {
    chrome.contextMenus.create({
      id: MenuIds.page,
      title: chrome.i18n.getMessage("menu_read_page"),
      contexts: ["page"],
    });
    chrome.contextMenus.create({
      id: MenuIds.selection,
      title: chrome.i18n.getMessage("menu_read_selection"),
      contexts: ["selection"],
    });
    chrome.contextMenus.create({
      id: MenuIds.link,
      title: chrome.i18n.getMessage("menu_read_link"),
      contexts: ["link"],
      targetUrlPatterns: ["http://*/*", "https://*/*"],
    });
  });
}

/**
 * @param {chrome.contextMenus.OnClickData} info
 * @returns {import("./collect.js").ReadRequest}
 */
export function requestFor(info) {
  switch (info.menuItemId) {
    case MenuIds.selection:
      return { kind: "selection", frameId: info.frameId ?? 0, selectionText: info.selectionText };
    case MenuIds.link:
      return { kind: "link", linkUrl: info.linkUrl };
    default:
      return { kind: "page" };
  }
}
