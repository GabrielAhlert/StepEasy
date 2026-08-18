<#
.SYNOPSIS
    Junta trechos separados de um take num video unico.

.DESCRIPTION
    Um GIF de README precisa contar a historia inteira em poucos segundos, e a
    historia raramente esta contigua no take: comeco, meio e fim ficam a minutos
    de distancia. Este script recorta os trechos e emenda, preservando a ordem
    informada.

    A saida e um intermediario em CRF 18 - visualmente sem perda - para servir de
    entrada ao gerar-midia.ps1 sem acumular artefato de recompressao.

.PARAMETER Trechos
    Pares "inicio-fim", em segundos. Ex.: @("5-12", "100-114", "126-132")

.EXAMPLE
    .\montar-trechos.ps1 -Entrada .\take.mp4 -Trechos @("5-12","100-114","126-132")
#>
param(
    [Parameter(Mandatory)][string]$Entrada,
    [Parameter(Mandatory)][string[]]$Trechos,
    [string]$Saida = "dist/demo/composto.mp4"
)

$ErrorActionPreference = "Stop"
if (-not (Test-Path $Entrada)) { throw "arquivo nao encontrado: $Entrada" }
New-Item -ItemType Directory -Force -Path (Split-Path $Saida) | Out-Null

$filtros = @()
$rotulos = ""
for ($i = 0; $i -lt $Trechos.Count; $i++) {
    $partes = $Trechos[$i] -split '-'
    if ($partes.Count -ne 2) { throw "trecho invalido: $($Trechos[$i]) (use inicio-fim)" }
    $ini = [double]$partes[0]
    $fim = [double]$partes[1]
    if ($fim -le $ini) { throw "trecho invalido: $($Trechos[$i]) (fim <= inicio)" }

    # setpts zera o relogio de cada pedaco; sem isso o concat deixa buracos.
    $filtros += "[0:v]trim=start=${ini}:end=${fim},setpts=PTS-STARTPTS[t$i]"
    $rotulos += "[t$i]"
    "trecho {0}: {1}s -> {2}s ({3}s)" -f ($i + 1), $ini, $fim, ($fim - $ini)
}
$filtros += "$rotulos concat=n=$($Trechos.Count):v=1:a=0[saida]"
$grafo = $filtros -join ";"

& ffmpeg -hide_banner -loglevel error -y -i $Entrada `
    -filter_complex $grafo -map "[saida]" `
    -c:v libx264 -preset slow -crf 18 -pix_fmt yuv420p -an $Saida
if ($LASTEXITCODE -ne 0) { throw "ffmpeg falhou ao emendar os trechos" }

$dur = & ffprobe -v error -show_entries format=duration -of csv=p=0 $Saida
"`nresultado: {0}  ({1:N1}s, {2:N1} MB)" -f $Saida, [double]$dur, ((Get-Item $Saida).Length / 1MB)
