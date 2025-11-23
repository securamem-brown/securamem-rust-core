# PowerShell script to remove the duplicate landing page folder
# Run this after closing any programs that might be using the folder

$folderPath = "C:\Users\scorp\dbil\securamem-rust-core\securamem-ip-landing-page"

Write-Host "Attempting to remove: $folderPath" -ForegroundColor Yellow

if (Test-Path $folderPath) {
    try {
        Remove-Item -Path $folderPath -Recurse -Force -ErrorAction Stop
        Write-Host "✓ Successfully removed duplicate landing page folder!" -ForegroundColor Green
    }
    catch {
        Write-Host "✗ Failed to remove folder. Error: $($_.Exception.Message)" -ForegroundColor Red
        Write-Host ""
        Write-Host "The folder is likely being used by:" -ForegroundColor Yellow
        Write-Host "  - File Explorer (close any windows viewing this folder)"
        Write-Host "  - VS Code or another IDE (close the folder/project)"
        Write-Host "  - Command prompt/terminal (navigate away from this folder)"
        Write-Host ""
        Write-Host "After closing those programs, run this script again:" -ForegroundColor Cyan
        Write-Host "  powershell -ExecutionPolicy Bypass -File remove_landing_page.ps1"
    }
}
else {
    Write-Host "✓ Folder already removed or doesn't exist!" -ForegroundColor Green
}
