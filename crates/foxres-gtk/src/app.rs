//! Main window.

use std::{cell::RefCell, path::PathBuf, rc::Rc, time::Duration};

use foxres_core::{Color, ESeries, bands::BandCount};
use relm4::{
    ComponentParts, ComponentSender, SimpleComponent,
    adw::{self, prelude::*},
    gtk::{self, glib, glib::prelude::ToValue},
};

use crate::{draw, screenshot, text, tools};

/// Sidebar entries: page id, title and section, in display order.
pub const NAV: &[(&str, &str, &str)] = &[
    ("bands", "Color Code", "Identify"),
    ("smd", "SMD Codes", "Identify"),
    ("eseries", "Standard Values", "Choose"),
    ("combine", "Combinations", "Choose"),
    ("divider", "Voltage Divider", "Choose"),
    ("led", "LED Resistor", "Choose"),
    ("ohm", "Ohm's Law", "Analyze"),
    ("power", "Power & Package", "Analyze"),
    ("network", "Series & Parallel", "Analyze"),
    ("tolerance", "Tolerance", "Analyze"),
    ("thermal", "Temperature & Noise", "Analyze"),
    ("rtd", "RTD (Pt100/Pt1000)", "Instrumentation"),
    ("ntc", "NTC Thermistor", "Instrumentation"),
    ("loop", "4–20 mA Loop", "Instrumentation"),
    ("bridge", "Wheatstone Bridge", "Instrumentation"),
    ("wire", "Wire", "Wiring"),
    ("trace", "PCB Trace", "Wiring"),
];

const FOXIQ_URL: &str = "https://www.foxiq.io";

/// Text entries the screenshot mode fills in.
struct Entries {
    value: adw::EntryRow,
    smd: adw::EntryRow,
    marking: adw::EntryRow,
    nearest: adw::EntryRow,
    combo: adw::EntryRow,
}

impl Entries {
    fn new() -> Self {
        Self {
            value: adw::EntryRow::new(),
            smd: adw::EntryRow::new(),
            marking: adw::EntryRow::new(),
            nearest: adw::EntryRow::new(),
            combo: adw::EntryRow::new(),
        }
    }
}

pub struct App {
    window: adw::ApplicationWindow,
    split: adw::NavigationSplitView,
    sidebar: gtk::ListBox,
    page_title: String,
    triple: String,
    stack: adw::ViewStack,
    entries: Entries,
    drawing: gtk::DrawingArea,
    bands_group: adw::PreferencesGroup,
    band_rows: Vec<adw::ComboRow>,
    band_swatches: Vec<gtk::DrawingArea>,
    count: BandCount,
    colors: Rc<RefCell<Vec<Color>>>,
    screenshots: Option<PathBuf>,

    band_value: String,
    band_detail: String,
    value_hint: String,

    smd_value: String,
    smd_notation: String,
    smd_codes: [String; 3],
    smd_codes_error: String,

    series: ESeries,
    series_text: String,
    nearest: text::Nearest,

    combo_series: ESeries,
    combo_text: String,
    series_pair: String,
    parallel_pair: String,
}

#[derive(Debug)]
pub enum Msg {
    BandCount(u32),
    Band(usize, u32),
    ValueText(String),
    SmdCode(String),
    SmdValue(String),
    NearestText(String),
    NearestSeries(u32),
    ComboText(String),
    ComboSeries(u32),
    About,
    Navigate(i32),
    OpenWebsite,
    Shot(usize),
    Capture(usize),
}

const DEFAULT_SERIES: u32 = 3; // E24

#[relm4::component(pub)]
impl SimpleComponent for App {
    type Init = ();
    type Input = Msg;
    type Output = ();

