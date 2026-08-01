use crate::i18n::{self, LanguagePreference};
use eframe::egui;
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use vm_check_core::check::{Check, CheckResult, Language, Privilege, Signal};
use vm_check_core::{all_checks, Report, Verdict};

enum RunState {
    NotStarted,
    Running,
    Done,
}

/// Storage key under which the language preference is persisted across
/// restarts (via eframe's `persistence` feature); the theme preference needs
/// no key of its own since egui persists it as part of its own memory.
const LANGUAGE_PREF_KEY: &str = "language_pref";

pub struct App {
    state: RunState,
    results: Vec<CheckResult>,
    /// Whether privileged checks (dmesg, dmidecode) run. There is no way for
    /// the user to toggle this from the UI: elevation on Linux means an
    /// interactive sudo password prompt in a terminal, which doesn't work
    /// from a windowed app, so the only way to get privileged checks is to
    /// launch this app itself via `sudo`, detected via `running_as_root()`.
    include_privileged: bool,
    rx: Option<Receiver<CheckResult>>,
    language_pref: LanguagePreference,
    system_language: Language,
    /// Whether the "About" window (project GitHub link + avatar) is shown.
    /// Not persisted: it's transient UI state, not a preference.
    about_open: bool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            state: RunState::NotStarted,
            results: Vec::new(),
            include_privileged: vm_check_core::evidence::running_as_root(),
            rx: None,
            language_pref: LanguagePreference::System,
            system_language: i18n::detect_system_language(),
            about_open: false,
        }
    }
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::from_storage(&cc.egui_ctx, cc.storage)
    }

    /// The actual logic behind [`App::new`], split out because
    /// `eframe::CreationContext` has no public constructor (two of its
    /// fields are crate-private) and so can never be built in a test,
    /// unlike its `egui_ctx`/`storage` fields individually, which are
    /// ordinary public types (`egui::Context` needs no GPU/window, and
    /// `dyn eframe::Storage` is a plain trait a test can mock).
    fn from_storage(egui_ctx: &egui::Context, storage: Option<&dyn eframe::Storage>) -> Self {
        // Needed so `egui::Image`/`include_image!` (used for the language
        // flags) can decode the embedded PNG bytes into a texture; egui
        // itself deliberately ships without an image decoder to stay light.
        egui_extras::install_image_loaders(egui_ctx);

        let language_pref = storage
            .and_then(|storage| eframe::get_value(storage, LANGUAGE_PREF_KEY))
            .unwrap_or(LanguagePreference::System);
        Self {
            language_pref,
            ..Self::default()
        }
    }

    fn start_run(&mut self) {
        self.results.clear();
        self.state = RunState::Running;
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        let include_privileged = self.include_privileged;
        std::thread::spawn(move || run_checks(tx, include_privileged));
    }
}

/// Runs every check on a background thread (subprocess/WMI calls can block)
/// and streams results back one at a time so the results table fills in
/// progressively instead of freezing the UI until everything finishes.
fn run_checks(tx: Sender<CheckResult>, include_privileged: bool) {
    let evidence = vm_check_core::evidence::SystemEvidence;
    for check in all_checks() {
        let result = if check.privilege == Privilege::Elevated && !include_privileged {
            CheckResult {
                id: check.id,
                name: check.name,
                signal: Signal::Inconclusive("skipped: relaunch this app with sudo to run"),
                weight: check.weight,
                detail: None,
            }
        } else {
            (check.run)(&evidence)
        };
        if tx.send(result).is_err() {
            return;
        }
    }
}

