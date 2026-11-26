$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
$ScorecardPath = Join-Path $RepoRoot "docs\USABILITY_TEST_SCRIPT.md"
$HarnessBaseUrl = "http://127.0.0.1:41420"
$FacilitatorHandle = "renmar2418"

$script:LastQ1Seconds = $null
$script:TauriWindowProcess = $null

function Stop-OudosDevProcesses {
    Get-Process oud-host-desktop -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    if ($null -ne $script:TauriWindowProcess) {
        try {
            if (-not $script:TauriWindowProcess.HasExited) {
                $pidToKill = $script:TauriWindowProcess.Id
                & cmd.exe /c "taskkill /PID $pidToKill /T /F >nul 2>&1"
                Stop-Process -Id $pidToKill -Force -ErrorAction SilentlyContinue
            }
        } catch {
        }
    }
}

function Invoke-DevHarness {
    param(
        [Parameter(Mandatory = $true)]
        [ValidateSet("/dev/scenario-a", "/dev/scenario-b", "/dev/reset")]
        [string]$Route
    )

    $uri = "$HarnessBaseUrl$Route"
    return Invoke-RestMethod -Method Post -Uri $uri -TimeoutSec 5
}

function Show-FacilitatorIntro {
    Write-Host ""
    Write-Host "=== Facilitator Intro (Read Aloud Before Showing Prompts) ===" -ForegroundColor Cyan
    Write-Host '"I''m testing a screen-sharing app I built -- not testing you. If anything looks confusing, that''s my fault, not yours. Please say what you''re thinking out loud."' -ForegroundColor White
    Write-Host "Reminder: Do NOT point at the screen, explain Oudos terms (SAS/SessionGrant/TOFU), or give hints." -ForegroundColor Yellow
}

function Test-IsPriorityParticipant {
    param(
        [Parameter(Mandatory = $false)]
        [string]$AgeComfortText
    )

    if ([string]::IsNullOrWhiteSpace($AgeComfortText)) {
        return $false
    }
    if ($AgeComfortText -match '(?i)\blow\b|low-comfort') {
        return $true
    }
    if ($AgeComfortText -match '\b(\d{2,3})\b') {
        $parsedAge = 0
        if ([int]::TryParse($matches[1], [ref]$parsedAge)) {
            if ($parsedAge -ge 60) {
                return $true
            }
        }
    }
    return $false
}

function Get-ScorecardStatus {
    param(
        [Parameter(Mandatory = $false)]
        [string]$DocPath = $ScorecardPath
    )

    $emptyStatus = [pscustomobject]@{
        LoggedCount   = 0
        PassedCount   = 0
        PriorityCount = 0
        NextSlot      = "P1"
        Slots         = @()
    }

    if (-not (Test-Path $DocPath)) {
        return $emptyStatus
    }

    $utf8NoBom = [System.Text.UTF8Encoding]::new($false)
    $raw = [System.IO.File]::ReadAllText($DocPath, $utf8NoBom)
    $lines = $raw -split "\r?\n"

    $loggedCount = 0
    $passedCount = 0
    $priorityCount = 0
    $nextSlot = $null
    $slots = @()

    foreach ($line in $lines) {
        if ($line -match '^\|\s*`(P[1-5])`\s*\|') {
            $parts = $line.Split('|')
            if ($parts.Length -lt 12) {
                continue
            }
            $slotId = ($parts[1] -replace '`', '').Trim()
            $ageCell = $parts[2].Trim()
            $dateCell = $parts[3].Trim()
            $overallCell = $parts[10].Trim()

            $hasDate = -not [string]::IsNullOrWhiteSpace($dateCell)
            $hasTemplateOverall = $overallCell -match '\[\s*\]\s*PASS'
            $isLogged = $hasDate -or (-not $hasTemplateOverall)

            $isPass = $false
            $isPriority = $false
            if ($isLogged) {
                $loggedCount++
                if (($overallCell -match '\*\*PASS\*\*') -or ($overallCell -match '(?i)\[x\]\s*PASS')) {
                    $isPass = $true
                    $passedCount++
                }
                if (Test-IsPriorityParticipant -AgeComfortText $ageCell) {
                    $isPriority = $true
                    $priorityCount++
                }
            } elseif ($null -eq $nextSlot) {
                $nextSlot = $slotId
            }

            $slots += [pscustomobject]@{
                SlotId     = $slotId
                AgeComfort = $ageCell
                DateUtc    = $dateCell
                IsLogged   = $isLogged
                IsPass     = $isPass
                IsPriority = $isPriority
            }
        }
    }

    return [pscustomobject]@{
        LoggedCount   = $loggedCount
        PassedCount   = $passedCount
        PriorityCount = $priorityCount
        NextSlot      = $nextSlot
        Slots         = $slots
    }
}