    view! {
        #[root]
        adw::ApplicationWindow {
            set_title: Some("FoxRes"),
            set_default_size: (980, 760),

            #[local_ref]
            split -> adw::NavigationSplitView {
                set_min_sidebar_width: 220.0,
                set_max_sidebar_width: 280.0,

                #[wrap(Some)]
                set_sidebar = &adw::NavigationPage {
                    set_title: "FoxRes",
                    #[wrap(Some)]
                    set_child = &adw::ToolbarView {
                        add_top_bar = &adw::HeaderBar {},
                        #[wrap(Some)]
                        set_content = &gtk::ScrolledWindow {
                            set_hscrollbar_policy: gtk::PolicyType::Never,
                            #[local_ref]
                            sidebar -> gtk::ListBox {
                                add_css_class: "navigation-sidebar",
                                connect_row_selected[sender] => move |_, row| {
                                    if let Some(row) = row {
                                        sender.input(Msg::Navigate(row.index()));
                                    }
                                },
                            },
                        },
                        add_bottom_bar = &gtk::Button {
                            add_css_class: "flat",
                            set_margin_all: 6,
                            set_tooltip_text: Some("Open foxiq.io"),
                            #[wrap(Some)]
                            set_child = &gtk::Label {
                                set_markup: "<small>Made by <b>FoxIQ</b> · engineering services</small>",
                            },
                            connect_clicked => Msg::OpenWebsite,
                        },
                    },
                },

                #[wrap(Some)]
                set_content = &adw::NavigationPage {
                    #[watch]
                    set_title: &model.page_title,
                    #[wrap(Some)]
                    set_child = &adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {
                    pack_end = &gtk::Button {
                        set_icon_name: "help-about-symbolic",
                        set_tooltip_text: Some("About FoxRes"),
                        connect_clicked => Msg::About,
                    },
                },

                #[wrap(Some)]
                #[local_ref]
                set_content = stack -> adw::ViewStack {
                    add_titled_with_icon[Some("bands"), "Bands", "view-continuous-symbolic"] = &adw::PreferencesPage {
                        add = &adw::PreferencesGroup {
                            #[local_ref]
                            add = drawing -> gtk::DrawingArea {
                                set_content_height: 130,
                                set_hexpand: true,
                                set_accessible_role: gtk::AccessibleRole::Img,
                            },
                            add = &gtk::Label {
                                add_css_class: "title-1",
                                #[watch]
                                set_label: &model.band_value,
                            },
                            add = &gtk::Label {
                                add_css_class: "dim-label",
                                set_wrap: true,
                                set_justify: gtk::Justification::Center,
                                #[watch]
                                set_label: &model.band_detail,
                            },
                        },
                        add = &adw::PreferencesGroup {
                            add = &adw::ComboRow {
                                set_title: "Number of bands",
                                set_model: Some(&gtk::StringList::new(&["3 bands", "4 bands", "5 bands", "6 bands"])),
                                set_selected: 1,
                                connect_selected_notify[sender] => move |row| {
                                    sender.input(Msg::BandCount(row.selected()));
                                },
                            },
                            #[local_ref]
                            add = value_entry -> adw::EntryRow {
                                set_title: "Or type a value (4k7, 220, 1M)",
                                connect_changed[sender] => move |e| {
                                    sender.input(Msg::ValueText(e.text().to_string()));
                                },
                            },
                            add = &gtk::Label {
                                add_css_class: "caption",
                                add_css_class: "error",
                                set_margin_top: 6,
                                set_wrap: true,
                                #[watch]
                                set_visible: !model.value_hint.is_empty(),
                                #[watch]
                                set_label: &model.value_hint,
                            },
                        },
                        #[local_ref]
                        add = bands_group -> adw::PreferencesGroup {
                            set_title: "Bands",
                            set_description: Some("Read from the end farthest from the gap"),
                        },
                    },

                    add_titled_with_icon[Some("smd"), "SMD", "view-grid-symbolic"] = &adw::PreferencesPage {
                        add = &adw::PreferencesGroup {
                            set_title: "Decode a marking",
                            #[local_ref]
                            add = smd_entry -> adw::EntryRow {
                                set_title: "Code",
                                connect_changed[sender] => move |e| {
                                    sender.input(Msg::SmdCode(e.text().to_string()));
                                },
                            },
                            add = &gtk::Label {
                                add_css_class: "title-1",
                                set_margin_top: 12,
                                #[watch]
                                set_label: &model.smd_value,
                            },
                            add = &gtk::Label {
                                add_css_class: "dim-label",
                                #[watch]
                                set_label: &model.smd_notation,
                            },
                        },
                        add = &adw::PreferencesGroup {
                            set_title: "Find the marking for a value",
                            #[local_ref]
                            add = marking_entry -> adw::EntryRow {
                                set_title: "Resistance",
                                connect_changed[sender] => move |e| {
                                    sender.input(Msg::SmdValue(e.text().to_string()));
                                },
                            },
                            add = &gtk::Label {
                                add_css_class: "caption",
                                add_css_class: "error",
                                set_margin_top: 6,
                                #[watch]
                                set_visible: !model.smd_codes_error.is_empty(),
                                #[watch]
                                set_label: &model.smd_codes_error,
                            },
                        },
                        add = &adw::PreferencesGroup {
                            #[watch]
                            set_visible: !model.smd_codes[0].is_empty(),
                            add = &adw::ActionRow {
                                set_title: "3-digit (±5 %)",
                                add_css_class: "property",
                                set_subtitle_selectable: true,
                                #[watch]
                                set_subtitle: &model.smd_codes[0],
                            },
                            add = &adw::ActionRow {
                                set_title: "4-digit (±1 %)",
                                add_css_class: "property",
                                set_subtitle_selectable: true,
                                #[watch]
                                set_subtitle: &model.smd_codes[1],
                            },
                            add = &adw::ActionRow {
                                set_title: "EIA-96 (±1 %)",
                                add_css_class: "property",
                                set_subtitle_selectable: true,
                                #[watch]
                                set_subtitle: &model.smd_codes[2],
                            },
                        },
                    },

                    add_titled_with_icon[Some("eseries"), "E-Series", "view-list-symbolic"] = &adw::PreferencesPage {
                        add = &adw::PreferencesGroup {
                            set_title: "Nearest standard value",
                            #[local_ref]
                            add = nearest_entry -> adw::EntryRow {
                                set_title: "Target resistance",
                                connect_changed[sender] => move |e| {
                                    sender.input(Msg::NearestText(e.text().to_string()));
                                },
                            },
                            add = &adw::ComboRow {
                                set_title: "Series",
                                set_model: Some(&gtk::StringList::new(&series_names)),
                                set_selected: DEFAULT_SERIES,
                                connect_selected_notify[sender] => move |row| {
                                    sender.input(Msg::NearestSeries(row.selected()));
                                },
                            },
                            add = &gtk::Label {
                                add_css_class: "title-1",
                                set_margin_top: 12,
                                #[watch]
                                set_visible: !model.nearest.value.is_empty(),
                                #[watch]
                                set_label: &model.nearest.value,
                            },
                            add = &gtk::Label {
                                add_css_class: "dim-label",
                                set_margin_top: 6,
                                set_wrap: true,
                                #[watch]
                                set_label: &model.nearest.detail,
                            },
                        },
                        add = &adw::PreferencesGroup {
                            #[watch]
                            set_visible: !model.nearest.below.is_empty(),
                            add = &adw::ActionRow {
                                set_title: "Next value below",
                                add_css_class: "property",
                                set_subtitle_selectable: true,
                                #[watch]
                                set_subtitle: &model.nearest.below,
                            },
                            add = &adw::ActionRow {
                                set_title: "Next value above",
                                add_css_class: "property",
                                set_subtitle_selectable: true,
                                #[watch]
                                set_subtitle: &model.nearest.above,
                            },
                        },
                    },

                    add_titled_with_icon[Some("combine"), "Combine", "list-add-symbolic"] = &adw::PreferencesPage {
                        add = &adw::PreferencesGroup {
                            set_title: "Two-resistor combinations",
                            set_description: Some("Best pair of standard values for a value you don't stock"),
                            #[local_ref]
                            add = combo_entry -> adw::EntryRow {
                                set_title: "Target resistance",
                                connect_changed[sender] => move |e| {
                                    sender.input(Msg::ComboText(e.text().to_string()));
                                },
                            },
                            add = &adw::ComboRow {
                                set_title: "Series",
                                set_model: Some(&gtk::StringList::new(&series_names)),
                                set_selected: DEFAULT_SERIES,
                                connect_selected_notify[sender] => move |row| {
                                    sender.input(Msg::ComboSeries(row.selected()));
                                },
                            },
                        },
                        add = &adw::PreferencesGroup {
                            add = &adw::ActionRow {
                                set_title: "Series",
                                add_css_class: "property",
                                set_subtitle_selectable: true,
                                #[watch]
                                set_subtitle: &model.series_pair,
                            },
                            add = &adw::ActionRow {
                                set_title: "Parallel",
                                add_css_class: "property",
                                set_subtitle_selectable: true,
                                #[watch]
                                set_subtitle: &model.parallel_pair,
                            },
                            add = &adw::ActionRow {
                                set_title: "Three resistors",
                                add_css_class: "property",
                                set_subtitle_selectable: true,
                                #[watch]
                                set_subtitle: &model.triple,
                            },
                        },
                    },
                },
            },
                },
            },
        }
    }

    fn init((): (), root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let series_names: Vec<&str> = ESeries::ALL.iter().map(|s| s.name()).collect();
        let initial = vec![Color::Yellow, Color::Violet, Color::Red, Color::Gold];
        let (band_value, band_detail) = text::bands(&initial);
        let series = ESeries::ALL[DEFAULT_SERIES as usize];
        let nearest = text::nearest("", series);
        let (_, smd_notation) = text::smd_code("");

        let mut model = App {
            window: root.clone(),
            split: adw::NavigationSplitView::new(),
            sidebar: gtk::ListBox::new(),
            page_title: NAV[0].1.to_string(),
            triple: String::new(),
            stack: adw::ViewStack::new(),
            entries: Entries::new(),
            drawing: gtk::DrawingArea::new(),
            bands_group: adw::PreferencesGroup::new(),
            band_rows: Vec::new(),
            band_swatches: Vec::new(),
            count: BandCount::Four,
            colors: Rc::new(RefCell::new(initial)),
            screenshots: screenshot::dir(),
            band_value,
            band_detail,
            value_hint: String::new(),
            smd_value: String::new(),
            smd_notation,
            smd_codes: Default::default(),
            smd_codes_error: String::new(),
            series,
            series_text: String::new(),
            nearest,
            combo_series: series,
            combo_text: String::new(),
            series_pair: String::new(),
            parallel_pair: String::new(),
        };

        let colors = model.colors.clone();
        model.drawing.set_draw_func(move |_, cr, w, h| {
            draw::resistor(cr, w, h, &colors.borrow());
        });

        let stack = &model.stack;
        let split = &model.split;
        let sidebar = &model.sidebar;
        let drawing = &model.drawing;
        let bands_group = &model.bands_group;
        let value_entry = &model.entries.value;
        let smd_entry = &model.entries.smd;
        let marking_entry = &model.entries.marking;
        let nearest_entry = &model.entries.nearest;
        let combo_entry = &model.entries.combo;
        let widgets = view_output!();

        model.rebuild_band_rows(&sender);
        for tool in tools::all() {
            let (id, title) = (tool.id, tool.title);
            model.stack.add_titled(&tools::page(tool), Some(id), title);
        }
        for (id, title, _) in NAV {
            let label = gtk::Label::builder().label(*title).xalign(0.0).build();
            model
                .sidebar
                .append(&gtk::ListBoxRow::builder().child(&label).name(*id).build());
        }
        model.sidebar.set_header_func(|row, before| {
            let section = NAV[row.index() as usize].2;
            if before.map(|b| NAV[b.index() as usize].2) == Some(section) {
                row.set_header(None::<&gtk::Widget>);
            } else {
                let header = gtk::Label::builder()
                    .label(section)
                    .xalign(0.0)
                    .css_classes(["heading", "dim-label"])
                    .margin_top(if before.is_some() { 14 } else { 6 })
                    .margin_bottom(4)
                    .margin_start(10)
                    .build();
                row.set_header(Some(&header));
            }
        });
        let narrow = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
            adw::BreakpointConditionLengthType::MaxWidth,
            720.0,
            adw::LengthUnit::Sp,
        ));
        narrow.add_setter(&model.split, "collapsed", Some(&true.to_value()));
        root.add_breakpoint(narrow);
        model
            .sidebar
            .select_row(model.sidebar.row_at_index(0).as_ref());

        if model.screenshots.is_some() {
            let sender = sender.clone();
            glib::timeout_add_local_once(Duration::from_millis(1200), move || {
                sender.input(Msg::Shot(0));
            });
        }
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Msg, sender: ComponentSender<Self>) {
        match msg {
            Msg::BandCount(i) => {
                let Some(&count) = BandCount::CHOICES.get(i as usize) else {
                    return;
                };
                self.change_band_count(count);
                self.rebuild_band_rows(&sender);
            }
            Msg::Band(pos, idx) => {
                let role = self.count.roles()[pos];
                if let Some(&c) = role.choices().get(idx as usize) {
                    if let Some(slot) = self.colors.borrow_mut().get_mut(pos) {
                        *slot = c;
                    }
                    if let Some(swatch) = self.band_swatches.get(pos) {
                        swatch.queue_draw();
                    }
                    self.value_hint.clear();
                    self.refresh_bands();
                }
            }
            Msg::ValueText(t) => {
                if t.trim().is_empty() {
                    self.value_hint.clear();
                    return;
                }
                let (tol, tc) = self.tolerance_and_tempco();
                match text::value_to_bands(&t, self.count, tol, tc) {
                    Ok(colors) if colors.len() == self.count.len() => {
                        *self.colors.borrow_mut() = colors;
                        self.value_hint.clear();
                        self.rebuild_band_rows(&sender);
                    }
                    Ok(_) => self.value_hint = "0 Ω is a single black band".into(),
                    Err(e) => self.value_hint = e,
                }
            }
            Msg::SmdCode(t) => (self.smd_value, self.smd_notation) = text::smd_code(&t),
            Msg::SmdValue(t) => match text::smd_for_value(&t) {
                Ok(codes) => {
                    self.smd_codes = codes;
                    self.smd_codes_error.clear();
                }
                Err(e) => {
                    self.smd_codes = Default::default();
                    self.smd_codes_error = e;
                }
            },
            Msg::NearestText(t) => {
                self.series_text = t;
                self.refresh_nearest();
            }
            Msg::NearestSeries(i) => {
                self.series = ESeries::ALL.get(i as usize).copied().unwrap_or(self.series);
                self.refresh_nearest();
            }
            Msg::ComboText(t) => {
                self.combo_text = t;
                self.refresh_pairs();
            }
            Msg::ComboSeries(i) => {
                self.combo_series = ESeries::ALL
                    .get(i as usize)
                    .copied()
                    .unwrap_or(self.combo_series);
                self.refresh_pairs();
            }
            Msg::About => self.show_about(),
            Msg::Navigate(i) => {
                if let Some((id, title, _)) = usize::try_from(i).ok().and_then(|i| NAV.get(i)) {
                    self.stack.set_visible_child_name(id);
                    self.page_title = (*title).to_string();
                    self.split.set_show_content(true);
                }
            }
            Msg::OpenWebsite => {
                gtk::UriLauncher::new(FOXIQ_URL).launch(
                    Some(&self.window),
                    gtk::gio::Cancellable::NONE,
                    |_| {},
                );
            }
            Msg::Shot(i) => self.prepare_shot(i, &sender),
            Msg::Capture(i) => {
                if let (Some(dir), Some(shot)) = (&self.screenshots, screenshot::SHOTS.get(i)) {
                    let path = dir.join(shot.file);
                    gtk::prelude::GtkWindowExt::set_focus(&self.window, gtk::Widget::NONE);
                    match screenshot::save(&self.window, &path) {
                        Ok(()) => eprintln!("saved {}", path.display()),
                        Err(e) => eprintln!("could not save {}: {e}", path.display()),
                    }
                }
                sender.input(Msg::Shot(i + 1));
            }
        }
    }
}

