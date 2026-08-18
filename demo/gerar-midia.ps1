<#
.SYNOPSIS
    Converte a gravacao bruta da demo em MP4 e GIF prontos para publicar.

.DESCRIPTION
    O arquivo que sai do OBS ou do ShareX e grande demais para o README e traz
    sobras no comeco e no fim. Este script corta, redimensiona e gera as duas
    formas que o GitHub aceita bem:

      - MP4 H.264, para anexar numa issue ou release e embutir como player;
      - GIF com paleta propria, para o topo do README, onde nao ha player.

    O GIF usa palettegen/paletteuse em vez da paleta fixa de 256 cores: numa
    captura de tela, a paleta padrao suja o texto e as bordas de cinza.

.EXAMPLE
    .\gerar-midia.ps1 -Entrada .\take-bruto.mkv -Inicio 00:00:03 -Duracao 90

.EXAMPLE
    # So o trecho do GIF, mais curto
    .\gerar-midia.ps1 -Entrada .\take-bruto.mkv -Inicio 00:00:28 -Duracao 15 -Sufixo destaque
#>
param(
    [Parameter(Mandatory)][string]$Entrada,
    [string]$Inicio = "00:00:00",
    [double]$Duracao = 0,
    [string]$Saida = "dist/demo",
    [string]$Sufixo = "",
    [int]$LarguraVideo = 1280,
    [int]$LarguraGif = 800,
    [int]$FpsGif = 12
)

$ErrorActionPreference = "Stop"

if (-not (Get-Command ffmpeg -ErrorAction SilentlyContinue)) {
    throw "ffmpeg nao encontrado no PATH. Instale com: winget install Gyan.FFmpeg"
}
if (-not (Test-Path $Entrada)) { throw "arquivo nao encontrado: $Entrada" }

New-Item -ItemType Directory -Force -Path $Saida | Out-Null
$nome = if ($Sufixo) { "stepeasy-demo-$Sufixo" } else { "stepeasy-demo" }
$mp4 = Join-Path $Saida "$nome.mp4"
$gif = Join-Path $Saida "$nome.gif"
$paleta = Join-Path $env:TEMP "stepeasy-paleta.png"

# -ss antes de -i busca pelo indice (rapido); -t limita a duracao.
$corte = @("-ss", $Inicio)
if ($Duracao -gt 0) { $corte += @("-t", $Duracao) }

Write-Host "-> MP4 ($LarguraVideo px)"
& ffmpeg -hide_banner -loglevel error -y @corte -i $Entrada `
    -vf "scale=${LarguraVideo}:-2:flags=lanczos" `
    -c:v libx264 -preset slow -crf 23 -pix_fmt yuv420p `
    -movflags +faststart -an $mp4
if ($LASTEXITCODE -ne 0) { throw "ffmpeg falhou ao gerar o MP4" }

Write-Host "-> paleta do GIF"
& ffmpeg -hide_banner -loglevel error -y @corte -i $Entrada `
    -vf "fps=$FpsGif,scale=${LarguraGif}:-1:flags=lanczos,palettegen=stats_mode=diff" `
    $paleta
if ($LASTEXITCODE -ne 0) { throw "ffmpeg falhou ao gerar a paleta" }

Write-Host "-> GIF ($LarguraGif px, $FpsGif fps)"
& ffmpeg -hide_banner -loglevel error -y @corte -i $Entrada -i $paleta `
    -lavfi "fps=$FpsGif,scale=${LarguraGif}:-1:flags=lanczos[v];[v][1:v]paletteuse=dither=bayer:bayer_scale=3" `
    $gif
if ($LASTEXITCODE -ne 0) { throw "ffmpeg falhou ao gerar o GIF" }

Remove-Item $paleta -ErrorAction SilentlyContinue

foreach ($f in @($mp4, $gif)) {
    $mb = (Get-Item $f).Length / 1MB
    "{0,-40} {1,6:N1} MB" -f (Split-Path $f -Leaf), $mb
    if ($f -eq $gif -and $mb -gt 10) {
        Write-Warning "GIF acima de 10 MB. Corte um trecho menor (-Duracao) ou baixe -FpsGif/-LarguraGif."
    }
}