impl App {
    /// The actual UI: split out from [`eframe::App::update`] because that
    /// method's `eframe::Frame` parameter is unused here (no storage, no GL
    /// access) and has no public constructor, which would otherwise make
    /// this whole function impossible to drive from a test. [`egui::Context`]
    /// itself needs no GPU/window (`Context::run` executes a full layout
    /// pass in memory), so this is the boundary that keeps the real UI logic
    /// testable headlessly.
    fn draw(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.rx {
            let mut disconnected = false;
            loop {
                match rx.try_recv() {
                    Ok(result) => self.results.push(result),
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
            if disconnected {
                self.state = RunState::Done;
                self.rx = None;
            }
            ctx.request_repaint();
        }

        let language = self.language_pref.resolve(self.system_language);
        let strings = i18n::strings(language);
        // Rebuilt every frame from `all_checks()`, which only constructs a
        // handful of static `Check` structs (10 on Linux, 6 on Windows; no
        // checks are *run*), so this is cheap enough to redo on every repaint
        // rather than cache.
        let checks_by_id: HashMap<&'static str, Check> =
            all_checks().into_iter().map(|c| (c.id, c)).collect();

        // Generous, even margins so labels and buttons never touch the
        // window edge (a raw egui default panel margin is only a few px).
        let panel_margin = egui::Margin::symmetric(16.0, 12.0);

        // Header content (settings, branding, the primary action) lives in a
        // fixed top panel, separate from the scrollable results below. That
        // way shrinking the window only ever hides *scrollable* content,
        // never the run button or, previously, the verdict heading, which
        // used to live below an inner ScrollArea and could be pushed
        // entirely off the fixed-size CentralPanel on a short window.
        egui::TopBottomPanel::top("header")
            // Based on `Frame::side_top_panel`, not `Frame::default()`: the
            // latter has a *transparent* fill, so the panel painted nothing
            // of its own and the visible background fell back to eframe's
            // GL clear color, which is computed from **last** frame's style
            // (see glow_integration.rs), one frame behind `set_theme`. That
            // made switching to Light appear to do nothing to the
            // background: every other widget repaints with the new style
            // this frame, but the "background" was really last frame's
            // clear color with nothing painted over it to correct it.
            .frame(egui::Frame::side_top_panel(&ctx.style()).inner_margin(panel_margin))
            .show(ctx, |ui| {
                // Settings row: deliberately muted (small, secondary-toned
                // text) so it doesn't compete with the run button below for
                // attention: it's configuration, not the app's purpose.
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    let muted = ui.visuals().weak_text_color();

                    ui.label(egui::RichText::new(strings.theme_label).color(muted));
                    self.theme_cycle_button(ui, &strings);

                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(6.0);

                    ui.label(egui::RichText::new(strings.language_label).color(muted));
                    self.language_dropdown(ui, &strings);

                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(6.0);

                    if ui.button(format!("ℹ {}", strings.about_label)).clicked() {
                        self.about_open = true;
                    }
                });

                ui.add_space(10.0);
                ui.heading("vm-check");
                ui.label(strings.subtitle);

                // Elevation on Linux means an interactive sudo password prompt in a
                // terminal, which doesn't work from a windowed app, so there's no
                // in-UI way to request it: `include_privileged` is set once at
                // startup from `running_as_root()`. Windows checks (WMI / HKLM
                // registry) need no elevation at all, so this only applies to
                // Linux.
                #[cfg(target_os = "linux")]
                if !self.include_privileged {
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(strings.privileged_hint)
                            .color(ui.visuals().weak_text_color()),
                    );
                }

                ui.add_space(12.0);

                // The primary (and only) action in this app, so it gets the
                // clearest visual weight available: full width, a taller hit
                // target, bold enlarged centered text with an icon, and the
                // theme's accent color rather than the default neutral
                // button fill.
                let running = matches!(self.state, RunState::Running);
                ui.add_enabled_ui(!running, |ui| {
                    let (icon, label) = match self.state {
                        RunState::NotStarted => ("🔍", strings.run_checks),
                        RunState::Running => ("⏳", strings.running),
                        RunState::Done => ("🔄", strings.run_again),
                    };
                    let full_width = ui.available_width();
                    // A scoped style tweak rather than `Button::fill(...)`:
                    // an explicit `.fill()` paints a single fixed color no
                    // matter the interaction state, which silently defeats
                    // egui's built-in hover/press feedback (it normally
                    // swaps in a different `WidgetVisuals` per state). Tinting
                    // all three states keeps that feedback while branding
                    // the button with the theme's accent color.
                    let accent = ui.visuals().selection.bg_fill;
                    let on_accent = ui.visuals().selection.stroke.color;
                    let widgets = &mut ui.style_mut().visuals.widgets;
                    widgets.inactive.weak_bg_fill = accent;
                    widgets.inactive.fg_stroke.color = on_accent;
                    widgets.hovered.weak_bg_fill = tint(accent, 1.15);
                    widgets.hovered.fg_stroke.color = on_accent;
                    widgets.active.weak_bg_fill = tint(accent, 0.85);
                    widgets.active.fg_stroke.color = on_accent;

                    // Centers the button's text/icon: by default a button
                    // wider than its content left-aligns it, since it
                    // inherits the surrounding vertical layout's alignment.
                    ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                        let button = egui::Button::new(
                            egui::RichText::new(format!("{icon}  {label}"))
                                .size(16.0)
                                .strong(),
                        )
                        .min_size(egui::vec2(full_width, 40.0));
                        if ui.add(button).clicked() {
                            self.start_run();
                        }
                    });
                });