impl App {
    fn prepare_shot(&self, i: usize, sender: &ComponentSender<Self>) {
        if i == 0 {
            self.entries.value.set_text("4k7");
            self.entries.smd.set_text("01C");
            self.entries.marking.set_text("4.99k");
            self.entries.nearest.set_text("5k");
            self.entries.combo.set_text("5.7k");
        }
        let Some(shot) = screenshot::SHOTS.get(i) else {
            self.window.close();
            return;
        };
        adw::StyleManager::default().set_color_scheme(if shot.dark {
            adw::ColorScheme::ForceDark
        } else {
            adw::ColorScheme::ForceLight
        });
        if let Some(i) = NAV.iter().position(|(id, _, _)| *id == shot.page) {
            self.sidebar
                .select_row(self.sidebar.row_at_index(i as i32).as_ref());
        }
        let sender = sender.clone();
        gtk::prelude::GtkWindowExt::set_focus(&self.window, gtk::Widget::NONE);
        glib::timeout_add_local_once(Duration::from_millis(700), move || {
            sender.input(Msg::Capture(i));
        });
    }

    fn tolerance_and_tempco(&self) -> (Color, Color) {
        use foxres_core::bands::Role;
        let colors = self.colors.borrow();
        let find = |role: Role| {
            self.count
                .roles()
                .iter()
                .position(|&r| r == role)
                .and_then(|i| colors.get(i).copied())
        };
        let tol = find(Role::Tolerance).unwrap_or(match self.count {
            BandCount::Five | BandCount::Six => Color::Brown,
            _ => Color::Gold,
        });
        (tol, find(Role::Tempco).unwrap_or(Color::Brown))
    }

