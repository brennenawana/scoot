//! The tray citizen — a StatusNotifierItem published over DBus (PORTS.md §8).
//!
//! On Linux this is Scoot's *primary* surface, not a fallback: the reprioritized
//! matrix makes GNOME Wayland a must-pass cell, and there the tray and the
//! chime are the whole product. So the menu carries the settings that PORTS.md
//! §7 says a v0.1-parity shell owes — interval, styles, autostart, telemetry —
//! rather than deferring them to a window that does not exist.
//!
//! The tray thread never decides anything. Every menu item drops a `Command`
//! into the app's channel and the coordinator asks the core what it means.

use std::sync::mpsc::Sender;

use crate::assets::{Image, TRAY_ATLASES, TRAY_FRAME_COUNT};
use crate::storage::{STYLE_BUDDY_OVERLAY, STYLE_ICON_BOUNCE, STYLE_SOUND};

/// Everything the user can ask for from the tray. Deliberately data, not
/// closures over app state: the tray runs on its own thread and must not hold
/// a lock on the scheduler.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    NudgeNow,
    IMoved,
    PauseFor(Option<f64>),
    Resume,
    SetInterval(u32),
    SetStyle(String, bool),
    SetLaunchAtLogin(bool),
    SetTelemetry(bool),
    Quit,
}

/// The interval presets PRODUCT.md §1 specifies, plus the 1-minute testing
/// interval VERIFY.md §3 leans on (and which makes the acceptance checklist
/// runnable in a session rather than an afternoon).
pub const INTERVAL_PRESETS: &[(u32, &str)] = &[
    (1, "1 min (testing)"),
    (20, "20 min"),
    (30, "30 min"),
    (45, "45 min"),
    (60, "60 min"),
    (90, "90 min"),
];

/// Pre-decoded tray art: `frames[frame][size]`. Decoded once at startup
/// because ksni asks for the pixmap set on every update and PNG decoding on
/// the DBus path would show up as tray lag during a bounce.
pub struct TrayIcons {
    frames: Vec<Vec<ksni::Icon>>,
}

impl TrayIcons {
    pub fn load() -> Self {
        let mut frames = Vec::with_capacity(TRAY_FRAME_COUNT as usize);
        for frame in 0..TRAY_FRAME_COUNT {
            let mut sizes = Vec::with_capacity(TRAY_ATLASES.len());
            for (size, bytes) in TRAY_ATLASES {
                let Some(cell) = Image::decode(bytes).and_then(|a| a.frame(frame, TRAY_FRAME_COUNT))
                else {
                    continue;
                };
                sizes.push(ksni::Icon {
                    width: *size as i32,
                    height: *size as i32,
                    data: cell.to_argb32(),
                });
            }
            frames.push(sizes);
        }
        Self { frames }
    }

    fn pixmaps(&self, frame: u32) -> Vec<ksni::Icon> {
        self.frames
            .get(frame as usize)
            .or_else(|| self.frames.first())
            .cloned()
            .unwrap_or_default()
    }
}

pub struct ScootTray {
    tx: Sender<Command>,
    icons: TrayIcons,
    /// Which atlas frame is showing: 0 resting, 1-4 the bounce.
    pub frame: u32,
    /// The one-line status the menu shows at the top ("Next nudge in 12 min").
    pub status: String,
    pub interval_minutes: u32,
    pub enabled_styles: Vec<String>,
    pub paused: bool,
    pub launch_at_login: bool,
    pub telemetry_enabled: bool,
    /// Styles the *cell* can actually deliver. A style the platform lacks is
    /// shown disabled with the reason, never silently missing — the
    /// degradation table is the spec, and the user deserves to see it.
    pub overlay_available: bool,
    pub auto_credit_available: bool,
}

impl ScootTray {
    pub fn new(tx: Sender<Command>) -> Self {
        Self {
            tx,
            icons: TrayIcons::load(),
            frame: 0,
            status: "Starting…".into(),
            interval_minutes: 45,
            enabled_styles: vec![STYLE_BUDDY_OVERLAY.into(), STYLE_SOUND.into()],
            paused: false,
            launch_at_login: true,
            telemetry_enabled: true,
            overlay_available: false,
            auto_credit_available: false,
        }
    }

    fn send(&self, c: Command) {
        // A closed channel means the coordinator is already shutting down;
        // there is nothing useful to do and nothing worth crashing over.
        let _ = self.tx.send(c);
    }

    fn is_enabled(&self, id: &str) -> bool {
        self.enabled_styles.iter().any(|s| s == id)
    }

    // The menu's wording decisions live here rather than inline in `menu()`,
    // because ksni's MenuItem is opaque (no Debug, private fields) and these
    // are exactly the choices that must not regress: a style the cell cannot
    // deliver has to stay visible *and* explain itself.