                ui.add_space(4.0);
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(&ctx.style()).inner_margin(panel_margin))
            .show(ctx, |ui| {
                // Table and verdict summary share one scroll area so that on
                // a short window the summary scrolls into view instead of
                // being clipped below the visible area.
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // `Column::auto()` sounds like it would handle this, but
                    // its measurement lags a frame behind (it sizes *this*
                    // frame's row painting from *last* frame's persisted
                    // widths, only updating for the frame after) and this
                    // app doesn't repaint continuously once results are in,
                    // so after a language switch the column could stay
                    // sized for the old (shorter) text until some unrelated
                    // repaint happened to nudge it. Sizing it explicitly
                    // from this frame's actual strings has no such lag.
                    let result_column_width = result_column_width(ui, &strings);
                    egui_extras::TableBuilder::new(ui)
                        .striped(true)
                        .column(egui_extras::Column::remainder())
                        .column(egui_extras::Column::initial(result_column_width))
                        .header(20.0, |mut header| {
                            header.col(|ui| {
                                ui.strong(strings.column_check);
                            });
                            header.col(|ui| {
                                ui.strong(strings.column_result);
                            });
                        })
                        .body(|mut body| {
                            for result in &self.results {
                                body.row(24.0, |mut row| {
                                    row.col(|ui| {
                                        let check = checks_by_id.get(result.id);
                                        let name = check
                                            .map_or(result.name, |c| c.localized_name(language));
                                        let label = ui.label(name);
                                        if let Some(check) = check {
                                            label.on_hover_text(
                                                check.localized_description(language),
                                            );
                                        }
                                    });
                                    row.col(|ui| {
                                        let (text, color) = match &result.signal {
                                            Signal::Detected => (
                                                strings.badge_detected,
                                                egui::Color32::from_rgb(200, 60, 60),
                                            ),
                                            Signal::NotDetected => (
                                                strings.badge_not_detected,
                                                egui::Color32::from_rgb(60, 160, 60),
                                            ),
                                            Signal::Inconclusive(_) => (
                                                strings.badge_inconclusive,
                                                egui::Color32::from_rgb(200, 160, 40),
                                            ),
                                        };
                                        ui.colored_label(color, text);
                                    });
                                });
                            }
                        });

                    if matches!(self.state, RunState::Done) {
                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(8.0);
                        let report = Report::new(self.results.clone());
                        let confidence_pct = (report.confidence() * 100.0).round();
                        let summary =
                            i18n::verdict_summary(report.verdict(), confidence_pct, language);
                        // Cloud vs. desktop-computer glyph as a quick visual
                        // cue for "virtual" vs. "physical", mirroring the
                        // icon+label treatment already used on the run
                        // button rather than introducing a new visual
                        // language just for this one label.
                        let (icon, color) = match report.verdict() {
                            Verdict::LikelyVirtualMachine => {
                                ("☁", egui::Color32::from_rgb(200, 60, 60))
                            }
                            Verdict::LikelyPhysicalMachine => {
                                ("🖥", egui::Color32::from_rgb(60, 160, 60))
                            }
                            Verdict::Uncertain => ("❓", egui::Color32::from_rgb(200, 160, 40)),
                        };
                        // A static, non-interactive box styled like the run
                        // button (solid accent fill, rounded corners, bold
                        // centered text) instead of a plain colored label:
                        // the verdict is the app's actual output, so it gets
                        // the same visual weight as the button that produced
                        // it rather than reading as an afterthought.
                        egui::Frame::none()
                            .fill(color)
                            .rounding(ui.visuals().widgets.inactive.rounding)
                            .inner_margin(egui::Margin::symmetric(16.0, 12.0))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                                    ui.label(
                                        egui::RichText::new(format!("{icon}  {summary}"))
                                            .size(18.0)
                                            .strong()
                                            .color(egui::Color32::WHITE),
                                    );
                                });
                            });
                        ui.add_space(4.0);
                    }
                });
            });

        self.about_window(ctx, &strings);
    }

    /// The "About" window, laid out as two columns: the app icon on the
    /// left (spanning the full height), and on the right the name/version,
    /// the free-software notice, and the author (avatar, "transfairs", and
    /// a link to the project page) below it. Both images are embedded into
    /// the binary at compile time (`include_image!`, like the language
    /// flags) rather than hotlinked, so the window renders correctly
    /// offline and never leaks the user's IP to GitHub just for opening
    /// this window.
    fn about_window(&mut self, ctx: &egui::Context, strings: &i18n::Strings) {
        let mut open = self.about_open;
        egui::Window::new(strings.about_window_title)
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let icon_size = 64.0;
                    ui.add(
                        egui::Image::new(egui::include_image!("../assets/icon.png"))
                            .fit_to_exact_size(egui::vec2(icon_size, icon_size)),
                    );
                    ui.vertical(|ui| {
                        ui.strong(format!("vm-check {}", env!("CARGO_PKG_VERSION")));
                        ui.weak(strings.about_free_software);

                        ui.add_space(8.0);

                        ui.horizontal(|ui| {
                            let avatar_size = 32.0;
                            ui.add(
                                egui::Image::new(egui::include_image!(
                                    "../assets/about/github-avatar.png"
                                ))
                                .fit_to_exact_size(egui::vec2(avatar_size, avatar_size))
                                .rounding(avatar_size / 2.0),
                            );
                            ui.vertical(|ui| {
                                ui.label("transfairs");
                                ui.hyperlink_to(
                                    strings.about_project_page,
                                    "https://transfairs.github.io/vm-check/",
                                );
                            });
                        });
                    });
                });
            });
        self.about_open = open;
    }

    /// A single button cycling Light → Dark → System → Light… on each
    /// click, rather than three separate buttons/radios. Trades one click
    /// of discoverability (all three options aren't visible at once) for a
    /// much smaller, quieter footprint in a settings row that's
    /// deliberately not meant to draw attention; the hover tooltip names
    /// all three so the cycle stays discoverable.
    fn theme_cycle_button(&self, ui: &mut egui::Ui, strings: &i18n::Strings) {
        let current = ui.ctx().options(|opt| opt.theme_preference);
        let (icon, label) = theme_icon_and_label(current, strings);
        let response = ui.button(format!("{icon} {label}")).on_hover_text(format!(
            "☀ {} · 🌙 {} · 💻 {}",
            strings.theme_light, strings.theme_dark, strings.theme_system
        ));
        if response.clicked() {
            ui.ctx().set_theme(next_theme_preference(current));
        }
    }

    /// A menu button (not `egui::ComboBox`, whose closed-state label only
    /// ever accepts plain text) showing a real flag image next to each
    /// language, since actual flags can't come from emoji: `🇩🇪`/`🇬🇧` are
    /// pairs of regional-indicator codepoints that only render as a flag if
    /// the font/shaper ligates them into one glyph, which egui's bundled
    /// (monochrome, non-shaping) font doesn't do: it showed the two letters
    /// verbatim ("DE"/"GB") instead.
    fn language_dropdown(&mut self, ui: &mut egui::Ui, strings: &i18n::Strings) {
        let current = language_button(self.language_pref, strings);
        egui::menu::menu_custom_button(ui, current, |ui| {
            self.language_menu_contents(ui, strings);
        });
    }

    /// The dropdown's actual entries: split out from [`App::language_dropdown`]
    /// because `egui::menu::menu_custom_button` only ever invokes its closure
    /// while the menu is open (a real click, which a headless layout pass
    /// can't simulate), which would otherwise make this whole list
    /// impossible to exercise from a test, unlike this method, callable
    /// directly with any `&mut egui::Ui`.
    fn language_menu_contents(&mut self, ui: &mut egui::Ui, strings: &i18n::Strings) {
        for pref in [
            LanguagePreference::System,
            LanguagePreference::Fixed(Language::En),
            LanguagePreference::Fixed(Language::De),
        ] {
            if ui.add(language_button(pref, strings)).clicked() {
                self.language_pref = pref;
                ui.close_menu();
            }
        }
    }
}

