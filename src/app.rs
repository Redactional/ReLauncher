//! The ReLauncher window: the library on the left, the selected program's
//! details and actions on the right.

use crate::catalog::{Catalog, Program};
use crate::github::{self, Release};
use crate::install;
use crate::settings::{Installed, Settings};
use crate::theme::{self, ACCENT, ERROR, GAP, OK, STRONG, TEXT, WARN, WEAK};
use egui::{Color32, RichText, vec2};
use egui_material_icons::icons as ic;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};

const SELF_REPO: &str = env!("CARGO_PKG_REPOSITORY");

/// Results from background threads.
enum Msg {
    Catalog(Result<Catalog, String>),
    Release(String, Result<Release, String>),
    Icon(String, Vec<u8>),
    Requirements(String, Vec<bool>),
    Progress(String, String, f32),
    Done(String, Result<Done, String>),
    SelfUpdate(Release),
}

enum Done {
    Installed(String, PathBuf),
    Removed,
    Requirement,
}

struct Busy {
    id: String,
    stage: String,
    fraction: f32,
}

/// A program's state, as the library and the details show it.
enum State {
    NotInstalled,
    Installed(String),
    UpdateAvailable(String),
}

impl State {
    fn text(&self) -> (String, Color32) {
        match self {
            State::NotInstalled => ("Not installed".into(), WEAK),
            State::Installed(v) => (format!("Installed · {v}"), OK),
            State::UpdateAvailable(v) => (format!("Update available · {v}"), WARN),
        }
    }
}

