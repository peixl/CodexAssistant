param([string]$Missing = "C:\Users\asd\AppData\Local\Temp\missing.txt")

$ErrorActionPreference = "Stop"
$cache = Join-Path $env:USERPROFILE '.cargo\registry\cache\mirrors.tuna.tsinghua.edu.cn-4dc01642fd091eda'
if (-not (Test-Path $cache)) { New-Item -ItemType Directory -Path $cache | Out-Null }

[System.Net.ServicePointManager]::SecurityProtocol = [System.Net.SecurityProtocolType]::Tls12

$names = Get-Content $Missing
$total = $names.Count
$idx = 0
$ok = 0
$fail = 0
$failed = @()

foreach ($file in $names) {
  $idx++
  if ([string]::IsNullOrWhiteSpace($file)) { continue }
  $dst = Join-Path $cache $file
  if ((Test-Path $dst) -and (Get-Item $dst).Length -gt 0) { $ok++; continue }
  # filename = <name>-<version>.crate
  if ($file -notmatch '^(.+)-(\d[^.]*(?:\.[^.]+)*)\.crate$') {
    Write-Host "[$idx/$total] SKIP (unparsable): $file"
    $fail++
    $failed += $file
    continue
  }
  $name = $matches[1]
  $ver = $matches[2]
  $url = "https://static.crates.io/crates/$name/$name-$ver.crate"
  try {
    Invoke-WebRequest -Uri $url -OutFile $dst -UseBasicParsing -TimeoutSec 30
    $ok++
    if ($ok % 20 -eq 0) { Write-Host "[$idx/$total] downloaded $name $ver" }
  } catch {
    $fail++
    $failed += $file
    Write-Host "[$idx/$total] FAIL $name $ver  -> $($_.Exception.Message)"
  }
}

Write-Host "DONE ok=$ok fail=$fail"
if ($failed.Count -gt 0) {
  $failed | Out-File -FilePath (Join-Path (Split-Path $Missing) 'missing-failed.txt')
  Write-Host "Failed list -> $(Join-Path (Split-Path $Missing) 'missing-failed.txt')"
}
