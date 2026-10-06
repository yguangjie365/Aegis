// 备份运行期进度区。
//
// 🔴 **为什么没有百分比**：`commands.rs` 的 `progress` 回调目前是空实现
//   （事件名须走 contracts 注册表，W0-07 生成链未落地）。
//   在那之前显示「62%」是**编造数据** —— 假进度条比没有进度条更糟，
//   用户会以为快完了而不敢切走。接线后（`ProgressView` 已定义好）
//   把下面的文案换成 `stage` + `percent` 即可，结构不用改。

import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import { AppButton } from "@/ui/components/app-button";

const Bar = styled.div`
  display: flex;
  align-items: center;
  gap: var(--aegis-space-md);
  border: 1px solid var(--aegis-hairline);
  border-radius: var(--aegis-radius-sm);
  background: var(--aegis-canvas);
  padding: var(--aegis-space-md) var(--aegis-space-lg);
`;

const Text = styled.div`
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  gap: 2px;
`;

const Title = styled.div`
  font-weight: 700;
  font-size: var(--aegis-type-body-size);
  color: var(--aegis-ink);
`;

const Note = styled.div`
  font-size: var(--aegis-type-caption-size);
  color: var(--aegis-mute);
`;

/** 不定进度条：纯装饰的扫光，表示「在动但不知道进度」。 */
const Track = styled.div`
  height: 4px;
  border-radius: var(--aegis-radius-full);
  background: var(--aegis-surface-inset);
  overflow: hidden;
  margin-top: var(--aegis-space-xs);
`;

const Sweep = styled.div`
  height: 100%;
  width: 36%;
  border-radius: var(--aegis-radius-full);
  background: color-mix(in srgb, var(--aegis-accent) 70%, transparent);
  animation: aegis-backup-indeterminate 1.4s var(--aegis-motion-easing-linear) infinite;

  @keyframes aegis-backup-indeterminate {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(320%);
    }
  }

  /* 规范 §8.3：reduced-motion 下停帧。停在一个**可见**位置
     （0% 的 translateX(-100%) 会让条完全消失，读成「没在跑」）。 */
  @media (prefers-reduced-motion: reduce) {
    animation: none;
    transform: none;
    width: 100%;
    opacity: 0.35;
  }
`;

export function BackupProgress({
  onCancel,
  cancelPending,
}: {
  onCancel: () => void;
  cancelPending: boolean;
}) {
  const { t } = useTranslation("backup");
  return (
    <Bar role="status" aria-live="polite">
      <Text>
        <Title>{t("progress.title")}</Title>
        <Note>{t("progress.stageUnknown")}</Note>
        <Track>
          <Sweep />
        </Track>
        <Note>{t("progress.cancelHint")}</Note>
      </Text>
      <AppButton variant="secondary" size="sm" disabled={cancelPending} onClick={onCancel}>
        {t("progress.cancel")}
      </AppButton>
    </Bar>
  );
}