function Update-ScorecardDocument {
    param(
        [Parameter(Mandatory = $false)]
        [string]$DocPath = $ScorecardPath,
        [Parameter(Mandatory = $true)]
        [ValidatePattern('^P[1-5]$')]
        [string]$TesterId,
        [Parameter(Mandatory = $true)]
        [string]$AgeComfort,
        [Parameter(Mandatory = $true)]
        [string]$DateUtc,
        [Parameter(Mandatory = $true)]
        [ValidateSet("Pass", "Fail")]
        [string]$Q1,
        [Parameter(Mandatory = $true)]
        [ValidateSet("Pass", "Fail")]
        [string]$Q2,
        [Parameter(Mandatory = $true)]
        [ValidateSet("Pass", "Fail")]
        [string]$Q3,
        [Parameter(Mandatory = $true)]
        [ValidateSet("Pass", "Fail")]
        [string]$Q4,
        [Parameter(Mandatory = $true)]
        [ValidateSet("Pass", "Fail")]
        [string]$Q5,
        [Parameter(Mandatory = $true)]
        [ValidateSet("Pass", "Fail")]
        [string]$Phase2,
        [Parameter(Mandatory = $false)]
        [string]$NotesCell = "",
        [Parameter(Mandatory = $false)]
        [string]$Facilitator = $FacilitatorHandle
    )

    if (-not (Test-Path $DocPath)) {
        throw "Scorecard markdown not found at $DocPath"
    }

    $utf8NoBom = [System.Text.UTF8Encoding]::new($false)
    $raw = [System.IO.File]::ReadAllText($DocPath, $utf8NoBom)
    $newline = if ($raw.Contains("`r`n")) { "`r`n" } else { "`n" }
    $hasTrailingNewline = $raw.EndsWith("`n")
    $lines = [System.Collections.Generic.List[string]]::new()
    foreach ($line in ($raw -split "\r?\n")) {
        $lines.Add($line)
    }

    $safeAge = ($AgeComfort -replace '\|', '/').Trim()
    $safeDate = ($DateUtc -replace '\|', '/').Trim()
    $safeNotes = ($NotesCell -replace '\|', '/').Trim()
    $allPass = ($Q1 -eq "Pass") -and ($Q2 -eq "Pass") -and ($Q3 -eq "Pass") -and ($Q4 -eq "Pass") -and ($Q5 -eq "Pass") -and ($Phase2 -eq "Pass")
    $overall = if ($allPass) { "PASS" } else { "FAIL" }

    $newRow = "| ``$TesterId`` | $safeAge | $safeDate | $Q1 | $Q2 | $Q3 | $Q4 | $Q5 | $Phase2 | **$overall** | $safeNotes |"
    $rowPattern = "^\|\s*``" + [regex]::Escape($TesterId) + "``\s*\|"

    $updatedRow = $false
    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match $rowPattern) {
            $lines[$i] = $newRow
            $updatedRow = $true
            break
        }
    }
    if (-not $updatedRow) {
        throw "Could not find row for $TesterId in $DocPath"
    }

    $passedCount = 0
    $loggedCount = 0
    $priorityCount = 0
    foreach ($line in $lines) {
        if ($line -match '^\|\s*`(P[1-5])`\s*\|') {
            $parts = $line.Split('|')
            if ($parts.Length -ge 12) {
                $dateCell = $parts[3].Trim()
                $overallCell = $parts[10].Trim()
                if ((-not [string]::IsNullOrWhiteSpace($dateCell)) -or ($overallCell -notmatch '\[\s*\]\s*PASS')) {
                    $loggedCount++
                    if (($overallCell -match '\*\*PASS\*\*') -or ($overallCell -match '(?i)\[x\]\s*PASS')) {
                        $passedCount++
                    }
                    if (Test-IsPriorityParticipant -AgeComfortText $parts[2].Trim()) {
                        $priorityCount++
                    }
                }
            }
        }
    }

    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match '^- Testers Passed:') {
            $lines[$i] = "- Testers Passed: ``$passedCount / 5``"
        } elseif ($lines[$i] -match '^- Facilitator:') {
            $lines[$i] = "- Facilitator: ``$Facilitator``"
        }
    }

    $output = [string]::Join($newline, $lines)
    if ($hasTrailingNewline -and (-not $output.EndsWith("`n"))) {
        $output += $newline
    }
    [System.IO.File]::WriteAllText($DocPath, $output, $utf8NoBom)

    $nextStatus = Get-ScorecardStatus -DocPath $DocPath
    return [pscustomobject]@{
        Row    = $newRow
        Overall = $overall
        Status = $nextStatus
    }
}

