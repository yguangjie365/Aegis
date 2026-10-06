import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import shell from "@/locales/zh-CN/shell.json";
import common from "@/locales/zh-CN/common.json";
import music from "@/locales/zh-CN/music.json";
import overview from "@/locales/zh-CN/overview.json";
import fileTransfer from "@/locales/zh-CN/file-transfer.json";
import backup from "@/locales/zh-CN/backup.json";
import settings from "@/locales/zh-CN/settings.json";

// W0-14 首步：zh-CN 唯一语言、命名空间按域拆（common.* 通用态文案）。
// 裸中文扫描规则随 W0-04/W0-18 的 ESLint 基线落地；feature 命名空间随各模块开工追加。
void i18n.use(initReactI18next).init({
  lng: "zh-CN",
  fallbackLng: "zh-CN",
  resources: {
    // 🔴 键名用**驼峰**（`fileTransfer`）而文件名用 **kebab**（`file-transfer.json`）：
    //   i18next 的 ns 名即此处的键，两处不一致会在运行时退化成「显示原始 key」。
    "zh-CN": { shell, common, music, overview, fileTransfer, backup, settings },
  },
  interpolation: { escapeValue: false },
});

export default i18n;
