//! Listes du contenu **natif Kunos** à exclure du packaging d'export (§9.1/§11).
//! Porté de `drivers.py::isKunosDriver/isKunosCrew` et `fonts.py::isKunosFont`.
//! Un pilote/police/crew Kunos est livré avec le jeu : inutile (et faux) de
//! l'embarquer dans une archive de mod autonome.

/// Pilotes 3D natifs (`content/driver/<name>.kn5`).
pub fn is_kunos_driver(name: &str) -> bool {
    matches!(
        name,
        "2016_Driver"
            | "driver"
            | "driver_60"
            | "driver_70"
            | "driver_80"
            | "driver_back"
            | "driver_lod_b"
            | "driver_no_HANS"
            | "driver_ocolus"
            | "new_driver"
    )
}

/// Polices natives (`content/fonts/<name>.{png,txt,ttf}`).
pub fn is_kunos_font(name: &str) -> bool {
    const FONTS: &[&str] = &[
        "4c",
        "599_big",
        "599_mid",
        "650S_big",
        "650S_mid",
        "a",
        "aria",
        "arial",
        "arial_big",
        "audi_vln",
        "aventador",
        "aventador_b",
        "aventador_mid",
        "b",
        "bosch",
        "c7_big",
        "c7_mid",
        "c7_new",
        "comic",
        "console",
        "console_small",
        "default",
        "digital_big",
        "digital_big_f138",
        "digital_big_italic",
        "digital_mid",
        "digital_toyota",
        "digital_toyota_2",
        "e92_big",
        "e92_mid",
        "f312",
        "gallardo_1",
        "gallardo_2",
        "german_led",
        "german_led_mid",
        "ks_audi_r8_plus",
        "ks_corvette_c7",
        "ks_nissan_gtr",
        "ks_ruf12r",
        "led_audi",
        "led_big",
        "led_med",
        "mclarenmp4gt3",
        "mercedes_sls",
        "mg",
        "Microgramma",
        "Microsquare",
        "mp4_big",
        "porsche_big",
        "sls",
        "ttcup",
        "ttcup_big",
    ];
    FONTS.contains(&name)
}

/// Crews natifs (textures `content/texture/crew_<type><name>/`). `crew_type` ∈
/// {SUIT, HELMET, BRAND}. Les noms portent un backslash de tête (façon AC).
pub fn is_kunos_crew(name: &str, crew_type: &str) -> bool {
    match crew_type.to_ascii_uppercase().as_str() {
        "SUIT" => name.starts_with("\\type1\\") || name.starts_with("\\type2\\"),
        "HELMET" => matches!(
            name,
            "\\beige"
                | "\\black"
                | "\\blue"
                | "\\brown"
                | "\\cyan"
                | "\\green"
                | "\\grey"
                | "\\orange"
                | "\\purple"
                | "\\red"
                | "\\white"
                | "\\yellow"
        ),
        "BRAND" => matches!(
            name,
            "\\abarth"
                | "\\abarth2"
                | "\\alfa"
                | "\\alfa2"
                | "\\audi"
                | "\\audi2"
                | "\\bmw"
                | "\\chevy"
                | "\\chevy2"
                | "\\cobra"
                | "\\cobra2"
                | "\\ferrari"
                | "\\ferrari2"
                | "\\ford"
                | "\\ktm"
                | "\\lamborghini"
                | "\\lamborghini2"
                | "\\lotus"
                | "\\lotus_classic"
                | "\\maserati"
                | "\\maserati2"
                | "\\mazda"
                | "\\mazda2"
                | "\\mclaren"
                | "\\mclaren2"
                | "\\mercedes"
                | "\\mercedes2"
                | "\\nissan"
                | "\\nissan2"
                | "\\pagani"
                | "\\porsche"
                | "\\porsche2"
                | "\\praga"
                | "\\praga2"
                | "\\PSD"
                | "\\ruf"
                | "\\ruf2"
                | "\\scg"
                | "\\tatuus"
                | "\\toyota"
                | "\\toyota2"
        ),
        _ => false,
    }
}
