/// 芯片系列及其下的具体型号（用于三列选择器）。
#[derive(Clone)]
pub struct ChipFamilyInfo {
    pub name: String,
    pub brand: String,
    pub chips: Vec<String>,
}

/// 枚举 probe-rs 内置芯片，按系列分组（按名称排序），并附上制造商品牌。
pub fn builtin_chip_families() -> Vec<ChipFamilyInfo> {
    let registry = probe_rs::config::Registry::from_builtin_families();
    let mut families: Vec<ChipFamilyInfo> = registry
        .families()
        .iter()
        .map(|f| {
            let mut chips: Vec<String> = f.variants.iter().map(|c| c.name.clone()).collect();
            chips.sort();
            chips.dedup();
            ChipFamilyInfo {
                name: f.name.trim_end_matches(" Series").to_owned(),
                brand: family_brand(f),
                chips,
            }
        })
        .collect();
    families.sort_by(|a, b| a.name.cmp(&b.name));
    families
}

/// 品牌及其下的系列在 chip_families 列表中的索引。
#[derive(Clone)]
pub struct ChipBrandInfo {
    pub name: String,
    pub families: Vec<usize>,
}

/// 将系列列表按品牌分组（按品牌名排序，"其他" 排最后）。
pub fn group_brands(families: &[ChipFamilyInfo]) -> Vec<ChipBrandInfo> {
    let mut brands: Vec<ChipBrandInfo> = Vec::new();
    for (i, f) in families.iter().enumerate() {
        match brands.iter_mut().find(|b| b.name == f.brand) {
            Some(b) => b.families.push(i),
            None => brands.push(ChipBrandInfo {
                name: f.brand.clone(),
                families: vec![i],
            }),
        }
    }
    brands.sort_by(|a, b| a.name.cmp(&b.name));
    if let Some(pos) = brands.iter().position(|b| b.name == "Other") {
        let b = brands.remove(pos);
        brands.push(b);
    }
    brands
}

/// 将系列归属到品牌。优先按系列名前缀匹配已知品牌表，
/// 其次推断通用 ARM/RISC-V 目标，最后回退到 JEP106 制造商信息。
fn family_brand(f: &probe_rs::config::ChipFamily) -> String {
    let name = f.name.trim_end_matches(" Series");
    for &(prefix, brand) in BRAND_RULES {
        if name.starts_with(prefix) {
            return brand.to_owned();
        }
    }
    let lower = name.to_lowercase();
    if lower.contains("generic") && lower.contains("arm") {
        return "ARM".to_owned();
    }
    if lower.contains("generic") && (lower.contains("risc-v") || lower.contains("riscv")) {
        return "RISC-V".to_owned();
    }
    if let Some(code) = f.manufacturer {
        if let Some(raw) = code.get() {
            return normalize_brand(raw);
        }
    }
    "Other".to_owned()
}