pub struct App {
    settings: Settings,
    catalog: Option<Catalog>,
    catalog_error: Option<String>,
    releases: HashMap<String, Result<Release, String>>,
    icons: HashMap<String, egui::TextureHandle>,
    /// Per program: met or not, for each requirement that applies here.
    requirements: HashMap<String, Vec<bool>>,
    selected: String,
    search: String,
    busy: Option<Busy>,
    status: Option<(String, bool)>,
    confirm_uninstall: bool,
    settings_open: bool,
    self_update: Option<Release>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    ctx: egui::Context,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ctx = cc.egui_ctx.clone();
        theme::apply(&ctx);
        let (tx, rx) = channel();
        let mut app = Self {
            settings: Settings::load(),
            catalog: None,
            catalog_error: None,
            releases: HashMap::new(),
            icons: HashMap::new(),
            requirements: HashMap::new(),
            selected: String::new(),
            search: String::new(),
            busy: None,
            status: None,
            confirm_uninstall: false,
            settings_open: false,
            self_update: None,
            tx,
            rx,
            ctx,
        };
        app.refresh();
        if app.settings.check_self_update {
            app.spawn(|tx| {
                if let Ok(r) = github::latest(SELF_REPO, &crate::catalog::platform())
                    && github::newer(r.version(), env!("CARGO_PKG_VERSION"))
                {
                    let _ = tx.send(Msg::SelfUpdate(r));
                }
            });
        }
        app
    }

    fn spawn(&self, job: impl FnOnce(&Sender<Msg>) + Send + 'static) {
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            job(&tx);
            ctx.request_repaint();
        });
    }

    /// Reloads the program list, then each program's latest release and icon.
    fn refresh(&mut self) {
        self.catalog_error = None;
        self.releases.clear();
        self.spawn(|tx| {
            let _ = tx.send(Msg::Catalog(
                crate::catalog::load().map_err(|e| format!("{e:#}")),
            ));
        });
    }

    fn on_catalog(&mut self, catalog: Catalog) {
        let platform = crate::catalog::platform();
        for p in &catalog.programs {
            let icon_cache = icon_cache(&p.id);
            if !self.icons.contains_key(&p.id)
                && let Ok(png) = std::fs::read(&icon_cache)
                && let Some(t) = texture(&self.ctx, &p.id, &png)
            {
                self.icons.insert(p.id.clone(), t);
            }
            let (id, repo, platform) = (p.id.clone(), p.repo.clone(), platform.clone());
            self.spawn(move |tx| {
                let r = github::latest(&repo, &platform).map_err(|e| format!("{e:#}"));
                let _ = tx.send(Msg::Release(id, r));
            });
            if let Some(url) = p.icon.clone() {
                let id = p.id.clone();
                self.spawn(move |tx| {
                    if let Ok(png) = crate::http::get_bytes(&url) {
                        if let Some(dir) = icon_cache.parent() {
                            let _ = std::fs::create_dir_all(dir);
                        }
                        let _ = std::fs::write(&icon_cache, &png);
                        let _ = tx.send(Msg::Icon(id, png));
                    }
                });
            }
            self.check_requirements(p);
        }
        if !catalog.programs.iter().any(|p| p.id == self.selected) {
            self.selected = catalog
                .programs
                .first()
                .map(|p| p.id.clone())
                .unwrap_or_default();
        }
        self.catalog = Some(catalog);
    }

    fn check_requirements(&self, p: &Program) {
        let p = p.clone();
        self.spawn(move |tx| {
            let met = p
                .requirements
                .iter()
                .filter(|r| r.applies())
                .map(install::check)
                .collect();
            let _ = tx.send(Msg::Requirements(p.id, met));
        });
    }

    fn poll(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Catalog(Ok(c)) => self.on_catalog(c),
                Msg::Catalog(Err(e)) => self.catalog_error = Some(e),
                Msg::Release(id, r) => {
                    self.releases.insert(id, r);
                }
                Msg::Icon(id, png) => {
                    if let Some(t) = texture(&self.ctx, &id, &png) {
                        self.icons.insert(id, t);
                    }
                }
                Msg::Requirements(id, met) => {
                    self.requirements.insert(id, met);
                }
                Msg::Progress(id, stage, fraction) => {
                    self.busy = Some(Busy {
                        id,
                        stage,
                        fraction,
                    });
                }
                Msg::Done(id, result) => self.on_done(id, result),
                Msg::SelfUpdate(r) => self.self_update = Some(r),
            }
        }
    }

    fn on_done(&mut self, id: String, result: Result<Done, String>) {
        self.busy = None;
        let name = self.program(&id).map_or(id.clone(), |p| p.name.clone());
        self.status = Some(match result {
            Ok(Done::Installed(version, path)) => {
                self.settings.installed.insert(
                    id.clone(),
                    Installed {
                        version: version.clone(),
                        path,
                    },
                );
                (format!("{name} {version} is installed"), false)
            }
            Ok(Done::Removed) => {
                self.settings.installed.remove(&id);
                (format!("{name} was uninstalled"), false)
            }
            Ok(Done::Requirement) => ("Requirement installed".into(), false),
            Err(e) => (e, true),
        });
        if let Err(e) = self.settings.save() {
            self.status = Some((format!("Could not save settings: {e:#}"), true));
        }
        if let Some(p) = self.program(&id).cloned() {
            self.check_requirements(&p);
        }
    }

    fn program(&self, id: &str) -> Option<&Program> {
        self.catalog.as_ref()?.programs.iter().find(|p| p.id == id)
    }

    fn installed(&self, id: &str) -> Option<Installed> {
        self.settings
            .installed
            .get(id)
            .filter(|i| i.path.exists())
            .cloned()
    }

    fn state(&self, p: &Program) -> State {
        let latest = self.releases.get(&p.id).and_then(|r| r.as_ref().ok());
        match (self.installed(&p.id), latest) {
            (Some(i), Some(r)) if github::newer(r.version(), &i.version) => {
                State::UpdateAvailable(r.version().into())
            }
            (Some(i), _) => State::Installed(i.version),
            (None, _) => State::NotInstalled,
        }
    }

    // -------------------------------------------------------------- actions

    fn start_install(&mut self, p: &Program, release: &Release) {
        let (p, release, dir) = (p.clone(), release.clone(), self.settings.install_dir());
        let required: Vec<_> = p
            .requirements
            .iter()
            .filter(|r| r.applies() && !r.optional && r.install.run.is_some())
            .cloned()
            .collect();
        self.busy = Some(Busy {
            id: p.id.clone(),
            stage: "Starting".into(),
            fraction: 0.0,
        });
        self.spawn(move |tx| {
            let id = p.id.clone();
            let progress = |stage: &str, f: f32| {
                let _ = tx.send(Msg::Progress(id.clone(), stage.into(), f));
            };
            // The program's runtime requirements first (each may ask for admin rights).
            for req in required.iter().filter(|r| !install::check(r)) {
                progress(&format!("Installing {}", req.name), 0.0);
                if let Err(e) = install::install_requirement(req) {
                    let _ = tx.send(Msg::Done(id.clone(), Err(format!("{e:#}"))));
                    return;
                }
            }
            let mut report = |stage: &str, f: f32| progress(stage, f);
            let result = install::install(&p, &release, &dir, &mut report)
                .map(|path| Done::Installed(release.version().into(), path))
                .map_err(|e| format!("{e:#}"));
            let _ = tx.send(Msg::Done(id, result));
        });
    }

    fn start_uninstall(&mut self, p: &Program, folder: PathBuf) {
        let p = p.clone();
        self.busy = Some(Busy {
            id: p.id.clone(),
            stage: "Uninstalling".into(),
            fraction: 1.0,
        });
        self.spawn(move |tx| {
            let r = install::uninstall(&p, &folder)
                .map(|_| Done::Removed)
                .map_err(|e| format!("{e:#}"));
            let _ = tx.send(Msg::Done(p.id, r));
        });
    }

    fn start_requirement(&mut self, p: &Program, index: usize) {
        let Some(req) = p
            .requirements
            .iter()
            .filter(|r| r.applies())
            .nth(index)
            .cloned()
        else {
            return;
        };
        if let Some(url) = &req.install.open {
            self.ctx.open_url(egui::OpenUrl::new_tab(url));
            return;
        }
        self.busy = Some(Busy {
            id: p.id.clone(),
            stage: format!("Installing {}", req.name),
            fraction: 0.0,
        });
        let id = p.id.clone();
        self.spawn(move |tx| {
            let r = install::install_requirement(&req)
                .map(|_| Done::Requirement)
                .map_err(|e| format!("{e:#}"));
            let _ = tx.send(Msg::Done(id, r));
        });
    }

    // -------------------------------------------------------------- library

    fn library_ui(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.label(theme::header("LIBRARY"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(6.0);
                if theme::icon_button(ui, ic::ICON_SETTINGS, "Settings").clicked() {
                    self.settings_open = true;
                }
                let refresh = ui.add_enabled_ui(self.busy.is_none(), |ui| {
                    theme::icon_button(ui, ic::ICON_REFRESH, "Check for updates")
                });
                if refresh.inner.clicked() {
                    self.refresh();
                }
            });
        });
        ui.add_space(6.0);
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(10, 0))
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text(theme::label(ic::ICON_SEARCH, "Search"))
                        .desired_width(f32::INFINITY),
                );
            });
        ui.add_space(8.0);

        let Some(catalog) = &self.catalog else {
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(14, 6))
                .show(ui, |ui| match &self.catalog_error {
                    Some(e) => {
                        ui.label(RichText::new("Couldn't load the program list.").color(WARN));
                        ui.label(RichText::new(e).small().color(WEAK));
                    }
                    None => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(RichText::new("Loading programs…").color(WEAK));
                        });
                    }
                });
            return;
        };
        let needle = self.search.to_lowercase();
        let mut select = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for p in catalog
                    .programs
                    .iter()
                    .filter(|p| needle.is_empty() || p.name.to_lowercase().contains(&needle))
                {
                    if self.library_row(ui, p) {
                        select = Some(p.id.clone());
                    }
                }
            });
        if let Some(id) = select {
            self.selected = id;
        }
    }

    /// One library entry; true when clicked.
    fn library_row(&self, ui: &mut egui::Ui, p: &Program) -> bool {
        let (rect, resp) =
            ui.allocate_exact_size(vec2(ui.available_width(), 56.0), egui::Sense::click());
        let painter = ui.painter_at(rect);
        let selected = p.id == self.selected;
        if selected {
            painter.rect_filled(rect, 0, theme::ROW_SELECTED);
            painter.rect_filled(
                egui::Rect::from_min_size(rect.min, vec2(3.0, rect.height())),
                0,
                ACCENT,
            );
        } else if resp.hovered() {
            painter.rect_filled(rect, 0, theme::ROW_HOVER);
        }
        let icon_rect = egui::Rect::from_min_size(rect.min + vec2(14.0, 10.0), vec2(36.0, 36.0));
        match self.icons.get(&p.id) {
            Some(icon) => {
                egui::Image::new(icon)
                    .corner_radius(6)
                    .paint_at(ui, icon_rect);
            }
            None => {
                painter.rect_filled(icon_rect, 6, theme::ROW_HOVER);
            }
        }
        painter.text(
            rect.min + vec2(60.0, 12.0),
            egui::Align2::LEFT_TOP,
            &p.name,
            egui::FontId::proportional(14.0),
            if selected { STRONG } else { TEXT },
        );
        let (state, color) = self.state(p).text();
        painter.circle_filled(rect.min + vec2(64.0, 37.5), 3.0, color);
        painter.text(
            rect.min + vec2(72.0, 31.0),
            egui::Align2::LEFT_TOP,
            state,
            egui::FontId::proportional(11.0),
            WEAK,
        );
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
    }

    // -------------------------------------------------------------- details

    fn details_ui(&mut self, ui: &mut egui::Ui) {
        let Some(p) = self.program(&self.selected).cloned() else {
            ui.centered_and_justified(|ui| {
                if self.catalog.is_some() {
                    ui.label(RichText::new("No programs yet.").color(WEAK));
                } else {
                    ui.spinner();
                }
            });
            return;
        };
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(28, 24))
                    .show(ui, |ui| {
                        ui.set_max_width(ui.available_width().min(860.0));
                        self.hero_ui(ui, &p);
                        ui.add_space(14.0);
                        if !p.description.is_empty() {
                            theme::card(ui, |ui| {
                                ui.label(theme::header("ABOUT"));
                                ui.add_space(4.0);
                                ui.label(&p.description);
                            });
                            ui.add_space(14.0);
                        }
                        self.requirements_ui(ui, &p);
                        self.notes_ui(ui, &p);
                    });
            });
    }

    /// Icon, name, version and the actions.
    fn hero_ui(&mut self, ui: &mut egui::Ui, p: &Program) {
        let release = self.releases.get(&p.id).cloned();
        let latest = release.as_ref().and_then(|r| r.as_ref().ok()).cloned();
        let installed = self.installed(&p.id);
        let state = self.state(p);
        let busy_here = self.busy.as_ref().is_some_and(|b| b.id == p.id);
        let idle = self.busy.is_none();
        theme::card(ui, |ui| {
            ui.horizontal(|ui| {
                match self.icons.get(&p.id) {
                    Some(icon) => {
                        ui.add(
                            egui::Image::new(icon)
                                .corner_radius(12)
                                .fit_to_exact_size(vec2(84.0, 84.0)),
                        );
                    }
                    None => {
                        ui.add_space(84.0);
                    }
                }
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.add_space(6.0);
                    ui.label(RichText::new(&p.name).size(24.0).strong().color(STRONG));
                    ui.add_space(2.0);
                    match &release {
                        Some(Ok(r)) => {
                            let mut line = format!("Version {}", r.version());
                            if !r.date.is_empty() {
                                line += &format!("  ·  Released {}", r.date);
                            }
                            ui.label(RichText::new(line).color(WEAK));
                        }
                        Some(Err(e)) => {
                            ui.label(RichText::new(e).color(WARN));
                        }
                        None => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(
                                    RichText::new("Checking for the latest version…").color(WEAK),
                                );
                            });
                        }
                    }
                    ui.add_space(4.0);
                    let (text, color) = state.text();
                    status_chip(ui, &text, color);
                });

                // Actions, right-aligned.
                ui.with_layout(
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| match (&installed, &latest) {
                        (Some(i), _) => {
                            if theme::icon_button(ui, ic::ICON_DELETE, "Uninstall").clicked()
                                && idle
                            {
                                self.confirm_uninstall = true;
                            }
                            if theme::icon_button(
                                ui,
                                ic::ICON_FOLDER_OPEN,
                                "Open the install folder",
                            )
                            .clicked()
                            {
                                open_folder(&i.path);
                            }
                            ui.add_space(4.0);
                            if let State::UpdateAvailable(v) = &state
                                && let Some(r) = &latest
                                && theme::secondary(
                                    ui,
                                    idle && r.asset.is_some(),
                                    ic::ICON_SYSTEM_UPDATE_ALT,
                                    &format!("Update to {v}"),
                                )
                                .clicked()
                            {
                                self.start_install(p, r);
                            }
                            if theme::primary(ui, idle, ic::ICON_PLAY_ARROW, "Launch").clicked()
                                && let Err(e) = install::launch(p, &i.path)
                            {
                                self.status = Some((format!("{e:#}"), true));
                            }
                        }
                        (None, Some(r)) => {
                            let resp = theme::primary(
                                ui,
                                idle && r.asset.is_some(),
                                ic::ICON_DOWNLOAD,
                                "Install",
                            );
                            if r.asset.is_none() {
                                resp.on_disabled_hover_text(format!(
                                    "Release {} has no download for {}",
                                    r.tag,
                                    crate::catalog::platform()
                                ));
                            } else if resp.clicked() {
                                self.start_install(p, r);
                            }
                        }
                        (None, None) => {
                            theme::primary(ui, false, ic::ICON_DOWNLOAD, "Install");
                        }
                    },
                );
            });

            if busy_here && let Some(b) = &self.busy {
                ui.add_space(12.0);
                ui.add(
                    egui::ProgressBar::new(b.fraction)
                        .text(RichText::new(&b.stage).small())
                        .desired_height(18.0)
                        .fill(ACCENT)
                        .animate(true),
                );
            }

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(2.0);
            let location = match &installed {
                Some(i) => format!("Installed in {}", i.path.display()),
                None => format!(
                    "Installs to {}",
                    self.settings.install_dir().join(&p.name).display()
                ),
            };
            ui.label(RichText::new(location).small().color(WEAK));
        });
    }

    fn requirements_ui(&mut self, ui: &mut egui::Ui, p: &Program) {
        let reqs: Vec<_> = p.requirements.iter().filter(|r| r.applies()).collect();
        if reqs.is_empty() {
            return;
        }
        let met = self.requirements.get(&p.id).cloned();
        let idle = self.busy.is_none();
        let mut install_req = None;
        theme::card(ui, |ui| {
            ui.label(theme::header("REQUIREMENTS"));
            ui.add_space(6.0);
            for (i, r) in reqs.iter().enumerate() {
                let ok = met.as_ref().and_then(|m| m.get(i)).copied();
                ui.horizontal(|ui| {
                    match ok {
                        Some(true) => {
                            ui.label(ic::ICON_CHECK_CIRCLE.rich_text().size(18.0).color(OK))
                        }
                        Some(false) if r.optional => ui.label(
                            ic::ICON_RADIO_BUTTON_UNCHECKED
                                .rich_text()
                                .size(18.0)
                                .color(WEAK),
                        ),
                        Some(false) => ui.label(ic::ICON_ERROR.rich_text().size(18.0).color(WARN)),
                        None => ui.spinner(),
                    };
                    ui.label(RichText::new(&r.name).color(TEXT));
                    if r.optional {
                        ui.label(RichText::new("Optional").small().color(WEAK));
                    }
                    if ok == Some(false) {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if r.install.run.is_some() {
                                if ui
                                    .add_enabled(
                                        idle,
                                        egui::Button::new(theme::label(
                                            ic::ICON_DOWNLOAD,
                                            "Install",
                                        )),
                                    )
                                    .clicked()
                                {
                                    install_req = Some(i);
                                }
                            } else if r.install.open.is_some()
                                && ui
                                    .button(theme::label(ic::ICON_OPEN_IN_NEW, "Get it"))
                                    .clicked()
                            {
                                install_req = Some(i);
                            }
                        });
                    }
                });
                if !r.note.is_empty() {
                    ui.horizontal(|ui| {
                        ui.add_space(26.0);
                        ui.label(RichText::new(&r.note).small().color(WEAK));
                    });
                }
                if i + 1 < reqs.len() {
                    ui.add_space(4.0);
                }
            }
            if self.installed(&p.id).is_none()
                && reqs.iter().any(|r| !r.optional && r.install.run.is_some())
            {
                ui.add_space(6.0);
                ui.label(
                    RichText::new("Missing required items are set up when you install.")
                        .small()
                        .color(WEAK),
                );
            }
        });
        ui.add_space(14.0);
        if let Some(i) = install_req {
            self.start_requirement(p, i);
        }
    }

    fn notes_ui(&mut self, ui: &mut egui::Ui, p: &Program) {
        let Some(Ok(r)) = self.releases.get(&p.id).cloned() else {
            return;
        };
        theme::card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(theme::header(&format!("WHAT'S NEW IN {}", r.version())));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(theme::label(ic::ICON_OPEN_IN_NEW, "Release page"))
                                    .small(),
                            )
                            .frame(false),
                        )
                        .clicked()
                    {
                        self.ctx.open_url(egui::OpenUrl::new_tab(&r.page));
                    }
                });
            });
            ui.add_space(6.0);
            release_notes(ui, &r.notes);
        });
    }

    // -------------------------------------------------------------- dialogs

    fn uninstall_dialog(&mut self, ctx: &egui::Context) {
        if !self.confirm_uninstall {
            return;
        }
        let Some(p) = self.program(&self.selected).cloned() else {
            self.confirm_uninstall = false;
            return;
        };
        let Some(i) = self.installed(&p.id) else {
            self.confirm_uninstall = false;
            return;
        };
        let mut choice = None;
        let modal = egui::Modal::new(egui::Id::unique("uninstall"))
            .backdrop_color(Color32::from_black_alpha(150))
            .frame(dialog_frame(ctx))
            .show(ctx, |ui| {
                ui.set_width(420.0);
                ui.label(
                    RichText::new(format!("Uninstall {}?", p.name))
                        .size(17.0)
                        .strong()
                        .color(STRONG),
                );
                ui.add_space(8.0);
                ui.label(format!("This deletes {}.", i.path.display()));
                ui.label(
                    RichText::new("Files you made with it, like projects, are not touched.")
                        .color(WEAK),
                );
                ui.add_space(14.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let delete = egui::Button::new(
                        RichText::new(theme::label(ic::ICON_DELETE, "Uninstall"))
                            .color(Color32::WHITE),
                    )
                    .fill(ERROR.gamma_multiply(0.8))
                    .min_size(vec2(110.0, 30.0));
                    if ui.add(delete).clicked() {
                        choice = Some(true);
                    }
                    if ui
                        .add(egui::Button::new("Cancel").min_size(vec2(80.0, 30.0)))
                        .clicked()
                    {
                        choice = Some(false);
                    }
                });
            });
        if choice.is_none() && modal.should_close() {
            choice = Some(false);
        }
        if let Some(yes) = choice {
            self.confirm_uninstall = false;
            if yes {
                self.start_uninstall(&p, i.path);
            }
        }
    }

    fn settings_dialog(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let mut changed = false;
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::unique("settings"))
            .backdrop_color(Color32::from_black_alpha(150))
            .frame(dialog_frame(ctx))
            .show(ctx, |ui| {
                ui.set_width(500.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Settings").size(17.0).strong().color(STRONG));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::icon_button(ui, ic::ICON_CLOSE, "Close").clicked() {
                            close = true;
                        }
                    });
                });
                ui.add_space(12.0);
                ui.label(theme::header("INSTALL LOCATION"));
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let mut shown = self.settings.install_dir().display().to_string();
                    ui.add(
                        egui::TextEdit::singleline(&mut shown)
                            .interactive(false)
                            .desired_width(ui.available_width() - 150.0),
                    );
                    if ui.button(theme::label(ic::ICON_FOLDER_OPEN, "Browse…")).clicked()
                        && let Some(dir) = rfd::FileDialog::new()
                            .set_directory(self.settings.install_dir())
                            .pick_folder()
                    {
                        self.settings.install_dir = Some(dir);
                        changed = true;
                    }
                    if ui
                        .add_enabled(self.settings.install_dir.is_some(), egui::Button::new("Reset"))
                        .on_hover_text("Use the default location")
                        .clicked()
                    {
                        self.settings.install_dir = None;
                        changed = true;
                    }
                });
                ui.label(
                    RichText::new(
                        "New installs go here. Installed programs stay where they are until reinstalled.",
                    )
                    .small()
                    .color(WEAK),
                );
                ui.add_space(14.0);
                ui.label(theme::header("UPDATES"));
                ui.add_space(4.0);
                changed |= ui
                    .checkbox(
                        &mut self.settings.check_self_update,
                        "Check for a new ReLauncher at startup",
                    )
                    .changed();
                ui.add_space(14.0);
                ui.separator();
                ui.label(
                    RichText::new(format!(
                        "ReLauncher {}  ·  {}",
                        env!("CARGO_PKG_VERSION"),
                        crate::catalog::platform()
                    ))
                    .small()
                    .color(WEAK),
                );
            });
        if changed && let Err(e) = self.settings.save() {
            self.status = Some((format!("Could not save settings: {e:#}"), true));
        }
        if close || modal.should_close() {
            self.settings_open = false;
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll();
        let ctx = ui.ctx().clone();

        if let Some(r) = self.self_update.clone() {
            egui::Panel::top("self_update")
                .frame(
                    egui::Frame::new()
                        .fill(theme::ROW_SELECTED)
                        .inner_margin(egui::Margin::symmetric(14, 6)),
                )
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(ic::ICON_NEW_RELEASES.rich_text().color(ACCENT));
                        ui.label(format!(
                            "ReLauncher {} is available (you have {}).",
                            r.version(),
                            env!("CARGO_PKG_VERSION")
                        ));
                        if ui.link("Download").clicked() {
                            ctx.open_url(egui::OpenUrl::new_tab(&r.page));
                        }
                    });
                });
        }
        egui::Panel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(theme::HEADER)
                    .inner_margin(egui::Margin::symmetric(12, 4)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (text, error) = self
                        .status
                        .clone()
                        .unwrap_or_else(|| ("Ready".into(), false));
                    ui.label(
                        RichText::new(text)
                            .small()
                            .color(if error { ERROR } else { WEAK }),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!(
                                "ReLauncher {}  |  {}",
                                env!("CARGO_PKG_VERSION"),
                                crate::catalog::platform()
                            ))
                            .small()
                            .color(WEAK),
                        );
                    });
                });
            });
        egui::Panel::left("library")
            .resizable(false)
            .exact_size(270.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::STRIP)
                    .stroke(egui::Stroke::new(1.0, GAP)),
            )
            .show(ui, |ui| self.library_ui(ui));
        egui::CentralPanel::no_frame()
            .frame(egui::Frame::new().fill(theme::PANEL))
            .show(ui, |ui| self.details_ui(ui));

        self.uninstall_dialog(&ctx);
        self.settings_dialog(&ctx);
        if self.busy.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

