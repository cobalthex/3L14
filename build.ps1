#[CmdletBinding()]

param(
    [Parameter(ParameterSetName = 'Game')]
    [switch]$Game,

    [Parameter(ParameterSetName = 'Game')]
    [switch]$Run = $true,

    [Parameter(ParameterSetName = 'Test')]
    [switch]$Test,

    [Parameter(ParameterSetName = 'BuildAssets')]
    [string[]]$Asset,

    [Parameter(ParameterSetName = 'QueryAsset')]
    [string]$Query
)

switch ($PSCmdlet.ParameterSetName)
{
    'Game' {
        & cargo @('build','run')[$Run.IsPresent] --package 'game_3l14'
    }`
    'Test' {
        & cargo test --workspace
    }
    'BuildAssets' {
        cargo run --package assets_builder_3l14 -- build @($Asset | % { '--source',$_ })
    }
    'QueryAsset' {
        cargo run --package assets_browser_3l14 -- --asset-key $Query
    }
}