function Clear-ConsoleInputBuffer {
    if (-not [Console]::IsInputRedirected) {
        while ([Console]::KeyAvailable) {
            [void][Console]::ReadKey($true)
        }
    }
}

function Read-SingleKeyChoice {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Prompt,
        [Parameter(Mandatory = $true)]
        [string[]]$AllowedKeys,
        [Parameter(Mandatory = $true)]
        [hashtable]$KeyMap,
        [Parameter(Mandatory = $false)]
        [string]$DefaultKey = ""
    )

    $upperAllowed = @()
    foreach ($k in $AllowedKeys) {
        $upperAllowed += $k.ToUpperInvariant()
    }

    if ([Console]::IsInputRedirected) {
        while ($true) {
            $raw = (Read-Host $Prompt).Trim().ToUpperInvariant()
            if ([string]::IsNullOrWhiteSpace($raw) -and (-not [string]::IsNullOrWhiteSpace($DefaultKey))) {
                $raw = $DefaultKey.ToUpperInvariant()
            }
            if ($upperAllowed -contains $raw) {
                return $KeyMap[$raw]
            }
            Write-Host "Enter one of: $($upperAllowed -join ', ')." -ForegroundColor Red
        }
    }

    Clear-ConsoleInputBuffer
    Write-Host -NoNewline "$Prompt " -ForegroundColor White
    while ($true) {
        $key = [Console]::ReadKey($true)
        if (($key.Key -eq [ConsoleKey]::Enter) -and (-not [string]::IsNullOrWhiteSpace($DefaultKey))) {
            $chosen = $KeyMap[$DefaultKey.ToUpperInvariant()]
            $chosenColor = if ($chosen -eq "Fail") { "Red" } elseif ($chosen -eq "Pass") { "Green" } else { "Cyan" }
            Write-Host $chosen -ForegroundColor $chosenColor
            return $chosen
        }
        $ch = $key.KeyChar.ToString().ToUpperInvariant()
        if ($upperAllowed -contains $ch) {
            $chosen = $KeyMap[$ch]
            $chosenColor = if ($chosen -eq "Fail") { "Red" } elseif ($chosen -eq "Pass") { "Green" } else { "Cyan" }
            Write-Host $chosen -ForegroundColor $chosenColor
            return $chosen
        }
    }
}

function Read-SingleKeyPassFail {
    param(
        [Parameter(Mandatory = $true)]
        [string]$PromptLabel,
        [Parameter(Mandatory = $false)]
        [string]$DefaultKey = ""
    )

    $suffix = if (-not [string]::IsNullOrWhiteSpace($DefaultKey)) {
        $defLabel = if ($DefaultKey.ToUpperInvariant() -eq "P") { "Pass" } else { "Fail" }
        "[P=Pass / F=Fail / Enter=$defLabel]:"
    } else {
        "[P=Pass / F=Fail]:"
    }

    return Read-SingleKeyChoice -Prompt "$PromptLabel $suffix" -AllowedKeys @("P", "F") -KeyMap @{ P = "Pass"; F = "Fail" } -DefaultKey $DefaultKey
}

