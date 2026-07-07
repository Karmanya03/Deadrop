use console::style;
use qrcode::{QrCode, render::unicode};

pub fn print_qr(url: &str) {
    let code = match QrCode::new(url.as_bytes()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("  {} QR generation failed: {}", style("⚠").yellow(), e);
            return;
        }
    };

    let image = code
        .render::<unicode::Dense1x2>()
        .dark_color(unicode::Dense1x2::Light)
        .light_color(unicode::Dense1x2::Dark)
        .quiet_zone(true)
        .build();

    eprintln!("{}", image);
    eprintln!("  {}", style("Scan to download on any device").dim());
    eprintln!();
}