/// 系列名前缀 -> 品牌。必须按前缀长度从长到短排列。
const BRAND_RULES: &[(&str, &str)] = &[
    ("Generic RISC-V", "RISC-V"),
    ("Generic ARMv", "ARM"),
    ("MIMXRT", "NXP"),
    ("Raspberry", "Raspberry Pi"),
    ("Microchip", "Microchip"),
    ("OpenTitan", "lowRISC"),
    ("Trident", "Trident IoT"),
    ("Nuclei", "Nuclei"),
    ("STM32", "ST"),
    ("ADuCM", "Analog Devices"),
    ("MSP432", "TI"),
    ("MSPM0", "TI"),
    ("MAX326", "Maxim"),
    ("MAX780", "Maxim"),
    ("MAX326", "Maxim"),
    ("MAX32", "Maxim"),
    ("MAX78", "Maxim"),
    ("MAX7", "Maxim"),
    ("EFM32", "Silicon Labs"),
    ("EFR32", "Silicon Labs"),
    ("EFM8", "Silicon Labs"),
    ("EFM", "Silicon Labs"),
    ("GD32", "GigaDevice"),
    ("AT32", "Artery"),
    ("PIC32", "Microchip"),
    ("PIC24", "Microchip"),
    ("dsPIC", "Microchip"),
    ("ATSAM", "Microchip"),
    ("ATmega", "Microchip"),
    ("ATtiny", "Microchip"),
    ("AT90", "Microchip"),
    ("SAM", "Microchip"),
    ("MSP", "TI"),
    ("nRF", "Nordic"),
    ("LPC", "NXP"),
    ("MCX", "NXP"),
    ("OL23", "NXP"),
    ("S32K", "NXP"),
    ("iMX", "NXP"),
    ("TMS570", "TI"),
    ("CC13", "TI"),
    ("CC23", "TI"),
    ("LM3S", "TI"),
    ("Tiva", "TI"),
    ("AM2", "TI"),
    ("RA", "Renesas"),
    ("XMC", "Infineon"),
    ("FM3", "Infineon"),
    ("PSC3", "Infineon"),
    ("PSOC", "Infineon"),
    ("psoc", "Infineon"),
    ("TLE", "Infineon"),
    ("CY8", "Infineon"),
    ("HT32", "Holtek"),
    ("HT50", "Holtek"),
    ("HF", "Holtek"),
    ("HK32", "Hangshun"),
    ("HC32", "HDSC"),
    ("CH32", "WCH"),
    ("CH6", "WCH"),
    ("CW32", "Xinyuan"),
    ("AIR", "AirM2M"),
    ("HPM", "HPMicro"),
    ("SF32", "Siflower"),
    ("W75", "WIZnet"),
    ("Zynq", "AMD"),
    ("VA1", "Silicon Space"),
    ("VA4", "Silicon Space"),
    ("synwit", "Synwit"),
    ("SiFive", "SiFive"),
    ("fe3", "SiFive"),
    ("PAC5", "Qorvo"),
    ("PY32", "Puya"),
    ("ESP32", "Espressif"),
    ("ESP", "Espressif"),
    ("RP2", "Raspberry Pi"),
    ("ARM", "ARM"),
    ("RISC-V", "RISC-V"),
];

/// 将 JEP106 官方厂商名缩写为常用品牌名。
fn normalize_brand(raw: &str) -> String {
    match raw.trim() {
        "STMicroelectronics" => "ST",
        "Nordic VLSI ASA" => "Nordic",
        "Espressif Systems" => "Espressif",
        "NXP Semiconductors" => "NXP",
        "Microchip Technology Inc" => "Microchip",
        "Atmel Corporation" => "Microchip",
        "Renesas Technology Corp" => "Renesas",
        "Silicon Laboratories" => "Silicon Labs",
        "Texas Instruments" => "TI",
        "Infineon Technologies" => "Infineon",
        "Cypress Semiconductor" => "Infineon",
        "Dialog Semiconductor" => "Dialog",
        "Analog Devices Inc" => "Analog Devices",
        "Maxim Integrated Products" => "Maxim",
        "Nuvoton Technology Corp" => "Nuvoton",
        "GigaDevice Semiconductor" => "GigaDevice",
        "ARM Ltd" => "ARM",
        other => other,
    }
    .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brand_grouping_covers_all_families() {
        let families = builtin_chip_families();
        let brands = group_brands(&families);
        let total: usize = brands.iter().map(|b| b.families.len()).sum();
        assert_eq!(
            total,
            families.len(),
            "every family must be assigned to a brand"
        );

        let mut others: Vec<&str> = brands
            .iter()
            .filter(|b| b.name == "Other")
            .flat_map(|b| b.families.iter())
            .map(|&i| families[i].name.as_str())
            .collect();
        others.sort();
        assert_eq!(others, vec!["CIU32F0"], "unexpected unknown brand(s)");

        let brand_of = |name: &str| {
            families
                .iter()
                .position(|f| f.name == name)
                .and_then(|i| brands.iter().find(|b| b.families.contains(&i)))
                .map(|b| b.name.as_str())
        };
        assert_eq!(brand_of("STM32F1"), Some("ST"));
        assert_eq!(brand_of("nRF52"), Some("Nordic"));
        assert_eq!(brand_of("RP235x"), Some("Raspberry Pi"));
        assert_eq!(brand_of("MAX32660"), Some("Maxim"));
        assert_eq!(brand_of("psoc6_01"), Some("Infineon"));
        assert_eq!(brand_of("GD32F1x0"), Some("GigaDevice"));
        assert_eq!(brand_of("SAM3U"), Some("Microchip"));
        assert_eq!(brand_of("Generic ARMv8-M"), Some("ARM"));
    }
}
