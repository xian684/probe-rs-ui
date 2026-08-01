/// 界面语言。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Zh,
    En,
}

impl Lang {
    pub fn is_en(&self) -> bool {
        matches!(self, Lang::En)
    }

    /// 根据当前语言选择中文或英文文本。
    pub fn pick<T>(&self, zh: T, en: T) -> T {
        if self.is_en() {
            en
        } else {
            zh
        }
    }
}
