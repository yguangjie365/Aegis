# build-monitor.ps1 — AegisMonitor 传感器 sidecar 可复现构建（M-01 片1-a，2026-10-03）
#
# 为什么单独成文：`Aegis/monitor/` 是全工程唯一的 C# 编译单元，构建链与 Rust/前端两条腿无关；
# 把命令固化在这里，spike 取证（spikes/K-Q12-1-monitor-build.md）与后续 CI 都调同一入口，
# 避免"我机器上能编"的黑盒状态。
#
# 🔴 本文件必须存为 **UTF-8 with BOM**（2026-10-07 补注）。
#   Windows PowerShell 5.1 读**无 BOM** 的 UTF-8 脚本时按系统 ANSI 代码页（简中环境为 GBK）
#   解码，本文件的中文注释里某些字节序列会被误判成引号 / 方括号 / `$`，
#   实测报 `所在位置 …:23 字符:14`、`意外的标记"build-monitor]"`、`字符串缺少终止符`
#   —— 表现为**解析失败**，与 dotnet 无关（首次遇到极易误判为「脚本写错了」或「环境坏了」）。
#   修法即加 BOM（本行开头的 U+FEFF 即为该 BOM，编辑器须保留它）。
#   判据：`[byte[]]` 前三字节应为`EF BB BF`。
#
# 用法（在仓库任意位置执行均可）：
#   powershell -ExecutionPolicy Bypass -File tools/build-monitor.ps1
#   powershell -ExecutionPolicy Bypass -File tools/build-monitor.ps1 -Configuration Debug
param(
    [ValidateSet("Debug", "Release")]
    [string]$Configuration = "Release"
)

$ErrorActionPreference = "Stop"

# $PSScriptRoot = Aegis/tools → 上级即仓库根
$RepoRoot = Split-Path -Parent $PSScriptRoot
$Project = Join-Path $RepoRoot "monitor/AegisMonitor.csproj"
$OutDir = Join-Path $RepoRoot "monitor/bin/$Configuration/net48"

if (-not (Get-Command dotnet -ErrorAction SilentlyContinue)) {
    throw "找不到 dotnet CLI。先安装 .NET SDK 8（winget install Microsoft.DotNet.SDK.8）"
}

Write-Host "[build-monitor] dotnet SDK:" -ForegroundColor Cyan
dotnet --list-sdks

Write-Host "[build-monitor] restore $Project" -ForegroundColor Cyan
dotnet restore $Project
if ($LASTEXITCODE -ne 0) { throw "dotnet restore 失败（退出码 $LASTEXITCODE）" }

Write-Host "[build-monitor] build -c $Configuration -o $OutDir" -ForegroundColor Cyan
dotnet build $Project -c $Configuration -o $OutDir --no-restore
if ($LASTEXITCODE -ne 0) { throw "dotnet build 失败（退出码 $LASTEXITCODE）" }

$Exe = Join-Path $OutDir "AegisMonitor.exe"
if (-not (Test-Path $Exe)) { throw "构建声称成功但产物缺失: $Exe" }

$hash = (Get-FileHash $Exe -Algorithm SHA256).Hash
$size = (Get-Item $Exe).Length
Write-Host "[build-monitor] OK $Exe" -ForegroundColor Green
Write-Host "[build-monitor] size=$size sha256=$hash" -ForegroundColor Green
