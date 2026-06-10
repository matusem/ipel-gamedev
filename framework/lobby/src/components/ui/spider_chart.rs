use dioxus::prelude::*;

#[derive(Clone, Copy, PartialEq)]
pub struct SpiderAxis {
    pub label: &'static str,
    pub value: f32, // 1.0 – 5.0
}

#[component]
pub fn SpiderChart(axes: Vec<SpiderAxis>, size: u32) -> Element {
    let n = axes.len().max(3);
    let cx = size as f32 / 2.0;
    let cy = size as f32 / 2.0;
    let max_r = size as f32 * 0.36;
    let levels = [1.0, 2.0, 3.0, 4.0, 5.0];

    let angle = |i: usize| -> f32 {
        let t = i as f32 / n as f32;
        -std::f32::consts::FRAC_PI_2 + t * std::f32::consts::TAU
    };

    let point = |i: usize, scale: f32| -> (f32, f32) {
        let a = angle(i);
        let r = max_r * (scale / 5.0);
        (cx + r * a.cos(), cy + r * a.sin())
    };

    let data_points: Vec<(f32, f32)> = axes
        .iter()
        .enumerate()
        .map(|(i, ax)| point(i, ax.value.clamp(1.0, 5.0)))
        .collect();

    let polygon = data_points
        .iter()
        .map(|(x, y)| format!("{x},{y}"))
        .collect::<Vec<_>>()
        .join(" ");

    rsx! {
        div { class: "flex flex-col items-center gap-3",
            svg {
                class: "text-outline-variant",
                width: "{size}",
                height: "{size}",
                view_box: "0 0 {size} {size}",
                for level in levels {
                    {
                        let pts: String = (0..n)
                            .map(|i| {
                                let (x, y) = point(i, level);
                                format!("{x},{y}")
                            })
                            .collect::<Vec<_>>()
                            .join(" ");
                        rsx! {
                            polygon {
                                points: "{pts}",
                                fill: "none",
                                stroke: "currentColor",
                                stroke_width: "1",
                                opacity: "0.25",
                            }
                        }
                    }
                }
                for i in 0..n {
                    {
                        let (x, y) = point(i, 5.0);
                        rsx! {
                            line {
                                x1: "{cx}",
                                y1: "{cy}",
                                x2: "{x}",
                                y2: "{y}",
                                stroke: "currentColor",
                                stroke_width: "1",
                                opacity: "0.2",
                            }
                        }
                    }
                }
                polygon {
                    points: "{polygon}",
                    fill: "rgba(79, 70, 229, 0.25)",
                    stroke: "rgb(129, 140, 248)",
                    stroke_width: "2",
                }
                for (i, ax) in axes.iter().enumerate() {
                    {
                        let (x, y) = point(i, 5.8);
                        rsx! {
                            text {
                                x: "{x}",
                                y: "{y}",
                                text_anchor: "middle",
                                dominant_baseline: "middle",
                                class: "fill-on-surface-variant text-[9px] font-label-caps uppercase",
                                "{ax.label}"
                            }
                        }
                    }
                }
            }
            div { class: "grid grid-cols-5 gap-2 w-full max-w-xs text-center",
                for ax in axes {
                    div {
                        p { class: "text-[10px] text-outline uppercase", "{ax.label}" }
                        p { class: "font-mono-code text-sm text-primary", "{ax.value:.1}" }
                    }
                }
            }
        }
    }
}