    fn overlay_style_label(&self) -> String {
        if self.overlay_available {
            "Buddy overlay".into()
        } else {
            "Buddy overlay (needs X11)".into()
        }
    }

    fn pause_item_label(&self) -> &'static str {
        if self.paused {
            "Resume"
        } else {
            "Pause"
        }
    }

    /// The honest degradation notice from PORTS.md §8's idle row.
    fn auto_credit_notice(&self) -> Option<&'static str> {
        if self.auto_credit_available {
            None
        } else {
            Some("Auto-credit off (no idle clock here)")
        }
    }
}

impl ksni::Tray for ScootTray {
    fn id(&self) -> String {
        "scoot".into()
    }

    fn title(&self) -> String {
        "Scoot".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        self.icons.pixmaps(self.frame)
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: "Scoot".into(),
            description: self.status.clone(),
            ..Default::default()
        }
    }

    /// Left click credits a move. PRODUCT.md §1 puts manual credit one click
    /// away ("click the buddy, or menu bar item → I moved"); on a cell with no
    /// overlay the tray icon *is* the buddy, so this is that click.
    fn activate(&mut self, _x: i32, _y: i32) {
        self.send(Command::IMoved);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;

        let mut items: Vec<ksni::MenuItem<Self>> = vec![
            StandardItem {
                label: self.status.clone(),
                enabled: false,
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Nudge now".into(),
                activate: Box::new(|t: &mut Self| t.send(Command::NudgeNow)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "I moved".into(),
                activate: Box::new(|t: &mut Self| t.send(Command::IMoved)),
                ..Default::default()
            }
            .into(),
        ];

        items.push(if self.paused {
            StandardItem {
                label: self.pause_item_label().into(),
                activate: Box::new(|t: &mut Self| t.send(Command::Resume)),
                ..Default::default()
            }
            .into()
        } else {
            SubMenu {
                label: self.pause_item_label().into(),
                submenu: vec![
                    StandardItem {
                        label: "For 1 hour".into(),
                        activate: Box::new(|t: &mut Self| {
                            t.send(Command::PauseFor(Some(3600.0)))
                        }),
                        ..Default::default()
                    }
                    .into(),
                    StandardItem {
                        label: "Until I resume".into(),
                        activate: Box::new(|t: &mut Self| t.send(Command::PauseFor(None))),
                        ..Default::default()
                    }
                    .into(),
                ],
                ..Default::default()
            }
            .into()
        });

        items.push(MenuItem::Separator);

        items.push(
            SubMenu {
                label: "Interval".into(),
                submenu: INTERVAL_PRESETS
                    .iter()
                    .map(|(minutes, label)| {
                        let m = *minutes;
                        CheckmarkItem {
                            label: (*label).to_string(),
                            checked: self.interval_minutes == m,
                            activate: Box::new(move |t: &mut Self| {
                                t.send(Command::SetInterval(m))
                            }),
                            ..Default::default()
                        }
                        .into()
                    })
                    .collect(),
                ..Default::default()
            }
            .into(),
        );

        // Styles the cell cannot deliver are shown, disabled, with the reason
        // in the label — "documented, not hidden" (PORTS.md §8).
        let overlay_label = self.overlay_style_label();
        items.push(
            SubMenu {
                label: "Nudge style".into(),
                submenu: vec![
                    CheckmarkItem {
                        label: overlay_label,
                        checked: self.is_enabled(STYLE_BUDDY_OVERLAY) && self.overlay_available,
                        enabled: self.overlay_available,
                        activate: Box::new(|t: &mut Self| {
                            let on = t.is_enabled(STYLE_BUDDY_OVERLAY);
                            t.send(Command::SetStyle(STYLE_BUDDY_OVERLAY.into(), !on));
                        }),
                        ..Default::default()
                    }
                    .into(),
                    CheckmarkItem {
                        label: "Chime".into(),
                        checked: self.is_enabled(STYLE_SOUND),
                        activate: Box::new(|t: &mut Self| {
                            let on = t.is_enabled(STYLE_SOUND);
                            t.send(Command::SetStyle(STYLE_SOUND.into(), !on));
                        }),
                        ..Default::default()
                    }
                    .into(),
                    CheckmarkItem {
                        label: "Icon bounce".into(),
                        checked: self.is_enabled(STYLE_ICON_BOUNCE),
                        activate: Box::new(|t: &mut Self| {
                            let on = t.is_enabled(STYLE_ICON_BOUNCE);
                            t.send(Command::SetStyle(STYLE_ICON_BOUNCE.into(), !on));
                        }),
                        ..Default::default()
                    }
                    .into(),
                ],
                ..Default::default()
            }
            .into(),
        );

        items.push(
            CheckmarkItem {
                label: "Start at login".into(),
                checked: self.launch_at_login,
                activate: Box::new(|t: &mut Self| {
                    t.send(Command::SetLaunchAtLogin(!t.launch_at_login))
                }),
                ..Default::default()
            }
            .into(),
        );

        items.push(
            CheckmarkItem {
                label: "Share anonymous counts".into(),
                checked: self.telemetry_enabled,
                activate: Box::new(|t: &mut Self| {
                    t.send(Command::SetTelemetry(!t.telemetry_enabled))
                }),
                ..Default::default()
            }
            .into(),
        );

        if let Some(notice) = self.auto_credit_notice() {
            items.push(
                StandardItem {
                    label: notice.into(),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
            );
        }

        items.push(MenuItem::Separator);
        items.push(
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|t: &mut Self| t.send(Command::Quit)),
                ..Default::default()
            }
            .into(),
        );

        items
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn tray() -> (ScootTray, mpsc::Receiver<Command>) {
        let (tx, rx) = mpsc::channel();
        (ScootTray::new(tx), rx)
    }

    #[test]
    fn every_frame_offers_every_size_as_argb32() {
        let icons = TrayIcons::load();
        for frame in 0..TRAY_FRAME_COUNT {
            let pixmaps = icons.pixmaps(frame);
            assert_eq!(pixmaps.len(), TRAY_ATLASES.len(), "frame {frame}");
            for icon in pixmaps {
                assert_eq!(
                    icon.data.len(),
                    (icon.width * icon.height * 4) as usize,
                    "{}px icon payload",
                    icon.width
                );
            }
        }
    }

    #[test]
    fn an_out_of_range_frame_falls_back_to_resting() {
        let icons = TrayIcons::load();
        let shape = |v: Vec<ksni::Icon>| {
            v.into_iter()
                .map(|i| (i.width, i.height, i.data))
                .collect::<Vec<_>>()
        };
        assert_eq!(shape(icons.pixmaps(99)), shape(icons.pixmaps(0)));
    }

    #[test]
    fn the_bounce_frames_actually_differ_from_resting() {
        // If the atlas ever regressed to five identical cells the icon-bounce
        // style would silently become a no-op.
        let icons = TrayIcons::load();
        let resting = icons.pixmaps(0)[0].data.clone();
        let moved: Vec<_> = (1..TRAY_FRAME_COUNT)
            .map(|f| icons.pixmaps(f)[0].data.clone())
            .collect();
        assert!(
            moved.iter().any(|d| *d != resting),
            "no bounce frame differs from the resting frame"
        );
    }

    #[test]
    fn left_click_credits_a_move() {
        use ksni::Tray;
        let (mut t, rx) = tray();
        t.activate(0, 0);
        assert_eq!(rx.try_recv().unwrap(), Command::IMoved);
    }

    #[test]
    fn the_overlay_style_is_disabled_and_explained_where_it_cannot_run() {
        let (mut t, _rx) = tray();
        t.overlay_available = false;
        assert_eq!(t.overlay_style_label(), "Buddy overlay (needs X11)");

        t.overlay_available = true;
        assert_eq!(t.overlay_style_label(), "Buddy overlay");
    }

    #[test]
    fn a_cell_without_an_idle_clock_says_so_in_the_menu() {
        let (mut t, _rx) = tray();
        t.auto_credit_available = false;
        assert!(t.auto_credit_notice().is_some());

        t.auto_credit_available = true;
        assert!(t.auto_credit_notice().is_none());
    }

    #[test]
    fn pause_becomes_resume_once_paused() {
        let (mut t, _rx) = tray();
        assert_eq!(t.pause_item_label(), "Pause");
        t.paused = true;
        assert_eq!(t.pause_item_label(), "Resume");
    }

    #[test]
    fn the_menu_builds_in_every_capability_combination() {
        use ksni::Tray;
        // ksni's MenuItem is opaque, so the assertion available here is that
        // building never panics and the item count moves with the notice.
        for overlay in [false, true] {
            for auto in [false, true] {
                let (mut t, _rx) = tray();
                t.overlay_available = overlay;
                t.auto_credit_available = auto;
                let n = t.menu().len();
                assert!(n > 5, "menu suspiciously short: {n}");
                t.paused = true;
                assert!(!t.menu().is_empty());
            }
        }

        let (mut t, _rx) = tray();
        t.auto_credit_available = true;
        let with_credit = t.menu().len();
        t.auto_credit_available = false;
        assert_eq!(
            t.menu().len(),
            with_credit + 1,
            "the auto-credit notice should add exactly one item"
        );
    }

    #[test]
    fn interval_presets_cover_the_product_spec() {
        let minutes: Vec<u32> = INTERVAL_PRESETS.iter().map(|(m, _)| *m).collect();
        for required in [20, 30, 45, 60, 90] {
            assert!(minutes.contains(&required), "missing {required} min preset");
        }
    }
}