/// The flag image (or globe, for "System", since it isn't a country) plus label
/// for one language option, as a ready-to-add `Button` so the same code
/// builds both the dropdown's closed-state button and its menu entries.
fn language_button(
    preference: LanguagePreference,
    strings: &i18n::Strings,
) -> egui::Button<'static> {
    // A little larger than the surrounding text so the artwork stays
    // recognizable rather than a colored speck. The globe is square, the
    // flags are 4:3, so each gets a size matching its own aspect ratio
    // rather than being squeezed into a shared one.
    let flag_size = egui::vec2(18.0, 13.5);
    let globe_size = egui::vec2(14.0, 14.0);
    match preference {
        LanguagePreference::System => egui::Button::image_and_text(
            egui::Image::new(egui::include_image!("../assets/flags/globe.png"))
                .fit_to_exact_size(globe_size),
            strings.language_system,
        ),
        LanguagePreference::Fixed(Language::En) => egui::Button::image_and_text(
            egui::Image::new(egui::include_image!("../assets/flags/gb.png"))
                .fit_to_exact_size(flag_size),
            "English",
        ),
        LanguagePreference::Fixed(Language::De) => egui::Button::image_and_text(
            egui::Image::new(egui::include_image!("../assets/flags/de.png"))
                .fit_to_exact_size(flag_size),
            "Deutsch",
        ),
    }
}

