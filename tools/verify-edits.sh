#!/usr/bin/env bash
# verify-edits.sh —— 批量改文件后的**强制回读验证**（2026-10-04 因连续三次"以为改了其实没改"而立）
#
# 背景：曾三次发生「脚本打印『已修复』但文件里还是旧代码」，根因都是
#       Python heredoc 在同一次调用里处理多个文件时中间变量被复用/覆盖，
#       前一次写入被静默丢弃。详见 .workbuddy/memory/MEMORY.md 的「回读验证」条目。
#
# 用法：
#   bash Tools/verify-edits.sh <文件> "<grep 真值>"      # 该文件应含此串（且不含 -v 模式给的串）
#   bash Tools/verify-edits.sh -v <文件> "<应不存在的串>"
#
# 退出码：0 = 命中（符合预期）；1 = 未命中（说明改动没落盘，**禁止**汇报"已修复"）
set -uo pipefail

MODE="has"
if [ "${1:-}" = "-v" ]; then MODE="absent"; shift; fi

FILE="${1:-}"
WANT="${2:-}"

if [ -z "$FILE" ] || [ -z "$WANT" ]; then
  echo "用法: bash Tools/verify-edits.sh [-v] <文件> \"<期望字符串>\"" >&2
  exit 2
fi

if [ ! -f "$FILE" ]; then
  echo "❌ 文件不存在: $FILE" >&2
  exit 1
fi

CNT=$(grep -c -F -- "$WANT" "$FILE" 2>/dev/null || true)
CNT=${CNT:-0}

if [ "$MODE" = "absent" ]; then
  if [ "$CNT" -gt 0 ]; then
    echo "❌ 改动未落盘: $FILE 仍有 $CNT 处 '$WANT'"
    echo "   ⛔ 不要汇报『已修复』—— 先用 Edit 工具重做，勿用 Python 批量改多文件。"
    exit 1
  fi
  echo "✅ 通过: $FILE 已无 '$WANT'（回读确认改动落盘）"
  exit 0
fi

if [ "$CNT" -eq 0 ]; then
  echo "❌ 改动未落盘: $FILE 找不到 '$WANT'"
  echo "   ⛔ 不要汇报『已修复』—— 先用 Edit 工具重做，勿用 Python 批量改多文件。"
  exit 1
fi

echo "✅ 通过: $FILE 含 $CNT 处 '$WANT'（回读确认改动落盘）"
