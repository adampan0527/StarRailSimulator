param(
    [ValidateRange(1, 10000000)] [int] $Iterations = 250000,
    [ValidateRange(1, 10)] [int] $Runs = 3
)

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $projectRoot
try {
    cargo build -p star-rail-battle-core --example m02 --release --locked --target x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw 'The M02 Release build failed.' }
    $binary = (Resolve-Path -LiteralPath './target/x86_64-pc-windows-msvc/release/examples/m02.exe').Path

    $measurements = foreach ($mode in 'off', 'detailed') {
        for ($run = 1; $run -le $Runs; $run++) {
            $startInfo = [System.Diagnostics.ProcessStartInfo]::new($binary)
            $startInfo.UseShellExecute = $false
            $startInfo.CreateNoWindow = $true
            $startInfo.RedirectStandardOutput = $true
            $startInfo.RedirectStandardInput = $true
            foreach ($argValue in 'benchmark', $mode, [string] $Iterations, '--wait-for-peak') {
                $startInfo.ArgumentList.Add($argValue)
            }
            $process = [System.Diagnostics.Process]::Start($startInfo)
            try {
                $line = $process.StandardOutput.ReadLine()
                if (-not $line) { throw 'The benchmark produced no measurement.' }
                $result = $line | ConvertFrom-Json
                $process.Refresh()
                $peak = $process.PeakWorkingSet64
                if ($peak -le 0) { throw 'Windows did not report a process memory peak.' }
                $process.StandardInput.WriteLine()
                if (-not $process.WaitForExit(5000) -or $process.ExitCode -ne 0) {
                    throw 'The benchmark process did not exit cleanly.'
                }
                $result | Add-Member -NotePropertyName run -NotePropertyValue $run
                $result | Add-Member -NotePropertyName peak_working_set_bytes -NotePropertyValue $peak
                $result
            } finally {
                $process.StandardInput.Close()
                $process.Dispose()
            }
        }
    }
    $measurements | ConvertTo-Json -AsArray
} finally {
    Pop-Location
}
