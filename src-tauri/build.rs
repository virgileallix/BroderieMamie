use std::io::Write;

fn main() {
    // Clé Gemini par défaut, fournie au moment de la fabrication par le secret GitHub
    // GEMINI_API_KEY. Elle n'est jamais écrite dans le code : on la range brouillée
    // dans le dossier de compilation (pour qu'elle n'apparaisse pas en clair dans le .exe).
    println!("cargo:rerun-if-env-changed=GEMINI_API_KEY");
    let key = std::env::var("GEMINI_API_KEY").unwrap_or_default();
    let key = key.trim();
    let mixed: Vec<u8> = key.bytes().enumerate().map(|(i, b)| b ^ mask(i)).collect();
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("gemini_key.bin");
    std::fs::File::create(out).unwrap().write_all(&mixed).unwrap();
    tauri_build::build()
}

fn mask(i: usize) -> u8 {
    (0xA5u8 ^ (i as u8).wrapping_mul(31)).rotate_left((i % 7) as u32)
}
