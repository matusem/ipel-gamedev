use dioxus::prelude::*;

#[derive(Clone, Copy, PartialEq)]
pub enum AvatarSize {
    Sm,
    Md,
    Lg,
    Xl,
    Hero,
}

impl AvatarSize {
    fn class(self) -> &'static str {
        match self {
            AvatarSize::Sm => "h-8 w-8 text-xs",
            AvatarSize::Md => "h-10 w-10 text-sm",
            AvatarSize::Lg => "h-14 w-14 text-lg",
            AvatarSize::Xl => "h-20 w-20 text-3xl",
            AvatarSize::Hero => "h-28 w-28 text-4xl",
        }
    }
}

pub fn avatar_color(seed: &str) -> &'static str {
    let hash: u32 = seed.bytes().fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    const COLORS: [&str; 6] = [
        "bg-primary-container/40 text-primary",
        "bg-secondary-container/30 text-secondary",
        "bg-tertiary-container/30 text-tertiary",
        "bg-surface-container-highest text-on-surface",
        "bg-primary-container/25 text-on-primary-container",
        "bg-surface-bright text-on-surface-variant",
    ];
    COLORS[(hash as usize) % COLORS.len()]
}

pub fn avatar_initial(seed: &str) -> String {
    seed.chars()
        .find(|c| c.is_alphanumeric())
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "P".into())
}

#[component]
pub fn Avatar(seed: String, size: AvatarSize, image_url: Option<String>) -> Element {
    let size_class = size.class();
    let color = avatar_color(&seed);
    let initial = avatar_initial(&seed);
    rsx! {
        if let Some(url) = image_url {
            img {
                class: "rounded-full object-cover border border-outline-variant/40 shrink-0 {size_class}",
                src: "{url}",
                alt: "",
            }
        } else {
            span {
                class: "rounded-full border border-outline-variant/40 flex items-center justify-center font-manrope font-bold shrink-0 {size_class} {color}",
                "{initial}"
            }
        }
    }
}
