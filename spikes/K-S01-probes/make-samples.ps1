# K-S01 测试样本生成（长路径样本手工造很麻烦，故留脚本保证可复现）
# 用法：pwsh -File make-samples.ps1
$ErrorActionPreference = 'Stop'
$base = 'D:\MyProject\MyAPP\.ks01-test'
New-Item -ItemType Directory -Force -Path $base | Out-Null

# 样本 1：非 ASCII + 空格 + emoji（代理对）
$f1 = Join-Path $base '中文文件名 带空格 与emoji 🚀.txt'
Set-Content -LiteralPath $f1 -Value 'K-S01 非 ASCII 样本' -Encoding UTF8

# 样本 2：长路径 >260 字符
$deep = $base
for ($i = 1; $i -le 12; $i++) { $deep = Join-Path $deep ("很长的目录名称段-{0:D2}-用于逼近MAX_PATH边界" -f $i) }
New-Item -ItemType Directory -Force -Path $deep | Out-Null
$f2 = Join-Path $deep '长路径样本.txt'
Set-Content -LiteralPath $f2 -Value 'K-S01 长路径样本' -Encoding UTF8

# 样本 3：跨盘（C:）
$cbase = Join-Path $env:TEMP 'ks01-c-drive'
New-Item -ItemType Directory -Force -Path $cbase | Out-Null
$f3 = Join-Path $cbase '跨盘样本-C盘.txt'
Set-Content -LiteralPath $f3 -Value 'K-S01 跨盘样本' -Encoding UTF8

Write-Host "非ASCII : $f1  (len $($f1.Length))"
Write-Host "长路径  : len $($f2.Length)  存在 $(Test-Path -LiteralPath $f2)"
Write-Host "跨盘    : $f3  (len $($f3.Length))"