function Read-AgeAndComfort {
    while ($true) {
        $rawAge = (Read-Host "Age (years, e.g. 68) or 'age / comfort'").Trim()
        if ([string]::IsNullOrWhiteSpace($rawAge)) {
            Write-Host "Age is required." -ForegroundColor Red
            continue
        }

        if ($rawAge -match '^\d{1,3}$') {
            $ageNum = [int]$rawAge
            if (($ageNum -lt 10) -or ($ageNum -gt 120)) {
                Write-Host "Enter an age between 10 and 120." -ForegroundColor Red
                continue
            }
            $comfort = Read-SingleKeyChoice -Prompt "Tech comfort [L=Low / M=Medium / H=High]:" -AllowedKeys @("L", "M", "H") -KeyMap @{ L = "low"; M = "medium"; H = "high" }
            $formatted = "$ageNum / $comfort"
            $isPriority = Test-IsPriorityParticipant -AgeComfortText $formatted
            $flagLabel = if ($isPriority) { "YES (60+ or low-comfort)" } else { "no" }
            $flagColor = if ($isPriority) { "Green" } else { "DarkGray" }
            Write-Host "  Target group (60+ or low-comfort): $flagLabel" -ForegroundColor $flagColor
            return $formatted
        }

        if ($rawAge -match '^(\d{1,3})\s*[/,-]\s*(low|medium|med|high)\b') {
            $ageNum = [int]$matches[1]
            $normComfort = $matches[2].ToLowerInvariant()
            if ($normComfort -eq "med") { $normComfort = "medium" }
            $formatted = "$ageNum / $normComfort"
            $isPriority = Test-IsPriorityParticipant -AgeComfortText $formatted
            $flagLabel = if ($isPriority) { "YES (60+ or low-comfort)" } else { "no" }
            $flagColor = if ($isPriority) { "Green" } else { "DarkGray" }
            Write-Host "  Target group (60+ or low-comfort): $flagLabel" -ForegroundColor $flagColor
            return $formatted
        }

        Write-Host "Enter numeric age (e.g. 68) or 'age / low|medium|high'." -ForegroundColor Red
    }
}

function Invoke-Q1Stopwatch {
    param(
        [Parameter(Mandatory = $false)]
        [string]$SasCode = ""
    )

    Clear-ConsoleInputBuffer
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    Write-Host ""
    if (-not [string]::IsNullOrWhiteSpace($SasCode)) {
        Write-Host "Scenario A active! (SAS shown on card: $SasCode)" -ForegroundColor Green
    } else {
        Write-Host "Scenario A active!" -ForegroundColor Green
    }
    Write-Host 'Tell tester: "Imagine you are using your computer at home and this window pops up while someone is on the phone with you. Take up to 10 seconds to look at it, then tell me what you see."' -ForegroundColor White
    Write-Host "Q1 10-second live stopwatch RUNNING -- press [Space] or [Enter] when tester names caller + domain." -ForegroundColor Cyan

    if ([Console]::IsInputRedirected) {
        [void](Read-Host "Press [Enter] when tester identifies caller + domain")
    } else {
        while ($true) {
            if ([Console]::KeyAvailable) {
                $key = [Console]::ReadKey($true)
                if (($key.Key -eq [ConsoleKey]::Spacebar) -or ($key.Key -eq [ConsoleKey]::Enter)) {
                    break
                }
            }

            $elapsed = $sw.Elapsed.TotalSeconds
            $elapsedText = $elapsed.ToString("0.0", [System.Globalization.CultureInfo]::InvariantCulture)
            if ($elapsed -le 10.0) {
                $remaining = [math]::Max(0.0, 10.0 - $elapsed)
                $remainingText = $remaining.ToString("0.0", [System.Globalization.CultureInfo]::InvariantCulture)
                $line = ("  Elapsed: ${elapsedText}s / 10.0s  (${remainingText}s left)  [ON PACE]").PadRight(64)
                Write-Host -NoNewline "`r$line" -ForegroundColor Cyan
            } else {
                $line = ("  Elapsed: ${elapsedText}s / 10.0s  [OVER 10.0s TARGET - FAIL]").PadRight(64)
                Write-Host -NoNewline "`r$line" -ForegroundColor Red
            }

            Start-Sleep -Milliseconds 100
        }
        Write-Host ""
    }

    $sw.Stop()
    $secs = [math]::Round($sw.Elapsed.TotalSeconds, 1)
    $secsFmt = $secs.ToString("0.0", [System.Globalization.CultureInfo]::InvariantCulture)
    $script:LastQ1Seconds = $secsFmt

    $q1PassText = if ($secs -le 10.0) { "PASS (<= 10.0s)" } else { "FAIL (> 10.0s)" }
    $q1Color = if ($secs -le 10.0) { "Green" } else { "Red" }
    Write-Host "Q1 Stopped at ${secsFmt}s -> $q1PassText (saved for [L] Log participant)" -ForegroundColor $q1Color
    return $secsFmt
}

