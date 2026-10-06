import { describe, expect, it } from "vitest";
import { BackendUnavailableError, isBackendOffline } from "@/lib/tauri/invoke";
import { asBackupError } from "@/lib/tauri/backup";
import { asTransferError } from "@/lib/tauri/transfer";
import { failureDetail, failureReason } from "./failure-detail";

/* 为什么给这两个纯函数写测试（2026-10-06 UI 审查 D7）：
   「主线只说人话、命令名收进折叠」是一条**文案契约**，但它最容易在两种情况下悄悄失效：
   ① 有人把整句 message 直接塞回主线（一行代码就破约，肉眼看不出）；
   ② 归一化把错误身份吃掉 —— as*Error 会把 BackendUnavailableError 折成普通对象，
      若没有独立错误码，界面再也分不清「后端没在跑」与「后端回了未知码」。
   渲染探针能拍到 ① 的结果、拍不到 ② 的机制，故钉在单元层。 */

const OFFLINE = new BackendUnavailableError("backup_list_rules");

describe("failureReason / failureDetail —— 失败态文案的两段拆分", () => {
  it("后端未运行时主线只给人话，不含命令标识", () => {
    const reason = failureReason(OFFLINE);
    expect(reason).toBe("后端未运行");
    expect(reason).not.toMatch(/backup_list_rules/);
    expect(reason).not.toContain("Tauri");
  });

  it("命令名进折叠详情，且原样保留（报障时它是唯一定位手段）", () => {
    const detail = failureDetail(OFFLINE);
    expect(detail).toContain("backup_list_rules");
    expect(detail).toContain("Tauri");
  });

  it("🔴 归一化之后仍然认得「后端未运行」这一类（身份不丢）", () => {
    for (const normalized of [asBackupError(OFFLINE), asTransferError(OFFLINE)]) {
      expect(isBackendOffline(normalized)).toBe(true);
      expect(failureReason(normalized)).toBe("后端未运行");
    }
  });

  it("后端真回了错误载荷时：原样给原因，且**不**凭空造一个折叠", () => {
    const fromBackend = { code: "SOURCE_NOT_FOUND", message: "源目录不存在或已移动" };
    expect(failureReason(fromBackend)).toBe("源目录不存在或已移动");
    expect(failureDetail(fromBackend)).toBeNull();
  });

  it("非规范形状退化成 UNKNOWN，不冒充「后端未运行」", () => {
    const weird = { note: "没有 code 也没有 message" };
    expect(asBackupError(weird).code).toBe("UNKNOWN");
    expect(isBackendOffline(asBackupError(weird))).toBe(false);
  });

  it("规范形状原样透传（归一化不得改写后端给的码与文案）", () => {
    const ok = { code: "LOCK_HELD", message: "目标文件被占用" };
    expect(asTransferError(ok)).toEqual(ok);
  });
});
