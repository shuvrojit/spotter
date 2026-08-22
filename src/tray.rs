use anyhow::{Context, Result};
use async_channel::Sender;
use ksni::{blocking::TrayMethods, menu::StandardItem, MenuItem, Tray};
use std::sync::OnceLock;

const MATERIAL_ICON_VIEWBOX: f64 = 24.0;
const MATERIAL_ICON_SIZES: [i32; 4] = [16, 22, 32, 48];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Event {
    Open,
    Settings,
    Quit,
}

struct SpotterTray {
    events: Sender<Event>,
}

impl SpotterTray {
    fn send(&self, event: Event) {
        let _ = self.events.try_send(event);
    }
}

impl Tray for SpotterTray {
    fn id(&self) -> String {
        "spotter".to_string()
    }

    fn title(&self) -> String {
        "Spotter".to_string()
    }

    fn icon_name(&self) -> String {
        String::new()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        static ICONS: OnceLock<Vec<ksni::Icon>> = OnceLock::new();
        ICONS
            .get_or_init(|| {
                MATERIAL_ICON_SIZES
                    .into_iter()
                    .map(material_search_icon)
                    .collect()
            })
            .clone()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.send(Event::Open);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: "Open Spotter".to_string(),
                icon_name: "system-search".to_string(),
                activate: Box::new(|tray: &mut Self| tray.send(Event::Open)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Settings".to_string(),
                icon_name: "preferences-system".to_string(),
                activate: Box::new(|tray: &mut Self| tray.send(Event::Settings)),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".to_string(),
                icon_name: "application-exit".to_string(),
                activate: Box::new(|tray: &mut Self| tray.send(Event::Quit)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

fn material_search_icon(size: i32) -> ksni::Icon {
    const SAMPLES_PER_AXIS: usize = 4;
    const HANDLE: [(f64, f64); 4] = [
        (13.73, 14.43),
        (19.49, 20.19),
        (20.90, 18.78),
        (15.17, 13.05),
    ];

    let side = size as usize;
    let scale = MATERIAL_ICON_VIEWBOX / size as f64;
    let mut data = Vec::with_capacity(side * side * 4);

    for pixel_y in 0..side {
        for pixel_x in 0..side {
            let mut covered = 0;
            for sample_y in 0..SAMPLES_PER_AXIS {
                for sample_x in 0..SAMPLES_PER_AXIS {
                    let x = (pixel_x as f64 + (sample_x as f64 + 0.5) / SAMPLES_PER_AXIS as f64)
                        * scale;
                    let y = (pixel_y as f64 + (sample_y as f64 + 0.5) / SAMPLES_PER_AXIS as f64)
                        * scale;
                    let distance = ((x - 9.5).powi(2) + (y - 9.5).powi(2)).sqrt();
                    let in_lens = (4.5..=6.5).contains(&distance);
                    if in_lens || point_in_convex_polygon(x, y, &HANDLE) {
                        covered += 1;
                    }
                }
            }

            let alpha = (covered * 255 / (SAMPLES_PER_AXIS * SAMPLES_PER_AXIS)) as u8;
            // StatusNotifierItem pixmaps use ARGB32 in network byte order.
            data.extend_from_slice(&[alpha, 255, 255, 255]);
        }
    }

    ksni::Icon {
        width: size,
        height: size,
        data,
    }
}

fn point_in_convex_polygon(x: f64, y: f64, points: &[(f64, f64)]) -> bool {
    let mut has_clockwise_edge = false;
    let mut has_counterclockwise_edge = false;
    for (&(start_x, start_y), &(end_x, end_y)) in points.iter().zip(points.iter().cycle().skip(1)) {
        let cross = (end_x - start_x) * (y - start_y) - (end_y - start_y) * (x - start_x);
        has_clockwise_edge |= cross < 0.0;
        has_counterclockwise_edge |= cross > 0.0;
        if has_clockwise_edge && has_counterclockwise_edge {
            return false;
        }
    }
    true
}

pub(crate) struct Service {
    _handle: ksni::blocking::Handle<SpotterTray>,
}

pub(crate) fn start(events: Sender<Event>) -> Result<Service> {
    let handle = SpotterTray { events }
        .spawn()
        .context("register StatusNotifierItem")?;
    Ok(Service { _handle: handle })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activation_and_settings_menu_send_events() {
        let (sender, receiver) = async_channel::bounded(1);
        let mut tray = SpotterTray { events: sender };

        tray.activate(0, 0);

        assert_eq!(receiver.try_recv().unwrap(), Event::Open);
        let mut menu = tray.menu();
        let MenuItem::Standard(settings) = menu.remove(1) else {
            panic!("Settings should be a standard tray menu item");
        };
        assert_eq!(settings.label, "Settings");
        (settings.activate)(&mut tray);
        assert_eq!(receiver.try_recv().unwrap(), Event::Settings);
    }

    #[test]
    fn tray_uses_monochrome_material_search_pixmaps() {
        let (sender, _) = async_channel::bounded(1);
        let tray = SpotterTray { events: sender };

        assert!(tray.icon_name().is_empty());
        let icons = tray.icon_pixmap();
        assert_eq!(
            icons
                .iter()
                .map(|icon| (icon.width, icon.height))
                .collect::<Vec<_>>(),
            MATERIAL_ICON_SIZES
                .into_iter()
                .map(|size| (size, size))
                .collect::<Vec<_>>()
        );
        for icon in icons {
            assert_eq!(icon.data.len(), (icon.width * icon.height * 4) as usize);
            assert!(icon.data.chunks_exact(4).any(|pixel| pixel[0] > 0));
            assert!(icon
                .data
                .chunks_exact(4)
                .all(|pixel| pixel[1..] == [255, 255, 255]));
        }
    }

    #[test]
    fn material_search_icon_has_a_hollow_lens_and_handle() {
        let icon = material_search_icon(24);
        let alpha_at = |x: usize, y: usize| icon.data[(y * 24 + x) * 4];

        assert_eq!(alpha_at(9, 9), 0);
        assert!(alpha_at(18, 18) > 0);
    }
}