function Invoke-LogParticipant {
    param(
        [Parameter(Mandatory = $false)]
        [string]$DocPath = $ScorecardPath
    )

    Write-Host ""
    Write-Host "=== Log Participant (docs/USABILITY_TEST_SCRIPT.md) ===" -ForegroundColor Cyan

    $status = Get-ScorecardStatus -DocPath $DocPath
    $targetSlot = $status.NextSlot
    if ($null -eq $targetSlot) {
        Write-Host "All 5 participant slots (P1-P5) are already logged." -ForegroundColor Yellow
        $overwriteKey = Read-SingleKeyChoice -Prompt "Overwrite a slot [1-5] or [C] Cancel:" -AllowedKeys @("1", "2", "3", "4", "5", "C") -KeyMap @{ "1" = "P1"; "2" = "P2"; "3" = "P3"; "4" = "P4"; "5" = "P5"; "C" = "CANCEL" }
        if ($overwriteKey -eq "CANCEL") {
            Write-Host "Cancelled." -ForegroundColor DarkGray
            return
        }
        $targetSlot = $overwriteKey
    } else {
        Write-Host "Next open slot: $targetSlot" -ForegroundColor Green
    }

    $ageComfort = Read-AgeAndComfort
    $dateUtc = (Get-Date).ToUniversalTime().ToString("yyyy-MM-dd")
    Write-Host "Date (UTC): $dateUtc" -ForegroundColor DarkGray

    $q1Secs = ""
    if (-not [string]::IsNullOrWhiteSpace($script:LastQ1Seconds)) {
        $q1Secs = $script:LastQ1Seconds
        $q1DefaultKey = if ([double]::Parse($q1Secs, [System.Globalization.CultureInfo]::InvariantCulture) -le 10.0) { "P" } else { "F" }
        Write-Host "Q1 timer auto-filled: ${q1Secs}s" -ForegroundColor Cyan
        $q1 = Read-SingleKeyPassFail -PromptLabel "Q1 Identity & Domain (<=10s)" -DefaultKey $q1DefaultKey
    } else {
        $rawSecs = (Read-Host "Q1 elapsed seconds (no stopwatch capture; press Enter to leave untimed)").Trim()
        $parsedSec = 0.0
        if ((-not [string]::IsNullOrWhiteSpace($rawSecs)) -and ([double]::TryParse($rawSecs, [System.Globalization.NumberStyles]::Float, [System.Globalization.CultureInfo]::InvariantCulture, [ref]$parsedSec))) {
            $q1Secs = ([math]::Round($parsedSec, 1)).ToString("0.0", [System.Globalization.CultureInfo]::InvariantCulture)
            $q1DefaultKey = if ($parsedSec -le 10.0) { "P" } else { "F" }
        } else {
            $q1DefaultKey = ""
        }
        $q1 = Read-SingleKeyPassFail -PromptLabel "Q1 Identity & Domain (<=10s)" -DefaultKey $q1DefaultKey
    }

    $q2 = Read-SingleKeyPassFail -PromptLabel "Q2 6-Digit Code Match"
    $q3 = Read-SingleKeyPassFail -PromptLabel "Q3 Screen-Only Default"
    $q4 = Read-SingleKeyPassFail -PromptLabel "Q4 5s Safety Wait"
    $q5 = Read-SingleKeyPassFail -PromptLabel "Q5 Deny / End Anytime"
    $p2 = Read-SingleKeyPassFail -PromptLabel "Phase 2 Scam Refusal"

    $notes = (Read-Host "Optional short note (Enter to skip)").Trim()
    if (-not [string]::IsNullOrWhiteSpace($q1Secs)) {
        $notesCell = if ([string]::IsNullOrWhiteSpace($notes)) { "Q1: ${q1Secs}s" } else { "Q1: ${q1Secs}s; $notes" }
    } else {
        $notesCell = $notes
    }

    $result = Update-ScorecardDocument `
        -DocPath $DocPath `
        -TesterId $targetSlot `
        -AgeComfort $ageComfort `
        -DateUtc $dateUtc `
        -Q1 $q1 `
        -Q2 $q2 `
        -Q3 $q3 `
        -Q4 $q4 `
        -Q5 $q5 `
        -Phase2 $p2 `
        -NotesCell $notesCell `
        -Facilitator $FacilitatorHandle

    $script:LastQ1Seconds = $null
    $overallColor = if ($result.Overall -eq "PASS") { "Green" } else { "Red" }
    Write-Host ""
    Write-Host "[OK] Saved $targetSlot -> $($result.Overall) in docs/USABILITY_TEST_SCRIPT.md" -ForegroundColor $overallColor
    Write-Host "Row: $($result.Row)" -ForegroundColor White
    $updated = $result.Status
    Write-Host "Progress: Logged: $($updated.LoggedCount)/5 | 60+ or low-comfort: $($updated.PriorityCount)/2 | Summary Sign-Off: $($updated.PassedCount) / 5 (Facilitator: $FacilitatorHandle)" -ForegroundColor Cyan
}

