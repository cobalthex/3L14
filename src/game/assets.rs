use game_3l14::engine::AsIterator;
use game_3l14::engine::assets::*;
use game_3l14::engine::graphics::debug_gui::DebugGui;

// TODO: macroize this

pub(crate) struct GameAssets<'a>
{
    pub textures: TextureLifecycler<'a>,
    pub materials: MaterialLifecycler,
    pub shaders: ShaderLifecycler<'a>,
}
impl<'a> AssetLifecyclers for GameAssets<'a> { }
impl<'a> AssetLifecyclerLookup<texture::Texture> for GameAssets<'a>
{
    fn lifecycler(&self) -> &impl AssetLifecycler<texture::Texture> { &self.textures }
}
impl<'a> AssetLifecyclerLookup<Material> for GameAssets<'a>
{
    fn lifecycler(&self) -> &impl AssetLifecycler<Material> { &self.materials }
}
impl<'a> AssetLifecyclerLookup<Shader> for GameAssets<'a>
{
    fn lifecycler(&self) -> &impl AssetLifecycler<Shader> { &self.shaders }
}
pub struct GameAssetsIterator<'i, 'a>
{
    assets: &'i GameAssets<'a>,
    which: usize,
}
impl<'i, 'a> Iterator for GameAssetsIterator<'i, 'a>
{
    type Item = &'i dyn DebugGui<'a>;

    fn next(&mut self) -> Option<Self::Item>
    {
        let next: Option<Self::Item> = match self.which
        {
            0 => Some(&self.assets.textures),
            // 1 => Some(&self.assets.materials),
            _ => None
        };
        self.which += 1;
        next
    }
}
impl<'i, 'a: 'i> AsIterator<'i> for GameAssets<'a>
{
    type Item = &'i dyn DebugGui<'a>;
    type AsIter = GameAssetsIterator<'i, 'a>;

    fn as_iter(&'i self) -> Self::AsIter
    {
        Self::AsIter
        {
            assets: self,
            which: 0,
        }
    }
}