/// A small rounded label with a colored dot: "● Installed · 0.1.0".
fn status_chip(ui: &mut egui::Ui, text: &str, color: Color32) {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.15))
        .corner_radius(10)
        .inner_margin(egui::Margin::symmetric(8, 2))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                let (r, _) = ui.allocate_exact_size(vec2(7.0, 7.0), egui::Sense::hover());
                ui.painter().circle_filled(r.center(), 3.5, color);
                ui.label(RichText::new(text).small().color(color));
            });
        });
}

/// Release notes with light Markdown: headings, bullets, and plain lines.
fn release_notes(ui: &mut egui::Ui, notes: &str) {
    let notes = notes.trim();
    if notes.is_empty() {
        ui.label(RichText::new("No release notes.").color(WEAK));
        return;
    }
    for line in notes.lines() {
        let line = line.trim_end();
        if let Some(h) = line.strip_prefix('#') {
            ui.add_space(4.0);
            ui.label(
                RichText::new(h.trim_start_matches('#').trim())
                    .strong()
                    .color(STRONG),
            );
        } else if let Some(item) = line
            .trim_start()
            .strip_prefix("- ")
            .or_else(|| line.trim_start().strip_prefix("* "))
        {
            ui.horizontal_wrapped(|ui| {
                ui.add_space(4.0);
                ui.label(RichText::new("•").color(WEAK));
                ui.label(item.replace("**", ""));
            });
        } else if line.is_empty() {
            ui.add_space(4.0);
        } else {
            ui.label(line.replace("**", ""));
        }
    }
}

fn dialog_frame(ctx: &egui::Context) -> egui::Frame {
    egui::Frame::window(&ctx.global_style())
        .fill(theme::PANEL)
        .inner_margin(egui::Margin::same(20))
}

fn open_folder(path: &std::path::Path) {
    let program = match crate::catalog::os() {
        "windows" => "explorer",
        "macos" => "open",
        _ => "xdg-open",
    };
    let _ = std::process::Command::new(program).arg(path).spawn();
}

fn texture(ctx: &egui::Context, name: &str, png: &[u8]) -> Option<egui::TextureHandle> {
    let img = image::load_from_memory(png).ok()?.into_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    Some(ctx.load_texture(
        name,
        egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()),
        egui::TextureOptions::LINEAR,
    ))
}

fn icon_cache(id: &str) -> PathBuf {
    crate::catalog::config_dir()
        .join("icons")
        .join(format!("{id}.png"))
}
