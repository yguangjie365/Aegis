# build-monitor.ps1 — AegisMonitor 传感器 sidecar 可复现构建（M-01 片1-a，2026-10-03）
#
# 为什么单独成文：`Aegis/monitor/` 是全工程唯一的 C# 编译单元，构建链与 Rust/前端两条腿无关；
# 把命令固化在这里，spike 取证（spikes/K-Q12-1-monitor-build.md）与后续 CI 都调同一入口，
# 避免"我机器上能编"的黑盒状态。
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