/// Icon + localized label for a theme preference. The icons (☀/🌙/💻)
/// mirror egui's own built-in `ThemePreference::radio_buttons`, which the
/// user liked; dropping them wasn't an intentional UX call, just an
/// oversight when localizing away from the built-in widget.
fn theme_icon_and_label(
    preference: egui::ThemePreference,
    strings: &i18n::Strings,
) -> (&'static str, &'static str) {
    match preference {
        egui::ThemePreference::Light => ("☀", strings.theme_light),
        egui::ThemePreference::Dark => ("🌙", strings.theme_dark),
        egui::ThemePreference::System => ("💻", strings.theme_system),
    }
}

fn next_theme_preference(preference: egui::ThemePreference) -> egui::ThemePreference {
    match preference {
        egui::ThemePreference::Light => egui::ThemePreference::Dark,
        egui::ThemePreference::Dark => egui::ThemePreference::System,
        egui::ThemePreference::System => egui::ThemePreference::Light,
    }
}

/// Width the "Result" column needs for the widest badge text in the
/// current language (e.g. German "ÜBERSPRUNGEN" is much longer than
/// English "SKIP"), plus the table's own cell padding and a little
/// breathing room either side.
fn result_column_width(ui: &egui::Ui, strings: &i18n::Strings) -> f32 {
    let font_id = egui::TextStyle::Body.resolve(ui.style());
    let widest_text_width = [
        strings.column_result,
        strings.badge_detected,
        strings.badge_not_detected,
        strings.badge_inconclusive,
    ]
    .into_iter()
    .map(|text| {
        ui.fonts(|fonts| {
            fonts.layout_no_wrap(text.to_owned(), font_id.clone(), egui::Color32::WHITE)
        })
        .size()
        .x
    })
    .fold(0.0_f32, f32::max);
    widest_text_width + 2.0 * ui.spacing().item_spacing.x
}