    fn change_band_count(&mut self, count: BandCount) {
        let current = foxres_core::bands::decode(&self.colors.borrow()).map(|r| r.ohms);
        let (tol, tc) = self.tolerance_and_tempco();
        self.count = count;
        let tol = if count == BandCount::Four && tol.tolerance().unwrap_or(0.0) < 1.0 {
            Color::Gold
        } else {
            tol
        };
        let encoded = current
            .ok()
            .and_then(|ohms| foxres_core::bands::encode(ohms, count, tol, tc).ok())
            .filter(|c| c.len() == count.len());
        let default = match count {
            BandCount::Three => vec![Color::Yellow, Color::Violet, Color::Red],
            BandCount::Four | BandCount::One => {
                vec![Color::Yellow, Color::Violet, Color::Red, Color::Gold]
            }
            BandCount::Five => vec![
                Color::Yellow,
                Color::Violet,
                Color::Black,
                Color::Brown,
                Color::Brown,
            ],
            BandCount::Six => vec![
                Color::Yellow,
                Color::Violet,
                Color::Black,
                Color::Brown,
                Color::Brown,
                Color::Brown,
            ],
        };
        *self.colors.borrow_mut() = encoded.unwrap_or(default);
        self.value_hint.clear();
    }

    /// Recreate the per-band dropdowns to match `count` and `colors`.
    fn rebuild_band_rows(&mut self, sender: &ComponentSender<Self>) {
        for row in self.band_rows.drain(..) {
            self.bands_group.remove(&row);
        }
        self.band_swatches.clear();
        let colors = self.colors.borrow().clone();
        for (pos, &role) in self.count.roles().iter().enumerate() {
            let choices = role.choices();
            let names: Vec<&str> = choices.iter().map(|c| c.name()).collect();
            let row = adw::ComboRow::builder()
                .title(format!("Band {}", pos + 1))
                .subtitle(role.label())
                .model(&gtk::StringList::new(&names))
                .build();
            let selected = colors
                .get(pos)
                .and_then(|c| choices.iter().position(|x| x == c))
                .unwrap_or(0);
            row.set_selected(selected as u32);

            let swatch = gtk::DrawingArea::builder()
                .content_width(18)
                .content_height(18)
                .valign(gtk::Align::Center)
                .build();
            let shared = self.colors.clone();
            swatch.set_draw_func(move |_, cr, w, h| {
                if let Some(&c) = shared.borrow().get(pos) {
                    draw::swatch(cr, w, h, c);
                }
            });
            row.add_prefix(&swatch);
            self.band_swatches.push(swatch);

            let sender = sender.clone();
            row.connect_selected_notify(move |r| sender.input(Msg::Band(pos, r.selected())));
            self.bands_group.add(&row);
            self.band_rows.push(row);
        }
        self.refresh_bands();
    }

