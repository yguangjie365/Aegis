// backup 域前端门面（M-06）：12 个命令。
// 🔴 业务代码不得裸调 tauriInvoke —— 一律经本文件（01 专题「前端不触引擎」白名单侧）。
//
// 命令面契约（crates/services/src/backup/commands.rs）：
// * 入参统一 `CmdEnvelope`（由 invoke.ts 自动包）；需要额外键的命令这里传对象，
//   invoke.ts 会把**顶层键平铺**到 envelope 之外（Rust 侧按 `args` 结构反序列化）
// * 失败 **reject**，载荷是 `{ code, message, correlationId }` 的 `CommandError`
// * 🔴 **例外**：`backup_list_rules` / `backup_list_snapshots` 在记录损坏时
//   **不 reject**，而是把警告放进 `warnings` 并按可读部分继续（R-18）
//   —— 界面显示警告条，不是失败态。
//
// 🟠 **本批无进度事件**：`commands.rs` 的 `progress` 回调是空实现
// （事件名须走 contracts注册表，W0-07 未落地）。故 `backup_start` 期间
// 页面的进度区按「运行中·阶段未知」处理，**不得**伪造百分比。

import { BACKEND_OFFLINE_CODE, invoke, isBackendOffline } from "./invoke";
import type { LogModule } from "@/lib/logging";
import type {
  BackupError,
  BackupOutcome,
  ExportView,
  ImportView,
  RestoreOutcome,
  RestorePlanView,
  RuleListView,
  RuleView,
  SettingsView,
  SnapshotView,
  VerifyReportView,
} from "@/types/backup";

const BACKUP: LogModule = "Backup";

/** 规则列表 + 页头汇总（F-FB-01）。 */
export async function fetchRules(): Promise<RuleListView> {
  return invoke<RuleListView>("backup_list_rules", BACKUP, "backupListRules", {});
}

/**
 * 保存规则（F-FB-02/04/05）。
 *
 * 🔴 `confirmedDraft=false` 且**全部来源都不存在**时，后端 reject
 * `DRAFT_CONFIRM_REQUIRED`（F-FB-04）—— 界面必须先弹确认再带 `true` 重试，
 * 不能把这条 reject 当成「保存失败」吞掉。
 */
export async function saveRule(rule: RuleView, confirmedDraft = false): Promise<RuleView> {
  return invoke<RuleView>("backup_save_rule", BACKUP, "backupSaveRule", {
    rule,
    confirmedDraft,
  });
}

/**
 * 删除规则（F-FB-06）。
 *
 * 🔴 **快照不连带删除** —— 界面必须在确认框里说清，否则用户以为磁盘腾干净了。
 * 返回实际删除条数（`1`）。
 */
export async function deleteRule(rule: RuleView): Promise<number> {
  return invoke<number>("backup_delete_rule", BACKUP, "backupDeleteRule", { rule });
}

/**
 * 立即备份（F-FB-16）。
 *
 * 🔴 这是**同步长任务**：后端在 `spawn_blocking` 里跑，调用期间 Promise 不 resolve。
 * 取消要用 `cancelBackup()`（另一个命令），不能中断这个 await。
 *
 * 🔴 失败只有**闸门类**（规则不存在 / 落点重叠 / 空间不足）才 reject；
 * 单文件失败、部分成功、用户取消**都 resolve**，结论在返回值里（R-13/R-14）。
 */
export async function startBackup(ruleId: string, label = ""): Promise<BackupOutcome> {
  return invoke<BackupOutcome>("backup_start", BACKUP, "backupStart", { ruleId, label });
}

/**
 * 请求取消备份（F-FB-19）。
 *
 * 🔴 返回 reject（`NOT_RUNNING`）表示**当前没有在跑** —— 那通常意味着
 * 「你点慢了，它刚跑完」。界面应据此刷新列表，而不是弹「取消失败」。
 * 真正的取消结论在 `startBackup` 的返回值里（`cancelled: true`）。
 */
export async function cancelBackup(): Promise<void> {
  return invoke<void>("backup_cancel", BACKUP, "backupCancel", {});
}

/** 快照列表（F-FB-26）。 */
export async function listSnapshots(ruleId: string): Promise<SnapshotView[]> {
  return invoke<SnapshotView[]>("backup_list_snapshots", BACKUP, "backupListSnapshots", {
    ruleId,
  });
}

/**
 * 手动校验快照（F-FB-28：读回全量比对）。
 *
 * 🔴 `compared < total` 时结论必为 `notFullyVerified` —— 界面要显示
 * 「抽查N/M」而不是「通过」（F-FB-29）。
 */
export async function verifySnapshot(
  ruleId: string,
  snapshotDir: string,
): Promise<VerifyReportView> {
  return invoke<VerifyReportView>("backup_verify_snapshot", BACKUP, "backupVerifySnapshot", {
    ruleId,
    snapshotDir,
  });
}

/** 删除快照（F-FB-30：二次确认由界面做）。 */
export async function deleteSnapshot(ruleId: string, snapshotDir: string): Promise<void> {
  return invoke<void>("backup_delete_snapshot", BACKUP, "backupDeleteSnapshot", {
    ruleId,
    snapshotDir,
  });
}

