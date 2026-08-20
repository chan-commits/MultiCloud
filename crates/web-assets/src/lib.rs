use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../frontend/web/dist/"]
pub struct Assets;