    fn refresh_bands(&mut self) {
        let colors = self.colors.borrow();
        (self.band_value, self.band_detail) = text::bands(&colors);
        let names: Vec<&str> = colors.iter().map(|c| c.name()).collect();
        self.drawing
            .update_property(&[gtk::accessible::Property::Label(&format!(
                "Resistor with bands {}",
                names.join(", ")
            ))]);
        self.drawing.queue_draw();
    }

    fn refresh_nearest(&mut self) {
        self.nearest = text::nearest(&self.series_text, self.series);
    }

    fn refresh_pairs(&mut self) {
        (self.series_pair, self.parallel_pair, self.triple) =
            text::pairs(&self.combo_text, self.combo_series);
    }

    fn show_about(&self) {
        let about = adw::AboutDialog::builder()
            .application_name("FoxRes")
            .application_icon(crate::APP_ID)
            .developer_name("FoxIQ")
            .version(env!("CARGO_PKG_VERSION"))
            .website("https://www.foxiq.io")
            .issue_url("https://github.com/richardnero/FoxRes/issues")
            .license_type(gtk::License::MitX11)
            .copyright("© 2026 FoxIQ")
            .comments("The resistor toolkit: identify, choose, design and verify — from color codes to 4–20 mA loops.")
            .build();
        about.add_link("FoxIQ engineering services", FOXIQ_URL);
        about.present(Some(&self.window));
    }
}