/**
 * 用系统文件管理器打开快照目录（F-FB-27）。
 *
 * 🔴 越界 / 不存在 / 无权限三种失败由**后端**分级上报（R-1 末句），
 * 前端不得自己拼路径 —— 那会把守卫绕过去。
 */
export async function openSnapshotDir(ruleId: string, snapshotDir: string): Promise<void> {
  return invoke<void>("backup_open_snapshot_dir", BACKUP, "backupOpenSnapshotDir", {
    ruleId,
    snapshotDir,
  });
}

/**
 * 单键切换规则的启用 / 停用（F-FB-07）。
 *
 * 🔴 走**专用命令**而非「保存规则」：保存要过完整表单校验（含草稿确认），
 * 于是「停用一条来源暂时不在的规则」会被拦下 —— 而用户只是想停用它。
 */
export async function setRuleEnabled(ruleId: string, enabled: boolean): Promise<void> {
  return invoke<void>("backup_set_rule_enabled", BACKUP, "backupSetRuleEnabled", {
    ruleId,
    enabled,
  });
}

/**
 * 导出规则到指定文件（F-FB-14）。
 *
 * 🔴 扩展名不合规由**后端**拒绝并说明原因，前端不静默改后缀。
 */
export async function exportRules(targetPath: string): Promise<ExportView> {
  return invoke<ExportView>("backup_export_rules", BACKUP, "backupExportRules", { targetPath });
}

/**
 * 导入规则（**合并式**，F-FB-15）。
 *
 * 🔴 不是整表替换：换机迁移时那会丢掉本机已建的规则。
 *   返回值含逐条明细（改名、空名），界面须展示 —— 静默导入等于骗人。
 */
export async function importRules(sourcePath: string): Promise<ImportView> {
  return invoke<ImportView>("backup_import_rules", BACKUP, "backupImportRules", { sourcePath });
}

/**
 * 重排规则顺序（F-FB-08）。
 *
 * `orderedIds` 必须是**全部**规则 id 的一个排列；缺条目/多条目后端直接拒绝
 * （否则「拖到末尾」会表现成「悄悄丢了一条」）。
 */
export async function reorderRules(orderedIds: string[]): Promise<void> {
  return invoke<void>("backup_reorder_rules", BACKUP, "backupReorderRules", { orderedIds });
}

/**
 * 恢复预演（F-FB-33/34/35，**只读不写盘**）。
 *
 * 🔴 与 `executeRestore` **共用后端同一 `plan`**（`restore::plan`）——
 * 界面不得自己算一份「大概会冲突多少」。
 * `policy` 传空串 = 取全局默认。
 */
export async function planRestore(
  ruleId: string,
  snapshotDir: string,
  customTarget = "",
  policy = "",
): Promise<RestorePlanView> {
  return invoke<RestorePlanView>("backup_plan_restore", BACKUP, "backupPlanRestore", {
    ruleId,
    snapshotDir,
    customTarget,
    policy,
  });
}

/**
 * 执行恢复（F-FB-36）。
 *
 * 🔴 单文件失败**不 reject**（计入 `failed` / `verifyMismatched` / `skipped`）——
 * 「1200 项里 3 项失败」是一次已完成的恢复，界面要照实报数而不是报异常。
 */
export async function executeRestore(
  ruleId: string,
  snapshotDir: string,
  customTarget = "",
  policy = "",
): Promise<RestoreOutcome> {
  return invoke<RestoreOutcome>("backup_execute_restore", BACKUP, "backupExecuteRestore", {
    ruleId,
    snapshotDir,
    customTarget,
    policy,
  });
}

/** 读设置（F-FB-24/29/34）。 */
export async function fetchSettings(): Promise<SettingsView> {
  return invoke<SettingsView>("backup_get_settings", BACKUP, "backupGetSettings", {});
}

/** 保存设置（后端原子写 + 数值钳制，R-19）。 */
export async function saveSettings(settings: SettingsView): Promise<void> {
  return invoke<void>("backup_save_settings", BACKUP, "backupSaveSettings", { settings });
}

/**
 * 把后端 reject 出来的东西收敛成 `BackupError`（`invoke` 可能抛出任意形状）。
 *
 * 🔴 `BackendUnavailableError` 是**永久性错误**（不在 Tauri 运行时内），界面须显示
 * 「后端未运行」而不是错误码 —— 故单独给 `BACKEND_OFFLINE` 码，而不是混进 UNKNOWN
 * 让身份在归一化途中丢失（2026-10-06 UI 审查 D7）。
 */
export function asBackupError(error: unknown): BackupError {
  if (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    "message" in error &&
    typeof (error as BackupError).code === "string" &&
    typeof (error as BackupError).message === "string"
  ) {
    return error as BackupError;
  }
  if (isBackendOffline(error)) {
    return {
      code: BACKEND_OFFLINE_CODE,
      message: error instanceof Error ? error.message : String(error),
    };
  }
  return {
    code: "UNKNOWN",
    message: error instanceof Error ? error.message : String(error),
  };
}