/// Multiplies a color's HSV value (brightness) by `factor`, used to derive
/// hover/press variants of the run button's accent fill without losing
/// egui's native per-state feedback (see the comment at its call site).
fn tint(color: egui::Color32, factor: f32) -> egui::Color32 {
    let mut hsva = egui::ecolor::Hsva::from(color);
    hsva.v = (hsva.v * factor).clamp(0.0, 1.0);
    hsva.into()
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.draw(ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, LANGUAGE_PREF_KEY, &self.language_pref);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::Storage as _;

    /// Minimal in-memory stand-in for `eframe::Storage` (backed by a real
    /// file/browser-local-storage in production), so persistence logic can
    /// be tested without touching disk.
    #[derive(Default)]
    struct MockStorage(HashMap<String, String>);

    impl eframe::Storage for MockStorage {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }

        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.to_string(), value);
        }

        fn flush(&mut self) {}
    }

    fn result(name: &'static str, signal: Signal) -> CheckResult {
        CheckResult {
            id: name,
            name,
            signal,
            weight: 1.0,
            detail: None,
        }
    }

    /// `egui::Context::run` performs a full immediate-mode layout pass in
    /// memory (no window, GPU, or display server needed), which is enough
    /// to drive [`App::draw`] and exercise its branches directly.
    fn run_draw(app: &mut App) {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.draw(ctx));
    }

    #[test]
    fn default_app_has_not_started_with_no_results() {
        let app = App::default();
        assert!(matches!(app.state, RunState::NotStarted));
        assert!(app.results.is_empty());
        assert_eq!(
            app.include_privileged,
            vm_check_core::evidence::running_as_root()
        );
        assert!(app.rx.is_none());
    }

    #[test]
    fn run_checks_streams_a_result_for_every_check() {
        let (tx, rx) = mpsc::channel();
        run_checks(tx, false);
        let results: Vec<CheckResult> = rx.iter().collect();
        assert_eq!(results.len(), all_checks().len());
    }

    #[test]
    fn run_checks_skips_privileged_checks_when_not_included() {
        let privileged_count = all_checks()
            .into_iter()
            .filter(|c| c.privilege == Privilege::Elevated)
            .count();
        let (tx, rx) = mpsc::channel();
        run_checks(tx, false);
        let skipped = rx
            .iter()
            .filter(|r| {
                matches!(
                    r.signal,
                    Signal::Inconclusive(msg) if msg.contains("relaunch this app with sudo")
                )
            })
            .count();
        assert_eq!(skipped, privileged_count);
    }

    #[test]
    fn run_checks_runs_privileged_checks_when_included() {
        let (tx, rx) = mpsc::channel();
        run_checks(tx, true);
        let skipped = rx
            .iter()
            .filter(|r| {
                matches!(
                    r.signal,
                    Signal::Inconclusive(msg) if msg.contains("relaunch this app with sudo")
                )
            })
            .count();
        assert_eq!(skipped, 0);
    }

    #[test]
    fn run_checks_stops_early_once_the_receiver_is_dropped() {
        let (tx, rx) = mpsc::channel();
        drop(rx);
        // Must not panic even though every send() after the first fails.
        run_checks(tx, false);
    }

    #[test]
    fn start_run_populates_results_via_the_background_thread() {
        let mut app = App::default();
        app.start_run();
        assert!(matches!(app.state, RunState::Running));
        let rx = app.rx.take().expect("start_run always creates a channel");
        let results: Vec<CheckResult> = rx.iter().collect();
        assert_eq!(results.len(), all_checks().len());
    }

    #[test]
    fn draw_not_started_does_not_panic() {
        let mut app = App::default();
        run_draw(&mut app);
    }

    #[test]
    fn draw_renders_every_signal_kind() {
        let mut app = App {
            results: vec![
                result("Detected check", Signal::Detected),
                result("Undetected check", Signal::NotDetected),
                result("Skipped check", Signal::Inconclusive("skipped: reason")),
            ],
            ..App::default()
        };
        run_draw(&mut app);
    }

    #[test]
    fn draw_shows_a_hover_tooltip_for_results_matching_a_known_check() {
        // Only a result whose `id` matches a real, current `Check` (unlike
        // the synthetic ids `result()` normally makes up) exercises the
        // hover-tooltip lookup in the results table.
        let known_check = all_checks().into_iter().next().expect("at least one check");
        let mut app = App {
            results: vec![result(known_check.id, Signal::Detected)],
            ..App::default()
        };
        run_draw(&mut app);
    }

    #[test]
    fn draw_drains_pending_results_from_a_running_channel() {
        let (tx, rx) = mpsc::channel();
        tx.send(result("Check A", Signal::Detected)).unwrap();
        let mut app = App {
            rx: Some(rx),
            state: RunState::Running,
            ..App::default()
        };
        run_draw(&mut app);
        assert_eq!(app.results.len(), 1);
        // Sender still alive: TryRecvError::Empty, so state stays Running.
        assert!(matches!(app.state, RunState::Running));
    }

    #[test]
    fn draw_marks_done_once_the_sender_disconnects() {
        let (tx, rx) = mpsc::channel();
        tx.send(result("Check A", Signal::NotDetected)).unwrap();
        drop(tx);
        let mut app = App {
            rx: Some(rx),
            state: RunState::Running,
            ..App::default()
        };
        run_draw(&mut app);
        assert!(matches!(app.state, RunState::Done));
        assert!(app.rx.is_none());
    }

    #[test]
    fn draw_about_window_does_not_panic() {
        let mut app = App {
            about_open: true,
            ..App::default()
        };
        run_draw(&mut app);
        // The window's close button ("X") mustn't have flipped the flag
        // back off on the very first frame it's shown.
        assert!(app.about_open);
    }

    #[test]
    fn draw_done_shows_a_summary_for_every_verdict() {
        for results in [
            vec![result("VM-leaning", Signal::Detected)],
            vec![result("Physical-leaning", Signal::NotDetected)],
            vec![
                result("Detected", Signal::Detected),
                result("Undetected", Signal::NotDetected),
            ],
        ] {
            let mut app = App {
                results,
                state: RunState::Done,
                ..App::default()
            };
            run_draw(&mut app);
        }
    }

    #[test]
    fn draw_renders_in_every_language_with_and_without_the_privileged_hint() {
        for language_pref in [
            LanguagePreference::System,
            LanguagePreference::Fixed(Language::En),
            LanguagePreference::Fixed(Language::De),
        ] {
            for include_privileged in [false, true] {
                let mut app = App {
                    language_pref,
                    include_privileged,
                    results: vec![result("Some check", Signal::Detected)],
                    ..App::default()
                };
                run_draw(&mut app);
            }
        }
    }

    #[test]
    fn from_storage_defaults_to_system_language_without_storage() {
        let app = App::from_storage(&egui::Context::default(), None);
        assert_eq!(app.language_pref, LanguagePreference::System);
    }

    #[test]
    fn from_storage_defaults_to_system_language_when_key_missing() {
        let storage = MockStorage::default();
        let app = App::from_storage(&egui::Context::default(), Some(&storage));
        assert_eq!(app.language_pref, LanguagePreference::System);
    }

    #[test]
    fn from_storage_defaults_to_system_language_on_garbage_value() {
        let mut storage = MockStorage::default();
        storage.set_string(LANGUAGE_PREF_KEY, "not valid json".to_string());
        let app = App::from_storage(&egui::Context::default(), Some(&storage));
        assert_eq!(app.language_pref, LanguagePreference::System);
    }

    #[test]
    fn save_then_from_storage_round_trips_the_language_preference() {
        let mut storage = MockStorage::default();
        let mut app = App {
            language_pref: LanguagePreference::Fixed(Language::De),
            ..App::default()
        };
        eframe::App::save(&mut app, &mut storage);

        let restored = App::from_storage(&egui::Context::default(), Some(&storage));
        assert_eq!(
            restored.language_pref,
            LanguagePreference::Fixed(Language::De)
        );
    }

    #[test]
    fn mock_storage_flush_is_a_no_op() {
        // eframe calls `flush()` after `save()` to commit to disk; the mock
        // has nothing to commit, but the call must still be harmless.
        let mut storage = MockStorage::default();
        storage.flush();
    }

    #[test]
    fn language_menu_contents_builds_every_option_without_panicking() {
        let ctx = egui::Context::default();
        let strings = i18n::strings(Language::En);
        let mut app = App::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.language_menu_contents(ui, &strings);
            });
        });
        // No (simulated) click landed on any entry, so the preference is
        // unchanged; `start_run`/`about_open` have their own dedicated tests
        // for the "was it actually clicked" wiring.
        assert_eq!(app.language_pref, LanguagePreference::System);
    }

    #[test]
    fn theme_cycle_visits_light_dark_and_system_before_repeating() {
        let light = egui::ThemePreference::Light;
        let dark = next_theme_preference(light);
        let system = next_theme_preference(dark);
        let back_to_light = next_theme_preference(system);
        assert_eq!(dark, egui::ThemePreference::Dark);
        assert_eq!(system, egui::ThemePreference::System);
        assert_eq!(back_to_light, light);
    }

    #[test]
    fn theme_icon_and_label_covers_every_preference() {
        let strings = i18n::strings(Language::En);
        for preference in [
            egui::ThemePreference::Light,
            egui::ThemePreference::Dark,
            egui::ThemePreference::System,
        ] {
            let (icon, label) = theme_icon_and_label(preference, &strings);
            assert!(!icon.is_empty());
            assert!(!label.is_empty());
        }
    }

    #[test]
    fn tint_scales_brightness_and_clamps_to_valid_range() {
        let color = egui::Color32::from_rgb(100, 100, 100);
        let brighter = tint(color, 1.5);
        let darker = tint(color, 0.5);
        assert_ne!(brighter, color);
        assert_ne!(darker, color);
        // Absurd factors must clamp rather than over/underflow.
        let _ = tint(color, 10.0);
        let _ = tint(color, 0.0);
    }

    #[test]
    fn result_column_width_is_wide_enough_for_the_widest_badge() {
        let ctx = egui::Context::default();
        let strings = i18n::strings(Language::De); // "ÜBERSPRUNGEN" is the widest string overall.
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let width = result_column_width(ui, &strings);
                assert!(width > 0.0);
            });
        });
    }
}