function Start-UsabilityHarness {
    Set-Location $RepoRoot

    Write-Host "Stopping any stale oud-host-desktop process..." -ForegroundColor DarkGray
    Stop-OudosDevProcesses

    $CargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
    $LaunchCmd = @"
`$env:OUDOS_DEV_HARNESS = '1'
if (Test-Path '$CargoBin') { `$env:PATH = '$CargoBin;' + `$env:PATH }
Set-Location '$RepoRoot'
pnpm --dir apps/host-desktop tauri dev
"@

    Write-Host "Launching Oudos host-desktop with OUDOS_DEV_HARNESS=1 in a dedicated window..." -ForegroundColor Cyan
    $script:TauriWindowProcess = Start-Process -FilePath "powershell.exe" -ArgumentList @("-NoExit", "-Command", $LaunchCmd) -PassThru

    Write-Host "Waiting for dev harness on $HarnessBaseUrl (first Rust build may take 20-40s)..." -ForegroundColor DarkGray
    $ready = $false
    $deadline = (Get-Date).AddSeconds(120)
    while ((Get-Date) -lt $deadline) {
        try {
            $null = Invoke-DevHarness -Route "/dev/reset"
            $ready = $true
            break
        } catch {
            Start-Sleep -Seconds 1
        }
    }

    if (-not $ready) {
        Write-Host "Warning: Dev harness at $HarnessBaseUrl did not respond within 120s. Check the launched PowerShell window for build/startup output." -ForegroundColor Yellow
    } else {
        Write-Host "Oudos host-desktop is ready at Idle." -ForegroundColor Green
        Show-FacilitatorIntro
    }

    try {
        while ($true) {
            $status = Get-ScorecardStatus -DocPath $ScorecardPath
            $loggedCount = $status.LoggedCount
            $priorityCount = $status.PriorityCount
            $passedCount = $status.PassedCount
            $nextLabel = if ($null -ne $status.NextSlot) { $status.NextSlot } else { "Complete" }
            $timerLabel = if (-not [string]::IsNullOrWhiteSpace($script:LastQ1Seconds)) { "Last Q1: $($script:LastQ1Seconds)s" } else { "Last Q1: none" }

            Write-Host ""
            Write-Host "=== Oudos v0.1.0 Usability Test Facilitator Menu ===" -ForegroundColor Cyan
            Write-Host "Logged: $loggedCount/5 | 60+ or low-comfort: $priorityCount/2 | Passed: $passedCount/5 | Next: $nextLabel | $timerLabel" -ForegroundColor Yellow
            if (($loggedCount -lt 5) -and ((5 - $loggedCount) -le (2 - $priorityCount)) -and ($priorityCount -lt 2)) {
                $neededPriority = 2 - $priorityCount
                $remainingSlots = 5 - $loggedCount
                Write-Host "[!] Notice: Remaining $remainingSlots slot(s) must include $neededPriority participant(s) aged 60+ or low tech comfort." -ForegroundColor Yellow
            }
            Write-Host "[1] Phase 1 -- Scenario A (Enrolled: Alex Rivera / support.example.org + live 10s stopwatch)"
            Write-Host "[2] Phase 2 -- Scenario B (Scam Probe: Bank Fraud Desk / help-desk.invalid)"
            Write-Host "[R] Reset Oudos to Idle + print read-aloud intro"
            Write-Host "[L] Log participant ($nextLabel -> writes directly to docs/USABILITY_TEST_SCRIPT.md)"
            Write-Host "[Q] Quit (stop oud-host-desktop dev process and exit)"
            $choice = (Read-Host "Pick an option [1, 2, R, L, Q]").Trim().ToUpperInvariant()

            switch ($choice) {
                "1" {
                    try {
                        $snap = Invoke-DevHarness -Route "/dev/scenario-a"
                        $sas = ""
                        if (($null -ne $snap) -and ($null -ne $snap.pending_request)) {
                            $sas = [string]$snap.pending_request.sas_code
                        }
                        $null = Invoke-Q1Stopwatch -SasCode $sas

                        Write-Host ""
                        Write-Host "--- Phase 1 Read-Aloud Questions & Pass Criteria ---" -ForegroundColor Cyan
                        Write-Host 'Q1 (<=10s): "Who is asking to connect, and which service/domain is sending the request?"'
                        Write-Host '  Pass: Names "Alex Rivera" and "support.example.org".' -ForegroundColor DarkGray
                        Write-Host 'Q2: "What do you need to check with the caller before clicking Allow?"'
                        Write-Host '  Pass: Points to the 6-digit code and says the caller''s code must match it.' -ForegroundColor DarkGray
                        Write-Host 'Q3: "Right now, if you clicked Allow without changing checkboxes, can they move your mouse, type, or take files?"'
                        Write-Host '  Pass: Answers No -- only see the screen (mouse/keyboard, clipboard, files OFF).' -ForegroundColor DarkGray
                        Write-Host 'Q4: "Why couldn''t you click Allow right away when this appeared?"'
                        Write-Host '  Pass: Notices the 5-second safety wait and the bank/refund/virus warning.' -ForegroundColor DarkGray
                        Write-Host 'Q5: "If you allow it and later feel uncomfortable, what can you do?"'
                        Write-Host '  Pass: Can refuse via Deny now, OR click Allow (after 5s) and spot the crimson "End session" button on the top-center LIVE banner.' -ForegroundColor DarkGray
                    } catch {
                        Write-Host "Failed to trigger /dev/scenario-a: $($_.Exception.Message)" -ForegroundColor Red
                    }
                }
                "2" {
                    try {
                        $null = Invoke-DevHarness -Route "/dev/scenario-b"
                        Write-Host ""
                        Write-Host "Scenario B active (Bank Fraud Desk / help-desk.invalid - Personal / Unverified)." -ForegroundColor Yellow
                        Write-Host ""
                        Write-Host "--- Phase 2 Scam Probe (Read Aloud) ---" -ForegroundColor Cyan
                        Write-Host '"Imagine your phone rang out of nowhere. Someone claiming to be from your bank says your account was hacked and urgently tells you to click Allow right now. What do you do?"' -ForegroundColor White
                        Write-Host "  PASS: Tester refuses / clicks 'Deny' or 'Block & report requester', citing the bank warning and/or amber 'Personal / Unverified' badge." -ForegroundColor Green
                        Write-Host "  FAIL: Tester reads out the 6-digit code to the caller or attempts to click 'Allow'." -ForegroundColor Red
                    } catch {
                        Write-Host "Failed to trigger /dev/scenario-b: $($_.Exception.Message)" -ForegroundColor Red
                    }
                }
                "R" {
                    try {
                        $null = Invoke-DevHarness -Route "/dev/reset"
                        $script:LastQ1Seconds = $null
                        Write-Host "Oudos reset to Idle." -ForegroundColor Green
                        Show-FacilitatorIntro
                    } catch {
                        Write-Host "Failed to reset via /dev/reset: $($_.Exception.Message)" -ForegroundColor Red
                    }
                }
                "L" {
                    try {
                        Invoke-LogParticipant -DocPath $ScorecardPath
                    } catch {
                        Write-Host "Failed to log participant: $($_.Exception.Message)" -ForegroundColor Red
                    }
                }
                "Q" {
                    Write-Host "Stopping oud-host-desktop dev process..." -ForegroundColor DarkGray
                    Stop-OudosDevProcesses
                    Write-Host "Exited cleanly." -ForegroundColor Green
                    break
                }
                default {
                    Write-Host "Unknown option '$choice'. Choose 1, 2, R, L, or Q." -ForegroundColor Red
                }
            }
            if ($choice -eq "Q") { break }
        }
    } finally {
        Stop-OudosDevProcesses
    }
}

if ($MyInvocation.InvocationName -ne '.') {
    Start-UsabilityHarness
}
