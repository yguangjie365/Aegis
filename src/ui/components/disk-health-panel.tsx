import { useTranslation } from 'react-i18next';
import type { DiskHealthRowDto } from '@/types/overview';

interface DiskHealthPanelProps {
  rows: DiskHealthRowDto[];
}

/**
 * F-06 磁盘健康面板：逐行列出每块存储设备（最多 5 行）
 * - 设备名、该盘最高温度、SMART 类属性数量
 * - 无端点时显示"未检测到存储设备（需管理员权限）"说明而非空面板
 */
export function DiskHealthPanel({ rows }: DiskHealthPanelProps) {
  const { t } = useTranslation('overview');

  // 空态处理：无数据或所有行都是空行
  const hasData = rows.some((row) => row.deviceName || row.reason);
  if (!hasData) {
    return (
      <div className="rounded-lg border border-border bg-card p-4">
        <h3 className="mb-2 text-sm font-semibold text-foreground">
          {t('card.title.storage')}
        </h3>
        <p className="text-sm text-muted-foreground">
          {t('diskHealth.noData')}
        </p>
      </div>
    );
  }

  return (
    <div className="rounded-lg border border-border bg-card p-4">
      <h3 className="mb-2 text-sm font-semibold text-foreground">
        {t('card.title.storage')}
      </h3>
      <div className="space-y-2">
        {rows.map((row, index) => {
          // 空行占位
          if (!row.deviceName && !row.reason) {
            return <div key={index} className="h-8" />;
          }

          // 错误态
          if (row.reason && !row.deviceName) {
            return (
              <div key={index} className="text-sm text-muted-foreground">
                {row.reason}
              </div>
            );
          }

          // 正常行
          const tempText =
            row.temperatureC != null ? `${row.temperatureC}°C` : '—';
          const healthPercentText =
            row.healthPercent != null ? `${row.healthPercent}%` : '—';

          // 健康状态颜色映射
          const healthColor =
            row.healthStatus === 'healthy'
              ? 'text-green-600'
              : row.healthStatus === 'warning'
              ? 'text-yellow-600'
              : row.healthStatus === 'unhealthy'
              ? 'text-red-600'
              : 'text-muted-foreground';

          return (
            <div
              key={index}
              className="flex items-center justify-between rounded border border-border bg-background px-3 py-2 text-sm"
            >
              <div className="flex-1 truncate font-medium">
                {row.deviceName}
              </div>
              <div className="ml-4 flex items-center gap-4 text-xs">
                <span className="text-muted-foreground">
                  {t('detail.display.temperature')}: {tempText}
                </span>
                <span className={healthColor}>
                  {t('diskHealth.health')}: {healthPercentText}
                </span>
                {row.smartAttributeCount > 0 && (
                  <span className="text-muted-foreground">
                    SMART: {row.smartAttributeCount}
                  </span>
                )}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
