mod editor_state;
mod plugin;
mod tabs;
mod fonts;

use machinery_api::foundation::ColorSrgbT;

struct TokenColor {
    scope: &'static str,
    color: ColorSrgbT,
}

const fn hex_token_color(scope: &'static str, color: u32) -> TokenColor {
    let bytes = color.to_le_bytes();
    TokenColor {
        scope,
        color: ColorSrgbT {
            r: bytes[3],
            g: bytes[2],
            b: bytes[1],
            a: bytes[0],
        },
    }
}
